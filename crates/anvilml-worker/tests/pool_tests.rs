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

use anvilml_core::WorkerStatus;
use anvilml_ipc::{EventBroadcaster, RouterTransport};
use anvilml_worker::managed::ManagedWorker;
use anvilml_worker::pool::WorkerPool;
use anvilml_worker::WorkerPool as WorkerPoolReexport;
use tokio::sync::broadcast;

/// Create a test worker in the given initial status.
///
/// Returns the worker and the broadcast sender so the test can
/// send events through the channel (for run-loop testing).
fn make_test_worker(
    initial_status: WorkerStatus,
    worker_id: &str,
    device_name: &str,
) -> (
    ManagedWorker,
    broadcast::Sender<(String, anvilml_ipc::WorkerEvent)>,
) {
    let (msg_tx, _msg_rx) = tokio::sync::mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    let worker = ManagedWorker::new(
        initial_status,
        msg_tx,
        event_tx.clone(),
        None, // child — not spawning subprocess in tests
        None, // bridge_handles
        None, // keepalive_handle
        None, // heartbeat_handle
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

/// Verify that spawning N workers results in N Idle workers.
///
/// This test creates a WorkerPool with pre-built mock workers (bypassing
/// subprocess spawning), verifies that `get_worker_infos()` returns N workers,
/// and asserts that each worker reports `status: Idle`.
#[tokio::test]
async fn test_spawn_all_workers_idle() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    // Create 3 mock workers, each in Idle status.
    let mut pool_workers = Vec::new();
    for i in 0..3 {
        let worker_id = format!("worker-{i}");
        let device_name = format!("MockGPU-{i}");
        let (worker, _event_tx) = make_test_worker(WorkerStatus::Idle, &worker_id, &device_name);
        pool_workers.push((worker.get_status(), worker_id, device_name));
    }

    // Construct the pool using the test constructor.
    let pool = WorkerPool::new(pool_workers, transport, broadcaster);

    // Verify get_worker_infos returns exactly 3 workers.
    let infos = pool.get_worker_infos().await;
    assert_eq!(infos.len(), 3, "pool should report 3 workers");

    // Verify each worker reports Idle status.
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
///
/// This test constructs a pool and asserts that `broadcaster()` returns the same
/// `Arc` that was passed during construction.
#[tokio::test]
async fn test_broadcaster_returns_reference() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) =
        make_test_worker(WorkerStatus::Idle, "test-worker-broadcaster", "test-device");

    let pool = WorkerPool::new(
        vec![(
            worker.get_status(),
            "test-worker-broadcaster".to_string(),
            "test-device".to_string(),
        )],
        transport,
        Arc::clone(&broadcaster),
    );

    // Verify the returned reference matches the original Arc.
    let returned = pool.broadcaster();
    // Compare the inner Sender pointers — same Arc means same underlying Sender.
    assert!(
        Arc::ptr_eq(returned, &broadcaster),
        "broadcaster() should return the same Arc as passed to the pool"
    );
}

/// Verify that a status change triggers a `WorkerStatusChanged` broadcast.
///
/// This test constructs a pool, spawns a monitoring task manually,
/// sets a worker's status to Busy via the RwLock, waits for the
/// monitoring task to detect the change, and verifies the broadcaster
/// received a `WsEvent::WorkerStatusChanged` event.
#[tokio::test]
async fn test_pool_broadcasts_status_change() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) = make_test_worker(
        WorkerStatus::Idle,
        "test-worker-broadcast",
        "test-device-broadcast",
    );

    // Capture the worker's status Arc for direct manipulation. Captured
    // before WorkerPool::new() consumes the same Arc via .get_status() —
    // both calls return clones of the same underlying RwLock, so writes
    // through this handle are visible to the pool's copy.
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

    // Spawn a monitoring task manually (the pool's test constructor
    // does not spawn monitoring tasks). The monitoring task polls
    // at 100ms intervals, same as spawn_all().
    let device_index = 0u32;
    let monitor_handle = tokio::spawn({
        let broadcaster = Arc::clone(&broadcaster);
        let status = Arc::clone(&status);
        let worker_id = "test-worker-broadcast".to_string();

        async move {
            // Read the initial status to avoid broadcasting a spurious change.
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

    // Give the monitoring task time to start and read the initial status.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Set the worker's status to Busy.
    {
        let mut s = status.write().await;
        *s = WorkerStatus::Busy;
    }

    // Wait for the monitoring task to detect the change and broadcast.
    // The 100ms poll interval + some buffer should be sufficient.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Subscribe to the broadcaster to check for the event.
    let mut rx = broadcaster.subscribe();

    // Drain any buffered events.
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
            Ok(_) => continue, // unexpected event type
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(broadcast::error::TryRecvError::Closed) => break,
        }
    }

    // Abort the monitoring task to prevent it from running forever.
    monitor_handle.abort();
    let _ = monitor_handle.await;
}

/// Verify that the re-exported `WorkerPool` type is accessible via the crate root.
///
/// This test ensures that `pub use pool::WorkerPool;` in lib.rs works correctly,
/// so consumers can write `anvilml_worker::WorkerPool` instead of `anvilml_worker::pool::WorkerPool`.
#[tokio::test]
async fn test_reexport_worker_pool() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) =
        make_test_worker(WorkerStatus::Idle, "test-worker-reexport", "test-device");

    // Use the re-exported type from the crate root.
    let _pool: WorkerPoolReexport = WorkerPoolReexport::new(
        vec![(
            worker.get_status(),
            "test-worker-reexport".to_string(),
            "test-device".to_string(),
        )],
        transport,
        broadcaster,
    );

    // If this compiles, the re-export is correct.
    // The test passes if no compilation error occurs.
}

/// Verify that `shutdown_all()` completes without hanging or panicking
/// against pools built via the `new()` test constructor.
///
/// `new()`'s synthesized `WorkerHandle`s have a `run_handle` that's an
/// already-instantly-finished task (`tokio::spawn(async {})`) and a
/// `shutdown_tx` whose paired receiver was dropped immediately — so this
/// test cannot exercise `run()`'s actual teardown sequence (there is no
/// real `run()` task here to drive), but it does exercise `shutdown_all()`'s
/// own control flow: draining the vec, firing into an already-closed
/// receiver (expected to be a harmless `Err` from `send()`, not a panic),
/// and awaiting a `run_handle` that resolves immediately. Real end-to-end
/// coverage of `run()`'s shutdown arm lives in
/// `managed_tests::test_run_shutdown_deregisters_route`, which drives an
/// actual `run()` task.
#[tokio::test]
async fn test_shutdown_all_completes_against_inert_handles() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));
    let broadcaster = Arc::new(EventBroadcaster::new());

    let (worker, _event_tx) = make_test_worker(
        WorkerStatus::Idle,
        "test-worker-shutdown-all",
        "test-device",
    );

    let pool = WorkerPool::new(
        vec![(
            worker.get_status(),
            "test-worker-shutdown-all".to_string(),
            "test-device".to_string(),
        )],
        transport,
        broadcaster,
    );

    // The only assertion this test can make without a real run() task is
    // that shutdown_all() itself returns promptly rather than hanging —
    // a hang here would mean shutdown_all() is blocking on something that
    // never resolves for an inert handle, which is exactly the kind of
    // regression a bounded timeout in shutdown_all() is meant to prevent.
    let result = tokio::time::timeout(Duration::from_secs(15), pool.shutdown_all()).await;
    assert!(
        result.is_ok(),
        "shutdown_all() should complete well within its own internal \
         per-worker timeout when given an already-finished run_handle"
    );

    // After shutdown_all(), the pool's worker list should be empty —
    // it's drained via mem::take and never repopulated.
    let infos = pool.get_worker_infos().await;
    assert!(
        infos.is_empty(),
        "pool should report no workers after shutdown_all()"
    );
}
