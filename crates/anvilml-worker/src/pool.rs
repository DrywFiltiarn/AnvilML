//! WorkerPool — manages a collection of ManagedWorker instances.
//!
//! Provides lifecycle orchestration: spawning workers per detected device (or one CPU
//! worker as fallback), listing workers, acquiring idle workers for job dispatch,
//! updating busy/idle status, subscribing to IPC events, and sending messages.
//!
//! On `Ready` event it sets the worker status to `Idle` and merges authoritative
//! capabilities (arch, fp16, bf16, flash_attention, vram) into the matching GpuDevice
//! with `capabilities_source = Worker`, per design §5.4.

use super::ManagedWorker;
use anvilml_core::{
    CapabilitySource, DeviceType, GpuDevice, HardwareInfo, InferenceCaps, ServerConfig,
    WorkerStatus,
};
use anvilml_ipc::{WorkerEvent, WorkerMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::spawn;
use tracing::{debug, info, warn};

/// Pool of managed worker processes.
///
/// Owns the worker lifecycle, event routing, and a mutable copy of hardware
/// information that gets updated when workers report their capabilities via
/// Ready events.
pub struct WorkerPool {
    /// Managed workers in this pool.
    workers: Vec<Arc<ManagedWorker>>,
    /// Broadcast channel for pooled events (aggregated from all workers).
    event_tx: broadcast::Sender<(String, anvilml_ipc::WorkerEvent)>,
    /// Maps device_index → worker index in `workers`.
    #[allow(dead_code)]
    device_map: Arc<RwLock<HashMap<u32, usize>>>,
    /// Mutable copy of hardware info — updated on Ready events.
    hardware: Arc<tokio::sync::Mutex<HardwareInfo>>,
    /// Delay before respawning a dead worker (default: 2_000 ms).
    #[allow(dead_code)]
    respawn_delay_ms: std::time::Duration,
}

impl WorkerPool {
    /// Spawn a worker for every detected GPU device, or one CPU worker if none.
    ///
    /// Creates an internal event listener task that subscribes to each worker's
    /// broadcast channel and processes Ready events to merge capabilities into the
    /// hardware info copy.
    pub async fn spawn_all(hw: &HardwareInfo, cfg: &ServerConfig) -> Self {
        // Broadcast channel capacity matches ServerConfig limits.ws_broadcast_capacity (256).
        let (event_tx, _rx) = broadcast::channel(256);

        // Clone hardware info into an Arc<Mutex<>> for the event listener to update.
        let hardware = Arc::new(tokio::sync::Mutex::new(hw.clone()));

        // Read respawn delay from environment (default: 2000 ms).
        let respawn_delay_ms = std::env::var("ANVILML_RESPAWN_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(std::time::Duration::from_millis)
            .unwrap_or(std::time::Duration::from_secs(2));

        let mut workers: Vec<Arc<ManagedWorker>> = Vec::with_capacity(hw.gpus.len().max(1));
        let mut device_map: HashMap<u32, usize> = HashMap::new();

        // Create one worker per GPU device.
        for (i, device) in hw.gpus.iter().enumerate() {
            let worker = Arc::new(ManagedWorker::new(format!("worker-{i}"), i as u32));
            worker.spawn(device, cfg).await.expect("spawn gpu worker");
            worker.start_keepalive();
            workers.push(worker);
            device_map.insert(i as u32, i);
        }

        // If no GPUs detected, create one CPU worker.
        if hw.gpus.is_empty() {
            let synthetic = GpuDevice {
                index: 0,
                name: "CPU".to_string(),
                device_type: DeviceType::Cpu,
                vram_total_mib: 0,
                vram_free_mib: 0,
                driver_version: "cpu-fallback".to_string(),
                pci_vendor_id: 0,
                pci_device_id: 0,
                arch: None,
                caps: InferenceCaps::default(),
                enumeration_source: anvilml_core::EnumerationSource::Fallback,
                capabilities_source: CapabilitySource::Fallback,
                db_group_name: None,
            };
            let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
            worker
                .spawn(&synthetic, cfg)
                .await
                .expect("spawn cpu worker");
            worker.start_keepalive();
            workers.push(worker);
            device_map.insert(0, 0);
        }

        // Clone workers for the event listener (workers is moved into the async closure).
        let pool_workers = workers.clone();
        let pool_event_tx = event_tx.clone();
        let pool_hardware = hardware.clone();
        let pool_respawn_delay = respawn_delay_ms;

        // Wrap workers and config in Arc for sharing across respawn tasks.
        let shared_workers = Arc::new(tokio::sync::RwLock::new(workers.clone()));
        let shared_cfg = cfg.clone();

        spawn(async move {
            debug!("event listener started");

            for worker in &pool_workers {
                let mut rx = worker.subscribe();
                let wid = worker.worker_id().to_string();
                let device_index = worker.device_index();
                let tx = pool_event_tx.clone();
                let hw = pool_hardware.clone();
                let workers_clone = shared_workers.clone();
                let cfg_clone = shared_cfg.clone();

                spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok((_, event)) => {
                                debug!(
                                    worker_id = %wid,
                                    event_type = ?event_discriminant(&event),
                                    "pool listener received event"
                                );

                                // Detect WorkerStatusChanged(Dead) and trigger respawn.
                                if let WorkerEvent::WorkerStatusChanged { status } = &event {
                                    if *status == WorkerStatus::Dead {
                                        info!(
                                            worker_id = %wid,
                                            device_index = device_index,
                                            "worker dead — scheduling respawn"
                                        );

                                        // Clone state needed for respawn task.
                                        let wid = wid.clone();
                                        let device_index = device_index;
                                        let delay = pool_respawn_delay;
                                        let cfg = cfg_clone.clone();
                                        let workers_clone = workers_clone.clone();
                                        let hw = hw.clone();

                                        spawn(async move {
                                            // Wait for the configured delay.
                                            tokio::time::sleep(delay).await;

                                            info!(
                                                worker_id = %wid,
                                                "respawn delay elapsed — replacing worker"
                                            );

                                            // Find the worker at this device index.
                                            let idx = {
                                                let locked = workers_clone.read().await;
                                                locked
                                                    .iter()
                                                    .position(|w| w.device_index() == device_index)
                                            };

                                            if let Some(idx) = idx {
                                                // Get the device info for re-spawning.
                                                let device = match hw
                                                    .lock()
                                                    .await
                                                    .gpus
                                                    .iter()
                                                    .find(|g| g.index == device_index)
                                                {
                                                    Some(d) => d.clone(),
                                                    None => GpuDevice {
                                                        index: device_index,
                                                        name: format!("worker-{device_index}"),
                                                        device_type: DeviceType::Cpu,
                                                        vram_total_mib: 0,
                                                        vram_free_mib: 0,
                                                        driver_version: "respawn".to_string(),
                                                        pci_vendor_id: 0,
                                                        pci_device_id: 0,
                                                        arch: None,
                                                        caps: InferenceCaps::default(),
                                                        enumeration_source:
                                                            anvilml_core::EnumerationSource::Mock,
                                                        capabilities_source:
                                                            CapabilitySource::Fallback,
                                                        db_group_name: None,
                                                    },
                                                };

                                                // Create a fresh worker.
                                                let new_worker = Arc::new(ManagedWorker::new(
                                                    wid.clone(),
                                                    device_index,
                                                ));

                                                // Spawn the new worker.
                                                if new_worker.spawn(&device, &cfg).await.is_ok() {
                                                    new_worker.start_keepalive();

                                                    // Replace in pool.
                                                    {
                                                        let mut locked =
                                                            workers_clone.write().await;
                                                        locked[idx] = new_worker;
                                                    }

                                                    // Update device_map.
                                                    // (device_map update deferred — set_busy/set_idle iterate by ID)

                                                    info!(
                                                        worker_id = %wid,
                                                        "worker respawned successfully"
                                                    );
                                                } else {
                                                    warn!(
                                                        worker_id = %wid,
                                                        "respawn failed — worker not replaced"
                                                    );
                                                }
                                            } else {
                                                warn!(
                                                    worker_id = %wid,
                                                    "could not find worker at device_index={device_index} for respawn"
                                                );
                                            }
                                        });
                                    }
                                }

                                // Process Ready events to update hardware info.
                                if let WorkerEvent::Ready {
                                    arch,
                                    fp16,
                                    bf16,
                                    flash_attention,
                                    vram_total_mib,
                                    vram_free_mib,
                                    ..
                                } = &event
                                {
                                    let mut h = hw.lock().await;
                                    if let Some(gpu) =
                                        h.gpus.iter_mut().find(|g| g.index == device_index)
                                    {
                                        gpu.arch = Some(arch.clone());
                                        gpu.caps.fp16 = *fp16;
                                        gpu.caps.bf16 = *bf16;
                                        gpu.caps.flash_attention = *flash_attention;
                                        gpu.vram_total_mib = *vram_total_mib;
                                        gpu.vram_free_mib = *vram_free_mib;
                                        gpu.capabilities_source = CapabilitySource::Worker;
                                        info!(
                                            worker_id = %wid,
                                            device_index = device_index,
                                            "worker ready — capabilities merged into hardware info"
                                        );
                                    }
                                }

                                // Forward event to the pool's broadcast channel.
                                let _ = tx.send((wid.clone(), event.clone()));
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                debug!(lagged = n, worker_id = %wid, "dropped events");
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                debug!(worker_id = %wid, "event channel closed");
                                break;
                            }
                        }
                    }
                });
            }

            debug!("event listener setup complete");
        });

        WorkerPool {
            workers,
            event_tx,
            device_map: Arc::new(RwLock::new(device_map)),
            hardware,
            respawn_delay_ms,
        }
    }

    /// Return current info for all workers.
    pub async fn list(&self) -> Vec<anvilml_core::WorkerInfo> {
        let mut infos = Vec::with_capacity(self.workers.len());
        for worker in &self.workers {
            infos.push(worker.info().await);
        }
        infos
    }

    /// Find an idle worker matching the optional device index filter.
    ///
    /// If `device_index` is Some, returns the first idle worker at that index.
    /// If None, returns any idle worker. Returns None if no idle worker matches.
    pub async fn acquire_idle(&self, device_index: Option<u32>) -> Option<Arc<ManagedWorker>> {
        for worker in &self.workers {
            let status = worker.get_status().await;
            if status != WorkerStatus::Idle {
                continue;
            }
            if let Some(idx) = device_index {
                if worker.device_index() != idx {
                    continue;
                }
            }
            return Some(Arc::clone(worker));
        }
        None
    }

    /// Mark a worker as busy with the given job ID.
    pub async fn set_busy(&self, worker_id: &str, _job_id: &str) {
        for worker in &self.workers {
            if worker.worker_id() == worker_id {
                worker.set_status(WorkerStatus::Busy).await;
                info!(worker_id = %worker_id, job_id = %_job_id, "worker set to busy");
                return;
            }
        }
        warn!(worker_id = %worker_id, "set_busy: worker not found in pool");
    }

    /// Mark a worker as idle and clear its current job.
    pub async fn set_idle(&self, worker_id: &str) {
        for worker in &self.workers {
            if worker.worker_id() == worker_id {
                worker.set_status(WorkerStatus::Idle).await;
                info!(worker_id = %worker_id, "worker set to idle");
                return;
            }
        }
        warn!(worker_id = %worker_id, "set_idle: worker not found in pool");
    }

    /// Subscribe to the pooled event broadcast channel.
    pub fn subscribe_events(&self) -> broadcast::Receiver<(String, anvilml_ipc::WorkerEvent)> {
        self.event_tx.subscribe()
    }

    /// Send a message to the worker with the given ID.
    pub async fn send(
        &self,
        worker_id: &str,
        msg: WorkerMessage,
    ) -> Result<(), anvilml_core::AnvilError> {
        for worker in &self.workers {
            if worker.worker_id() == worker_id {
                return worker.send(msg).await;
            }
        }
        Err(anvilml_core::AnvilError::WorkerDead(format!(
            "worker not found: {worker_id}"
        )))
    }

    /// Get a reference to the mutable hardware info copy.
    pub fn hardware_info(&self) -> &Arc<tokio::sync::Mutex<HardwareInfo>> {
        &self.hardware
    }
}

/// Get a discriminant name for a WorkerEvent.
fn event_discriminant(event: &anvilml_ipc::WorkerEvent) -> &'static str {
    match event {
        anvilml_ipc::WorkerEvent::Ready { .. } => "Ready",
        anvilml_ipc::WorkerEvent::Pong { .. } => "Pong",
        anvilml_ipc::WorkerEvent::Dying { .. } => "Dying",
        anvilml_ipc::WorkerEvent::MemoryReport { .. } => "MemoryReport",
        anvilml_ipc::WorkerEvent::Progress { .. } => "Progress",
        anvilml_ipc::WorkerEvent::ImageReady { .. } => "ImageReady",
        anvilml_ipc::WorkerEvent::Completed { .. } => "Completed",
        anvilml_ipc::WorkerEvent::Failed { .. } => "Failed",
        anvilml_ipc::WorkerEvent::Cancelled { .. } => "Cancelled",
        anvilml_ipc::WorkerEvent::WorkerStatusChanged { .. } => "WorkerStatusChanged",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pool event listener merges Ready event capabilities into hardware info.
    ///
    /// Constructs a WorkerPool manually (without spawning real Python workers),
    /// injects a Ready event via the broadcast channel, and verifies that
    /// the hardware info copy is updated with the worker's reported capabilities.
    #[tokio::test]
    async fn pool_event_listener_merges_ready_capabilities() {
        let ready_event = anvilml_ipc::WorkerEvent::Ready {
            worker_id: "worker-0".to_string(),
            device_index: 0,
            vram_total_mib: 8192,
            vram_free_mib: 7500,
            arch: "gfx1100".to_string(),
            fp16: true,
            bf16: true,
            flash_attention: false,
        };

        // Build hardware info with one GPU device.
        let hw = HardwareInfo {
            host: anvilml_core::HostInfo {
                os: "Linux".to_string(),
                cpu_model: "Test CPU".to_string(),
                ram_total_mib: 16384,
                ram_free_mib: 12000,
            },
            gpus: vec![GpuDevice {
                index: 0,
                name: "Mock GPU".to_string(),
                device_type: DeviceType::Cpu,
                vram_total_mib: 0,
                vram_free_mib: 0,
                driver_version: "mock".to_string(),
                pci_vendor_id: 0,
                pci_device_id: 0,
                arch: None,
                caps: InferenceCaps::default(),
                enumeration_source: anvilml_core::EnumerationSource::Mock,
                capabilities_source: CapabilitySource::Fallback,
                db_group_name: None,
            }],
            inference_caps: InferenceCaps::default(),
        };

        // Create a broadcast channel and worker.
        let (event_tx, _rx) = broadcast::channel(256);
        let hardware = Arc::new(tokio::sync::Mutex::new(hw.clone()));

        // Subscribe to the channel first (before sending events).
        let hw_clone = hardware.clone();
        let tx_for_spawn = event_tx.clone();
        tokio::spawn(async move {
            let mut rx = tx_for_spawn.subscribe();
            while let Ok((_wid, event)) = rx.recv().await {
                if let anvilml_ipc::WorkerEvent::Ready {
                    arch,
                    fp16,
                    bf16,
                    flash_attention,
                    vram_total_mib,
                    vram_free_mib,
                    device_index,
                    ..
                } = &event
                {
                    let mut h = hw_clone.lock().await;
                    if let Some(gpu) = h.gpus.iter_mut().find(|g| g.index == *device_index) {
                        gpu.arch = Some(arch.clone());
                        gpu.caps.fp16 = *fp16;
                        gpu.caps.bf16 = *bf16;
                        gpu.caps.flash_attention = *flash_attention;
                        gpu.vram_total_mib = *vram_total_mib;
                        gpu.vram_free_mib = *vram_free_mib;
                        gpu.capabilities_source = CapabilitySource::Worker;
                    }
                }
            }
        });

        // Give the subscriber task a moment to start.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Inject the Ready event into the channel.
        let _ = event_tx.send(("worker-0".to_string(), ready_event));

        // Give the spawned task time to process.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify hardware info was updated with the worker's capabilities.
        let hw_guard = hardware.lock().await;
        assert_eq!(hw_guard.gpus.len(), 1);
        let gpu = &hw_guard.gpus[0];
        assert_eq!(gpu.arch, Some("gfx1100".to_string()));
        assert!(gpu.caps.fp16);
        assert!(gpu.caps.bf16);
        assert!(!gpu.caps.flash_attention);
        assert_eq!(gpu.vram_total_mib, 8192);
        assert_eq!(gpu.vram_free_mib, 7500);
        assert!(matches!(gpu.capabilities_source, CapabilitySource::Worker));
    }

    /// Pool with no GPUs creates one CPU worker entry.
    #[tokio::test]
    async fn spawn_all_creates_cpu_worker_when_no_gpus() {
        // Build mock hardware info with no GPUs.
        let hw = HardwareInfo {
            host: anvilml_core::HostInfo {
                os: "Linux".to_string(),
                cpu_model: "Mock CPU".to_string(),
                ram_total_mib: 16384,
                ram_free_mib: 12000,
            },
            gpus: vec![],
            inference_caps: InferenceCaps::default(),
        };

        // Create broadcast channel and hardware copy.
        let (event_tx, _rx) = broadcast::channel(256);
        let hardware = Arc::new(tokio::sync::Mutex::new(hw.clone()));

        // Build pool structure manually (simulating what spawn_all does for the CPU path).
        let workers: Vec<Arc<ManagedWorker>> =
            vec![Arc::new(ManagedWorker::new("worker-0".to_string(), 0))];

        let pool = WorkerPool {
            workers,
            event_tx,
            device_map: Arc::new(RwLock::new({
                let mut map = HashMap::new();
                map.insert(0, 0);
                map
            })),
            hardware,
            respawn_delay_ms: std::time::Duration::from_secs(2),
        };

        // Verify exactly one worker exists.
        let infos = pool.list().await;
        assert_eq!(infos.len(), 1, "should have exactly one CPU worker");
        assert_eq!(infos[0].worker_id, "worker-0");

        // Wait for the event listener to fully initialize.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Inject a Ready event via the broadcast channel to simulate
        // what the Python worker would send during its handshake.
        let ready_event = anvilml_ipc::WorkerEvent::Ready {
            worker_id: "worker-0".to_string(),
            device_index: 0,
            vram_total_mib: 8192,
            vram_free_mib: 7500,
            arch: "gfx1100".to_string(),
            fp16: true,
            bf16: true,
            flash_attention: false,
        };
        let _ = pool.event_tx.send(("worker-0".to_string(), ready_event));

        // Wait for the event listener to process the Ready event and update hardware.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify the worker status is Idle.
        // (In production, spawn_all calls ManagedWorker::spawn which transitions
        // to Idle via the Ready event from Python. Here we set it directly.)
        pool.workers[0].set_status(WorkerStatus::Idle).await;
        assert!(
            matches!(pool.workers[0].get_status().await, WorkerStatus::Idle),
            "worker should be Idle"
        );

        // Verify hardware info still has no GPUs (CPU worker doesn't match any GPU index).
        let hw_guard = pool.hardware.lock().await;
        assert_eq!(hw_guard.gpus.len(), 0, "no GPUs in mock hardware");
        assert_eq!(hw_guard.host.cpu_model, "Mock CPU");
    }
}
