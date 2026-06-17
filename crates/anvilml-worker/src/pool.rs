//! WorkerPool — manages a collection of `ManagedWorker` instances.
//!
//! `WorkerPool` owns the workers, the shared `RouterTransport`, the shared
//! `EventBroadcaster`, and the single demux task (`crate::demux`) that reads
//! every worker's events off that transport. It provides methods to spawn
//! workers for all detected devices, retrieve worker info snapshots, and
//! broadcast status changes as `WsEvent::WorkerStatusChanged`.
//!
//! Each worker is tracked alongside its identity and device name (stored in
//! the pool because `ManagedWorker` does not expose these fields publicly).
//! A background monitoring task polls each worker's status at a 100ms
//! interval and broadcasts `WorkerStatusChanged` events when the status
//! changes.
//!
//! **Hard constraints:** Contain only pool management and status
//! broadcasting. No job dispatch logic — that belongs in the scheduler.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::{AnvilError, GpuDevice, ServerConfig, WorkerInfo};
use anvilml_ipc::{render_identity, EventBroadcaster, RouterTransport};
use tracing;

use crate::demux::{self, RouteTable};
use crate::managed::ManagedWorker;

/// A pool of managed workers, one per GPU device.
///
/// # Construction
///
/// Use `spawn_all()` to create a pool from a `ServerConfig` and a list of
/// `GpuDevice` values. Each device gets its own worker with a generated
/// identity (`"worker-0"`, `"worker-1"`, etc.).
pub struct WorkerPool {
    /// `ManagedWorker` does not expose `worker_id` or `device_name`
    /// publicly, so the pool stores these values alongside the worker
    /// reference. Each tuple is `(worker, worker_id, device_name)`.
    ///
    /// Wrapped in a `tokio::sync::Mutex` (rather than a bare `Vec`) so that
    /// `shutdown_all(&self)` can drain the vec and move each `Arc<ManagedWorker>`
    /// out by value through a shared reference — `WorkerPool` is held behind
    /// an `Arc` shared with `AppState` and the system stats tick task, so no
    /// method on this struct can ever take `self` by value. Moving entries
    /// out (rather than cloning the `Arc`s) is required for `Arc::try_unwrap`
    /// in `shutdown_all` to see a strong count of 1 and succeed.
    workers: tokio::sync::Mutex<Vec<(Arc<ManagedWorker>, String, String)>>,

    /// Stored for future dispatch routing logic (belongs in the scheduler).
    #[allow(dead_code)] // reserved for future dispatch routing
    transport: Arc<RouterTransport>,

    /// The shared event broadcaster used to notify WebSocket clients
    /// about worker status changes and other system events.
    broadcaster: Arc<EventBroadcaster>,

    /// The pool-wide demux task's handle (see `crate::demux`), guarded by a
    /// `Mutex` for the same reason `workers` is: `shutdown_all` only has
    /// `&self` and must still be able to take ownership of the handle to
    /// abort it. `None` only in pools built via `new()` for tests that
    /// never start a demux task at all.
    demux_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl WorkerPool {
    /// Spawn a managed worker for each device and return a `WorkerPool`.
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
    ///
    /// # Demux startup ordering
    ///
    /// The demux task is started once, before any device is spawned, with
    /// an empty routing table — not after the loop, with a fully-built one.
    /// `ManagedWorker::spawn()` starts a worker's keepalive (and so its
    /// first ping) before returning, so building the table only after every
    /// device has spawned would leave every worker's first ping/pong racing
    /// the demux task's own startup. Each worker's route is registered
    /// immediately after its own `spawn()` call returns, against a demux
    /// task that already exists, bounding that race to one lock acquisition
    /// rather than the time to spawn the whole pool.
    #[tracing::instrument(skip(cfg, devices, transport, broadcaster), fields(worker_count = %devices.len()))]
    pub async fn spawn_all(
        cfg: &ServerConfig,
        devices: &[GpuDevice],
        transport: Arc<RouterTransport>,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Result<Self, AnvilError> {
        let mut workers = Vec::with_capacity(devices.len());

        // Empty at this point — see "Demux startup ordering" above for why
        // the task must exist before any worker does, not after all of them.
        let routes: RouteTable =
            Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
        let demux_handle = demux::start(transport.clone(), routes.clone());

        for (i, device) in devices.iter().enumerate() {
            // Matches the naming convention the scheduler and worker
            // subprocess environment variables also use.
            let worker_id = format!("worker-{i}");

            let (worker, route_info) =
                ManagedWorker::spawn(cfg, device, transport.clone(), worker_id.clone()).await?;

            // Registered immediately, before any other bookkeeping for this
            // worker — its keepalive already fired ping #1 inside spawn(),
            // so every statement between spawn() returning and this
            // registration is additional race window.
            let key = render_identity(&route_info.ipc_identity);
            demux::register(&routes, key, (route_info.display_id, route_info.event_tx)).await;

            // GpuDevice's name is captured here rather than read back from
            // ManagedWorker, since device_name is private and may anyway be
            // overwritten by the Ready event later — the pool only needs a
            // stable label for WorkerInfo snapshots, not the live value.
            let device_name = device.name.clone();

            workers.push((Arc::new(worker), worker_id, device_name));
        }

        // One task per worker, polling rather than event-driven: the
        // alternative (subscribing to each worker's own event_tx) would
        // need its own demux-style fan-out, for a status display that
        // tolerates being up to 100ms stale.
        for (worker, worker_id, device_name) in &workers {
            let broadcaster = Arc::clone(&broadcaster);
            let worker_id = worker_id.clone();
            let device_index = devices
                .iter()
                .position(|d| d.name == *device_name)
                .unwrap_or(0) as u32;
            let status = worker.get_status();

            // Seeded with the worker's actual starting status so the first
            // poll iteration doesn't broadcast a spurious "change" from some
            // arbitrary default into Initializing.
            let initial_status = *status.read().await;
            let mut previous_status = initial_status;

            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let current_status = *status.read().await;

                    if current_status != previous_status {
                        tracing::debug!(
                            worker_id = %worker_id,
                            old_status = ?previous_status,
                            new_status = ?current_status,
                            "worker status changed, broadcasting"
                        );

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

        // Mandatory INFO log point per ENVIRONMENT.md §9; worker_count is
        // indexed by log aggregators.
        tracing::info!(
            worker_count = %devices.len(),
            "worker pool spawned"
        );

        Ok(Self {
            workers: tokio::sync::Mutex::new(workers),
            transport,
            broadcaster,
            demux_handle: tokio::sync::Mutex::new(Some(demux_handle)),
        })
    }

    /// Create a pool from pre-built workers (for testing), with no demux
    /// task and no background monitoring — tests that need either can
    /// start them manually against the same transport.
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
            workers: tokio::sync::Mutex::new(workers),
            transport,
            broadcaster,
            demux_handle: tokio::sync::Mutex::new(None),
        }
    }

    /// Retrieve a snapshot of all worker info.
    ///
    /// # Fields
    ///
    /// - `current_job_id`: Always `None` — the pool does not track job assignments
    ///   (that belongs in the scheduler).
    /// - `vram_used_mib`: Always `None` — VRAM usage is reported by the worker
    ///   via the `MemoryReport` event, which the pool does not currently track.
    pub async fn get_worker_infos(&self) -> Vec<WorkerInfo> {
        // Held only for this method's duration; shutdown_all is the only
        // other place touching self.workers, and shutdown happens once,
        // after the server has stopped accepting requests, so the two
        // are not expected to contend in practice.
        let workers = self.workers.lock().await;
        let mut infos = Vec::with_capacity(workers.len());

        for (worker, worker_id, device_name) in workers.iter() {
            let status = *worker.get_status().read().await;

            // Reconstructed by name match rather than stored directly,
            // since the tuple doesn't carry device_index and adding it
            // would duplicate what's already recoverable from `devices`
            // at spawn time.
            let device_index = workers
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

    /// Return a reference to the shared event broadcaster, so callers like
    /// the system stats tick can send their own events on the same bus.
    pub fn broadcaster(&self) -> &Arc<EventBroadcaster> {
        &self.broadcaster
    }

    /// Shut down every worker in the pool, then the demux task.
    ///
    /// This is the fix for workers surviving a supervisor Ctrl+C: previously
    /// nothing in `main.rs` ever called `ManagedWorker::shutdown()`, so the
    /// Python subprocesses were simply abandoned when the process exited.
    /// `with_graceful_shutdown` only drains in-flight HTTP connections — it
    /// has no knowledge of worker subprocesses.
    ///
    /// Takes `&self` because `WorkerPool` lives behind an `Arc` shared with
    /// `AppState` and the system stats tick task, so it can never be moved
    /// out at the call site. To still get owned `ManagedWorker` values (which
    /// `ManagedWorker::shutdown(mut self)` requires), this drains the entire
    /// `Vec` out of the `workers` mutex via `std::mem::take`, leaving the
    /// pool's list empty afterward. This moves each `Arc<ManagedWorker>` out
    /// by value rather than cloning it, so the strong reference count seen
    /// by `Arc::try_unwrap` below is exactly 1 for each worker — cloning
    /// instead of moving here was the bug in an earlier version of this
    /// method, since a clone always raises the count to 2 and guarantees
    /// `try_unwrap` fails.
    ///
    /// Workers are shut down sequentially rather than concurrently. This
    /// pool is sized to one worker per GPU (typically 1–8), so sequential
    /// shutdown completes well within the per-worker grace period used by
    /// `ManagedWorker::shutdown()` and keeps the shutdown log easy to read.
    ///
    /// The demux task is stopped last, after every worker, rather than
    /// first — each worker's own `shutdown()` still needs *something*
    /// notionally listening on its event channel right up until that
    /// worker's writer task delivers its `Shutdown` message, even though in
    /// practice nothing further arrives once a worker stops responding.
    /// Stopping demux first would not break shutdown, but would leave a
    /// brief window where the routing table still exists with no consumer
    /// reading it — stopping it last avoids reasoning about that window at all.
    ///
    /// # Returns
    ///
    /// Nothing. Failures to cleanly shut down an individual worker are
    /// logged at WARN rather than surfaced as an error — shutdown must
    /// proceed for all workers even if one is uncooperative.
    #[tracing::instrument(skip(self))]
    pub async fn shutdown_all(&self) {
        // shutdown_all is expected to run exactly once, at process exit,
        // so leaving the pool empty afterward is correct, not a leak.
        let drained = std::mem::take(&mut *self.workers.lock().await);

        for (worker, worker_id, _device_name) in drained {
            match Arc::try_unwrap(worker) {
                Ok(worker) => {
                    worker.shutdown().await;
                }
                Err(_) => {
                    // Given that the monitoring task only ever clones the
                    // status RwLock's Arc via get_status() — never the
                    // worker Arc itself — strong count should always be 1
                    // here; this branch existing at all is a tripwire for
                    // that invariant being broken elsewhere in the future.
                    tracing::warn!(
                        worker_id = %worker_id,
                        "could not take exclusive ownership of worker during \
                         shutdown; worker subprocess may remain running"
                    );
                }
            }
        }

        // Aborted rather than awaited: by this point every worker has
        // already been told to shut down, so there is nothing left for the
        // demux task to usefully deliver — waiting for transport.recv() to
        // itself error out would just add latency to shutdown for no benefit.
        if let Some(handle) = self.demux_handle.lock().await.take() {
            handle.abort();
        }
    }
}
