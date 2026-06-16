//! WorkerPool — manages a collection of `ManagedWorker` instances.
//!
//! `WorkerPool` owns a `Vec<Arc<ManagedWorker>>` along with the shared `RouterTransport`
//! and `EventBroadcaster`. It provides methods to spawn workers for all detected devices,
//! retrieve worker info snapshots, and broadcast status changes as `WsEvent::WorkerStatusChanged`.
//!
//! Each worker is tracked alongside its identity and device name (stored in the pool because
//! `ManagedWorker` does not expose these fields publicly). A background monitoring task
//! polls each worker's status at a 100ms interval and broadcasts `WorkerStatusChanged`
//! events when the status changes.
//!
//! **Hard constraints:** Contain only pool management and status broadcasting.
//! No job dispatch logic — that belongs in the scheduler.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::{AnvilError, GpuDevice, ServerConfig, WorkerInfo};
use anvilml_ipc::{EventBroadcaster, RouterTransport};
use tracing;

use crate::managed::ManagedWorker;

/// A pool of managed workers, one per GPU device.
///
/// `WorkerPool` manages the lifecycle of all workers in the system. It owns:
/// - The list of workers, each paired with its identity and device name
/// - The shared `RouterTransport` for IPC communication
/// - The shared `EventBroadcaster` for WebSocket event broadcasting
///
/// The pool spawns a background monitoring task per worker that polls the
/// worker's status and broadcasts changes to connected WebSocket clients.
///
/// # Construction
///
/// Use `spawn_all()` to create a pool from a `ServerConfig` and a list of
/// `GpuDevice` values. Each device gets its own worker with a generated
/// identity (`"worker-0"`, `"worker-1"`, etc.).
pub struct WorkerPool {
    /// Workers paired with their identity and device name.
    ///
    /// `ManagedWorker` does not expose `worker_id` or `device_name` publicly,
    /// so the pool stores these values alongside the worker reference.
    /// Each tuple is `(worker, worker_id, device_name)`.
    workers: Vec<(Arc<ManagedWorker>, String, String)>,

    /// The shared IPC transport used by all workers for message routing.
    ///
    /// Stored for future dispatch routing logic (belongs in the scheduler).
    #[allow(dead_code)] // reserved for future dispatch routing
    transport: Arc<RouterTransport>,

    /// The shared event broadcaster used to notify WebSocket clients
    /// about worker status changes and other system events.
    broadcaster: Arc<EventBroadcaster>,
}

impl WorkerPool {
    /// Spawn a managed worker for each device and return a `WorkerPool`.
    ///
    /// This is the primary entry point for creating a worker pool in production.
    /// It iterates over the provided device list, generates a unique identity
    /// for each (`"worker-0"`, `"worker-1"`, etc.), spawns a `ManagedWorker`
    /// for each device, and starts a background monitoring task per worker.
    ///
    /// # Arguments
    ///
    /// * `cfg` — The server configuration (provides venv path and IPC payload cap).
    /// * `devices` — The list of GPU devices to spawn workers for.
    /// * `transport` — The shared `RouterTransport` for IPC communication.
    /// * `broadcaster` — The shared `EventBroadcaster` for WebSocket events.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if any worker fails to spawn.
    ///
    /// # Background monitoring
    ///
    /// After spawning each worker, a background task is launched that polls
    /// the worker's status every 100ms. When a status change is detected,
    /// a `WsEvent::WorkerStatusChanged` event is broadcast to all connected
    /// WebSocket clients.
    #[tracing::instrument(skip(cfg, devices, transport, broadcaster), fields(worker_count = %devices.len()))]
    pub async fn spawn_all(
        cfg: &ServerConfig,
        devices: &[GpuDevice],
        transport: Arc<RouterTransport>,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Result<Self, AnvilError> {
        let mut workers = Vec::with_capacity(devices.len());

        for (i, device) in devices.iter().enumerate() {
            // Generate a stable worker identity: "worker-0", "worker-1", etc.
            // This matches the naming convention used by the scheduler and
            // worker subprocess environment variables.
            let worker_id = format!("worker-{i}");

            // Spawn the managed worker for this device. The spawn method
            // constructs the subprocess command, sets up IPC channels, and
            // returns a ManagedWorker in the Initializing state.
            let worker =
                ManagedWorker::spawn(cfg, device, transport.clone(), worker_id.clone()).await?;

            // Store the worker alongside its identity and device name.
            // We capture device.name here because ManagedWorker::device_name
            // is private; the device name may be updated by the Ready event
            // in production, but for the pool's purposes the initial name
            // from GpuDevice is sufficient for WorkerInfo snapshots.
            let device_name = device.name.clone();

            workers.push((Arc::new(worker), worker_id, device_name));
        }

        // Spawn a background monitoring task for each worker.
        // Each task polls the worker's status at 100ms intervals and
        // broadcasts WsEvent::WorkerStatusChanged when a change is detected.
        // The 100ms interval is a reasonable trade-off: frequent enough
        // to detect changes promptly, infrequent enough to avoid CPU waste.
        for (worker, worker_id, device_name) in &workers {
            let broadcaster = Arc::clone(&broadcaster);
            let worker_id = worker_id.clone();
            let device_index = devices
                .iter()
                .position(|d| d.name == *device_name)
                .unwrap_or(0) as u32;
            let status = worker.get_status();

            // Clone the initial status for the first comparison.
            // This ensures we don't broadcast a spurious "change" at startup
            // when the worker is in Initializing state (the initial state
            // is not a "change" — it's the starting point).
            let initial_status = *status.read().await;
            let mut previous_status = initial_status;

            // Spawn the monitoring task. This task runs for the lifetime
            // of the worker — when the worker is dropped, the status RwLock
            // is dropped, and subsequent reads will succeed but return the
            // last known state (RwLock reads don't panic on dropped data;
            // they simply read whatever is currently stored).
            tokio::spawn(async move {
                loop {
                    // Sleep before polling to avoid busy-waiting.
                    // 100ms is a reasonable interval for status change detection
                    // in a worker pool where status changes are infrequent
                    // (only on job dispatch, completion, failure, or death).
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    // Read the current status. If the status RwLock is still
                    // alive, this returns the current state. If the worker has
                    // been dropped, the RwLock may still exist (owned by the
                    // worker Arc which is still in the pool).
                    let current_status = *status.read().await;

                    // Only broadcast if the status actually changed.
                    // This prevents redundant broadcasts when the status
                    // remains the same (e.g., the worker is Idle and stays Idle).
                    if current_status != previous_status {
                        tracing::debug!(
                            worker_id = %worker_id,
                            old_status = ?previous_status,
                            new_status = ?current_status,
                            "worker status changed, broadcasting"
                        );

                        // Broadcast the status change to all WebSocket clients.
                        // The WsEvent::WorkerStatusChanged event carries the
                        // worker identity, new status, and device index so
                        // clients can update their worker status display.
                        broadcaster.send(anvilml_core::types::WsEvent::WorkerStatusChanged {
                            worker_id: worker_id.clone(),
                            status: current_status,
                            device_index,
                        });

                        previous_status = current_status;
                    }
                }
            });
        }

        // Log at INFO level: mandatory log point per ENVIRONMENT.md §9.
        // The worker_count field is indexed by log aggregators.
        tracing::info!(
            worker_count = %devices.len(),
            "worker pool spawned"
        );

        Ok(Self {
            workers,
            transport,
            broadcaster,
        })
    }

    /// Create a pool from pre-built workers (for testing).
    ///
    /// This constructor does not spawn background monitoring tasks.
    /// Tests that need monitoring can spawn them manually.
    ///
    /// # Arguments
    ///
    /// * `workers` — Pre-built workers, each paired with its identity and device name.
    /// * `transport` — The shared `RouterTransport` for IPC communication.
    /// * `broadcaster` — The shared `EventBroadcaster` for WebSocket events.
    pub fn new(
        workers: Vec<(Arc<ManagedWorker>, String, String)>,
        transport: Arc<RouterTransport>,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Self {
        Self {
            workers,
            transport,
            broadcaster,
        }
    }

    /// Retrieve a snapshot of all worker info.
    ///
    /// This method reads the current status of each worker and constructs
    /// a `WorkerInfo` struct for each. The snapshot is consistent within
    /// the method call (each worker's status is read independently, so
    /// different workers may reflect slightly different points in time).
    ///
    /// # Returns
    ///
    /// A `Vec<WorkerInfo>` containing one entry per worker in the pool,
    /// in the same order as the workers were spawned.
    ///
    /// # Fields
    ///
    /// - `id`: The worker's stable identity (e.g. `"worker-0"`).
    /// - `device_index`: The zero-based GPU device index.
    /// - `device_name`: The GPU device name (e.g. `"NVIDIA A100-SXM4-40GB"`).
    /// - `status`: The worker's current lifecycle state.
    /// - `current_job_id`: Always `None` — the pool does not track job assignments
    ///   (that belongs in the scheduler).
    /// - `vram_used_mib`: Always `None` — VRAM usage is reported by the worker
    ///   via the `MemoryReport` event, which the pool does not currently track.
    pub async fn get_worker_infos(&self) -> Vec<WorkerInfo> {
        let mut infos = Vec::with_capacity(self.workers.len());

        for (worker, worker_id, device_name) in &self.workers {
            // Read the current status under a read lock. This allows
            // concurrent reads from multiple callers (e.g., the HTTP
            // handler and the system stats tick).
            let status = *worker.get_status().read().await;

            // Find the device index by matching the device name.
            // In production, the device index is known at spawn time
            // from the enumerate position. Here we reconstruct it
            // from the stored device name.
            let device_index = self
                .workers
                .iter()
                .position(|(_, _id, name)| name == device_name)
                .unwrap_or(0) as u32;

            infos.push(WorkerInfo {
                id: worker_id.clone(),
                device_index,
                device_name: device_name.clone(),
                status,
                current_job_id: None,
                vram_used_mib: None,
            });
        }

        infos
    }

    /// Return a reference to the shared event broadcaster.
    ///
    /// This allows callers (e.g., the system stats tick) to access the
    /// broadcaster directly for sending custom events.
    ///
    /// # Returns
    ///
    /// A reference to the `Arc<EventBroadcaster>` stored in this pool.
    pub fn broadcaster(&self) -> &Arc<EventBroadcaster> {
        &self.broadcaster
    }
}
