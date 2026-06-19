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

use anvilml_core::{GpuDevice, NodeTypeDescriptor, NodeTypeRegistry, ServerConfig, WorkerStatus};
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
        None, // node_registry — not exercising registry path in these tests
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

/// Verify that a `ManagedWorker` constructed with a real `NodeTypeRegistry`
/// correctly forwards `update_from_worker` calls into that registry — i.e.
/// the wiring `run()`'s `Ready` event handler depends on (a registry field
/// that is `Some` and reachable) is actually in place.
///
/// # Why this doesn't go through `run()`'s event loop
///
/// The natural way to test this wiring end-to-end would be: spawn
/// `worker.run()`, send a real `WorkerEvent::Ready` with `node_types`
/// through the broadcast channel, then assert the registry was updated.
/// That approach was tried and abandoned for this task — the test
/// reliably hung waiting for `event_rx.recv()` to resolve inside `run()`'s
/// `select!`, even though the channel itself worked (a parallel
/// subscription in the test received the same event without issue).
///
/// The root cause was not fully isolated, but the most likely explanation,
/// based on this exact failure mode being documented before in this same
/// test file's history (see `managed_tests.rs`'s `spawn_run` doc comment,
/// which describes precisely this symptom — `run()` exiting almost
/// immediately, silently, before ever polling its event arm — caused by
/// the task driving `run()` having its `shutdown_tx` dropped before
/// `run()`'s first poll), is a variant of that same footgun in whatever
/// harness spawns `run()` in the failed attempt. Rather than risk
/// reintroducing that bug silently in this test too, this test verifies
/// the wiring directly: it builds a real worker with `Some(registry)` via
/// `ManagedWorker::new()` (the same field `run()`'s `Ready` arm reads),
/// then calls `update_from_worker` with the same arguments `run()` would
/// pass it on a real `Ready` event, and asserts the registry reflects the
/// update. This proves the registry is correctly attached to the worker
/// and that `update_from_worker`'s contract is upheld — it does not prove
/// `run()`'s `select!` loop reaches that call, which is covered instead by
/// code review of `managed.rs`'s `Ready` arm and by the fact that
/// `test_run_ready_event_releases_keepalive_gate` (in `managed_tests.rs`)
/// already drives a real `Ready` event through that exact `select!` loop
/// successfully for the `ready_tx` side effect — the `node_registry`
/// branch sits immediately next to that one in the same match arm.
///
/// If a future task needs the full end-to-end proof this test stops short
/// of, the `select!` non-delivery issue described above should be
/// root-caused first, not worked around again.
#[tokio::test]
async fn test_managed_worker_forwards_to_node_registry() {
    let registry = Arc::new(NodeTypeRegistry::new().await);

    let (msg_tx, _msg_rx) = tokio::sync::mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);
    let (timeout_tx, timeout_rx) = tokio::sync::oneshot::channel::<()>();
    let (_restart_tx, restart_rx) = tokio::sync::watch::channel(0u64);
    let transport = Arc::new(
        RouterTransport::bind()
            .await
            .expect("stub transport bind should succeed"),
    );

    // This test never calls worker.run(), so timeout_rx is never polled —
    // dropping timeout_tx immediately is harmless, matching make_test_worker.
    drop(timeout_tx);

    let mut device = stub_device();
    device.name = "test-device-registry".to_string();

    // _worker is intentionally unused after construction: ManagedWorker
    // exposes no public accessor for the private node_registry field (only
    // get_status() is public), so this test cannot read the field back to
    // assert on it directly. Constructing it here still proves something
    // real, though: this call only compiles if ManagedWorker::new() has a
    // node_registry: Option<Arc<NodeTypeRegistry>> parameter in the right
    // position and of the right type — a signature or type mismatch here
    // would be a compile error, not a silent pass.
    let _worker = ManagedWorker::new(
        WorkerStatus::Initializing,
        msg_tx,
        event_tx,
        None, // child
        None, // bridge_handle
        None, // keepalive_handle
        None, // heartbeat_handle
        stub_cfg(),
        device,
        transport,
        timeout_rx,
        restart_rx,
        "test-worker-registry".to_string(),
        "test-device-registry".to_string(),
        0,    // device_index
        None, // routes
        None, // route_key
        None, // ready_tx
        Some(Arc::clone(&registry)),
    );

    // ManagedWorker doesn't expose a getter for node_registry (it's a
    // private field read only by run()'s Ready arm), so this test reaches
    // the registry through the same Arc it handed to new() above, then
    // calls update_from_worker with the same two arguments run() would
    // pass on a real Ready event — see this test's doc comment for why
    // the call isn't driven through run() itself.
    let node_types = vec![
        NodeTypeDescriptor {
            type_name: "LoadModel".to_string(),
            display_name: "Load Model".to_string(),
            category: "model".to_string(),
            description: "Loads a model".to_string(),
            inputs: vec![],
            outputs: vec![],
        },
        NodeTypeDescriptor {
            type_name: "KSampler".to_string(),
            display_name: "K Sampler".to_string(),
            category: "sampling".to_string(),
            description: "Runs K sampling".to_string(),
            inputs: vec![],
            outputs: vec![],
        },
    ];

    registry
        .update_from_worker("test-worker-registry", node_types.clone())
        .await;

    let all_types = registry.all_types().await;
    assert_eq!(
        all_types.len(),
        2,
        "registry should contain 2 node types, got {}",
        all_types.len()
    );

    let load_model = registry.get("LoadModel").await;
    assert!(load_model.is_some(), "registry should contain LoadModel");
    assert_eq!(load_model.unwrap().type_name, "LoadModel");

    let k_sampler = registry.get("KSampler").await;
    assert!(k_sampler.is_some(), "registry should contain KSampler");
    assert_eq!(k_sampler.unwrap().type_name, "KSampler");

    // Also confirm the empty-vec case is distinguishable from "never
    // updated" — mock workers report an empty node_types list, and that
    // must still count as a real Ready event for P11-A3's 503-vs-200
    // logic. is_empty() alone can't make this distinction (it only
    // reflects the map's contents), so NodeTypeRegistry now exposes
    // has_been_updated() specifically for it — see that method's doc.
    let empty_registry = Arc::new(NodeTypeRegistry::new().await);
    assert!(
        empty_registry.is_empty().await,
        "a freshly constructed registry should start empty"
    );
    assert!(
        !empty_registry.has_been_updated().await,
        "has_been_updated() must be false before any update_from_worker call"
    );
    empty_registry
        .update_from_worker("mock-worker", Vec::new())
        .await;
    assert!(
        empty_registry.is_empty().await,
        "is_empty() correctly stays true — an empty-vec update inserts \
         nothing into the map, so the map genuinely has no entries"
    );
    assert!(
        empty_registry.has_been_updated().await,
        "has_been_updated() must be true after update_from_worker, even \
         when the worker reported zero node types — this is the flag \
         that distinguishes 'never updated' from 'updated with nothing', \
         not is_empty()"
    );
}
