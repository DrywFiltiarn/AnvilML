//! Integration tests for the `WorkerPool` struct.
//!
//! These tests verify pool construction, worker info retrieval, and
//! status change broadcasting. `WorkerPool::new()` (the test constructor)
//! takes pre-built `(status, worker_id, device_name)` triples rather than
//! `ManagedWorker` values — the pool no longer holds `ManagedWorker`
//! instances at all once `run()` has consumed them, so its test
//! constructor mirrors the same shape. `make_test_worker` below still
//! builds a real `ManagedWorker` (via `ManagedWorker::new()` with
//! pre-built channels) purely so its `get_status()` accessor can hand
//! back the same status `Arc` a test will later write to directly.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::{GpuDevice, ServerConfig, WorkerStatus};
use anvilml_ipc::{EventBroadcaster, RouterTransport};
use anvilml_worker::managed::ManagedWorker;
use anvilml_worker::pool::WorkerPool;
use anvilml_worker::WorkerPool as WorkerPoolReexport;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn stub_cfg() -> ServerConfig {
    ServerConfig::default()
}

fn stub_device() -> GpuDevice {
    GpuDevice {
        index: 0,
        name: "stub-device".to_string(),
        db_name: None,
        device_type: anvilml_core::DeviceType::Cpu,
        vram_total_mib: 0,
        vram_free_mib: 0,
        driver_version: String::new(),
        pci_vendor_id: 0,
        pci_device_id: 0,
        arch: None,
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Vulkan,
        capabilities_source: anvilml_core::CapabilitySource::DeviceTable,
    }
}

/// Create a test worker in the given initial status.
///
/// Returns the worker and the broadcast sender so the test can
/// send events through the channel (for run-loop testing).
async fn make_test_worker(
    initial_status: WorkerStatus,
    worker_id: &str,
    device_name: &str,
) -> (
    ManagedWorker,
    broadcast::Sender<(String, anvilml_ipc::WorkerEvent)>,
) {
    let (msg_tx, _msg_rx) = tokio::sync::mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);
    let (timeout_tx, timeout_rx) = tokio::sync::oneshot::channel::<()>();
    let (_restart_tx, restart_rx) = tokio::sync::watch::channel(0u64);
    let transport = Arc::new(
        RouterTransport::bind()
            .await
            .expect("stub transport bind should succeed"),
    );

    // immediately the timeout arm fires spuriously.
    // timeout_tx is intentionally dropped here — pool_tests never calls
    // worker.run(), so timeout_rx is never polled and the drop is harmless.
    drop(timeout_tx);

    let mut device = stub_device();
    device.name = device_name.to_string();

    let worker = ManagedWorker::new(
        initial_status,
        msg_tx,
        event_tx.clone(),
        None, // child — not spawning subprocess in tests
        None, // bridge_handle
        None, // keepalive_handle
        None, // heartbeat_handle
        stub_cfg(),
        device,
        transport,
        timeout_rx,
        restart_rx,
        worker_id.to_string(),
        device_name.to_string(),
        0,    // device_index
        None, // routes — these tests exercise the pool's test constructor,
        // which never starts a real demux task
        None, // route_key
        None, // ready_tx — no real keepalive task in this test
    );

    (worker, event_tx)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify that spawning N workers results in N Idle workers.
#[tokio::test]
async fn test_spawn_all_workers_idle() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let mut pool_workers = Vec::new();
    for i in 0..3 {
        let worker_id = format!("worker-{i}");
        let device_name = format!("MockGPU-{i}");
        let (worker, _event_tx) =
            make_test_worker(WorkerStatus::Idle, &worker_id, &device_name).await;
        pool_workers.push((worker.get_status(), worker_id, device_name));
    }

    let pool = WorkerPool::new(pool_workers, transport, broadcaster);

    let infos = pool.get_worker_infos().await;
    assert_eq!(infos.len(), 3, "pool should report 3 workers");

    for (i, info) in infos.iter().enumerate() {
        assert_eq!(
            info.status,
            WorkerStatus::Idle,
            "worker {i} should be Idle, got {:?}",
            info.status
        );
        assert_eq!(info.id, format!("worker-{i}"), "worker {i} id should match");
        assert_eq!(
            info.device_name,
            format!("MockGPU-{i}"),
            "worker {i} device name should match"
        );
        assert_eq!(
            info.device_index, i as u32,
            "worker {i} device index should match"
        );
        assert!(
            info.current_job_id.is_none(),
            "worker {i} current_job_id should be None"
        );
        assert!(
            info.vram_used_mib.is_none(),
            "worker {i} vram_used_mib should be None"
        );
    }
}

/// Verify that `broadcaster()` returns a valid reference to the stored EventBroadcaster.
#[tokio::test]
async fn test_broadcaster_returns_reference() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) =
        make_test_worker(WorkerStatus::Idle, "test-worker-broadcaster", "test-device").await;

    let pool = WorkerPool::new(
        vec![(
            worker.get_status(),
            "test-worker-broadcaster".to_string(),
            "test-device".to_string(),
        )],
        transport,
        Arc::clone(&broadcaster),
    );

    let returned = pool.broadcaster();
    assert!(
        Arc::ptr_eq(returned, &broadcaster),
        "broadcaster() should return the same Arc as passed to the pool"
    );
}

/// Verify that a status change triggers a `WorkerStatusChanged` broadcast.
#[tokio::test]
async fn test_pool_broadcasts_status_change() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) = make_test_worker(
        WorkerStatus::Idle,
        "test-worker-broadcast",
        "test-device-broadcast",
    )
    .await;

    let status = worker.get_status();

    let _pool = WorkerPool::new(
        vec![(
            worker.get_status(),
            "test-worker-broadcast".to_string(),
            "test-device-broadcast".to_string(),
        )],
        transport,
        Arc::clone(&broadcaster),
    );

    let device_index = 0u32;
    let monitor_handle = tokio::spawn({
        let broadcaster = Arc::clone(&broadcaster);
        let status = Arc::clone(&status);
        let worker_id = "test-worker-broadcast".to_string();

        async move {
            let mut previous_status = *status.read().await;

            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;

                let current_status = *status.read().await;

                if current_status != previous_status {
                    broadcaster.send(anvilml_core::types::WsEvent::WorkerStatusChanged {
                        worker_id: worker_id.clone(),
                        status: current_status,
                        device_index,
                    });
                    previous_status = current_status;
                }
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(150)).await;

    {
        let mut s = status.write().await;
        *s = WorkerStatus::Busy;
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut rx = broadcaster.subscribe();

    loop {
        match rx.try_recv() {
            Ok(anvilml_core::types::WsEvent::WorkerStatusChanged {
                worker_id,
                status,
                device_index: idx,
            }) => {
                assert_eq!(worker_id, "test-worker-broadcast");
                assert_eq!(status, WorkerStatus::Busy);
                assert_eq!(idx, 0);
                break;
            }
            Ok(_) => continue,
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }

    monitor_handle.abort();
    let _ = monitor_handle.await;
}

/// Verify that the re-exported `WorkerPool` type is accessible via the crate root.
#[tokio::test]
async fn test_reexport_worker_pool() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) =
        make_test_worker(WorkerStatus::Idle, "test-worker-reexport", "test-device").await;

    let _pool: WorkerPoolReexport = WorkerPoolReexport::new(
        vec![(
            worker.get_status(),
            "test-worker-reexport".to_string(),
            "test-device".to_string(),
        )],
        transport,
        broadcaster,
    );
}

/// Verify that `shutdown_all()` completes without hanging or panicking
/// against pools built via the `new()` test constructor.
#[tokio::test]
async fn test_shutdown_all_completes_against_inert_handles() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) = make_test_worker(
        WorkerStatus::Idle,
        "test-worker-shutdown-all",
        "test-device",
    )
    .await;

    let pool = WorkerPool::new(
        vec![(
            worker.get_status(),
            "test-worker-shutdown-all".to_string(),
            "test-device".to_string(),
        )],
        transport,
        broadcaster,
    );

    let result = tokio::time::timeout(Duration::from_secs(15), pool.shutdown_all()).await;
    assert!(
        result.is_ok(),
        "shutdown_all() should complete well within its own internal \
         per-worker timeout when given an already-finished run_handle"
    );

    let infos = pool.get_worker_infos().await;
    assert!(
        infos.is_empty(),
        "pool should report no workers after shutdown_all()"
    );
}
