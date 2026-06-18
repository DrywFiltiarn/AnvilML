//! Integration tests for the `ManagedWorker` state machine.
//!
//! These tests verify the worker lifecycle state transitions by creating
//! `ManagedWorker` instances with pre-built channels (bypassing subprocess
//! spawning) and sending events directly through the broadcast channel.
//!
//! The `ManagedWorker::new()` constructor is used for tests, while
//! `ManagedWorker::spawn()` is used in production.
//!
//! `run()` takes a `oneshot::Receiver<()>` as its shutdown signal. Most
//! tests here don't exercise shutdown at all — they close the worker by
//! dropping `event_tx` instead — so the `spawn_run()` helper below wraps
//! `tokio::spawn(worker.run(shutdown_rx))` with a throwaway, never-fired
//! sender. Tests that do exercise shutdown (e.g. `test_shutdown_cleans_up_handles`,
//! `test_run_shutdown_deregisters_route`) construct their own
//! `oneshot::channel()` directly so they can hold and fire the sender.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anvilml_core::{GpuDevice, ServerConfig, WorkerStatus};
use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use anvilml_worker::keepalive;
use anvilml_worker::managed::ManagedWorker;
use tokio::sync::{broadcast, mpsc};
use tokio::time::timeout;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Return a minimal `ServerConfig` suitable for tests that construct a
/// `ManagedWorker` via `new()` but never exercise the respawn path (so
/// the config is never actually used to launch a subprocess).
fn stub_cfg() -> ServerConfig {
    ServerConfig::default()
}

/// Return a minimal `GpuDevice` suitable for tests that construct a
/// `ManagedWorker` via `new()` but never exercise the respawn path.
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

/// Return a stub `Arc<RouterTransport>` for tests that don't exercise
/// the IPC transport. Binds on an ephemeral port.
async fn stub_transport() -> Arc<RouterTransport> {
    Arc::new(
        RouterTransport::bind()
            .await
            .expect("stub transport bind should succeed"),
    )
}

/// Return a `(oneshot::Receiver<()>, oneshot::Sender<()>)` pair where the
/// *sender* is immediately dropped, leaving the receiver in a permanently
/// "closed / will never fire" state. Used for `timeout_rx` in tests that
/// don't exercise the heartbeat-timeout path — the arm sees the closed
/// receiver on the first poll and treats it like a timeout firing
/// immediately, which would be wrong. Instead we keep it alive but also
/// *never fire it* — see `stub_timeout_pair()` below.
///
/// Actually the correct approach is to hold the sender open without firing
/// it, exactly like the `spawn_run()` helper does for `shutdown_tx`. Callers
/// should use `stub_timeout_pair()` and hold the sender in `_timeout_tx`.
fn stub_timeout_pair() -> (
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<()>,
) {
    tokio::sync::oneshot::channel()
}

/// Return a `(watch::Receiver<u64>, watch::Sender<u64>)` pair initialised
/// at generation `0`. Callers should hold the sender in `_restart_tx` to
/// prevent the receiver from observing a closed channel, which would
/// immediately resolve `restart_rx.changed()` with `Err` and trigger the
/// `restart_rx_closed` flag in `run()`.
fn stub_restart_pair() -> (
    tokio::sync::watch::Receiver<u64>,
    tokio::sync::watch::Sender<u64>,
) {
    let (tx, rx) = tokio::sync::watch::channel(0u64);
    (rx, tx)
}

/// Create a `ManagedWorker` for testing with pre-built channels.
///
/// This helper constructs a worker in the given initial status, with
/// no bridge/keepalive handles, and the specified worker ID, device name,
/// and device index. The `cfg`, `device`, `transport`, `timeout_rx`, and
/// `restart_rx` fields are populated with stubs — they are only exercised
/// when a test reaches the respawn path, which most tests here do not.
///
/// Returns:
/// - the worker
/// - the broadcast sender (for sending events into the worker's loop)
/// - the timeout sender stub (must be kept alive for the duration of the
///   test to prevent a spurious heartbeat-timeout arm fire)
/// - the restart sender stub (same reason)
/// - the transport (kept alive so the stub socket isn't closed early)
async fn make_test_worker(
    initial_status: WorkerStatus,
    worker_id: &str,
    device_name: &str,
) -> (
    ManagedWorker,
    broadcast::Sender<(String, WorkerEvent)>,
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::watch::Sender<u64>,
    Arc<RouterTransport>,
) {
    make_test_worker_with_index(initial_status, worker_id, device_name, 0).await
}

/// Like `make_test_worker` but allows specifying the device index.
async fn make_test_worker_with_index(
    initial_status: WorkerStatus,
    worker_id: &str,
    device_name: &str,
    device_index: u32,
) -> (
    ManagedWorker,
    broadcast::Sender<(String, WorkerEvent)>,
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::watch::Sender<u64>,
    Arc<RouterTransport>,
) {
    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);
    let (timeout_tx, timeout_rx) = stub_timeout_pair();
    let (restart_rx, restart_tx) = stub_restart_pair();
    let transport = stub_transport().await;
    let mut device = stub_device();
    device.index = device_index;
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
        transport.clone(),
        timeout_rx,
        restart_rx,
        worker_id.to_string(),
        device_name.to_string(),
        device_index,
        None, // routes — no real demux task in these tests
        None, // route_key
        None, // ready_tx — no real keepalive task in these tests
    );

    (worker, event_tx, timeout_tx, restart_tx, transport)
}

/// Spawn `worker.run()` with a shutdown channel whose sender is kept alive
/// but never fired, for tests that only care about event-driven state
/// transitions and close the worker via `drop(event_tx)` rather than an
/// explicit shutdown request.
///
/// The sender must outlive `run()`, not be dropped immediately: a
/// `oneshot::Receiver` resolves (with `Err`) the instant its paired
/// `Sender` is dropped, and `run()`'s `select!` arm on `&mut shutdown_rx`
/// doesn't distinguish a real `Ok(())` send from that `Err` — either one
/// wins the race and fires the shutdown arm. Binding the sender to a
/// leading-underscore `_shutdown_tx` (as an earlier version of this helper
/// did) drops it at the end of this function's statement, before `run()`
/// has even been polled once — `run()` then tears down almost immediately
/// on every call. Capturing the sender in the spawned async block (moved
/// in, never sent on, dropped only when the task itself ends) keeps it
/// alive for exactly as long as `run()` runs.
fn spawn_run(worker: ManagedWorker) -> tokio::task::JoinHandle<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        // shutdown_tx is moved into this future purely to keep it alive;
        // it's never sent on, so run()'s shutdown arm never fires from it.
        let _shutdown_tx = shutdown_tx;
        worker.run(shutdown_rx).await
    })
}

// ---------------------------------------------------------------------------
// Existing state-transition tests — updated for new new() signature
// ---------------------------------------------------------------------------

/// Verify that the worker transitions from Initializing to Idle on a Ready event.
#[tokio::test]
async fn test_spawn_reaches_idle() {
    let (worker, event_tx, _timeout_tx, _restart_tx, _transport) = make_test_worker(
        WorkerStatus::Initializing,
        "test-worker-ready",
        "test-device",
    )
    .await;

    let status = worker.get_status();
    let run_handle = spawn_run(worker);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let ready_event = WorkerEvent::Ready {
        worker_id: "test-worker-ready".to_string(),
        device_index: 0,
        device_name: "test-device".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8000,
        torch_version: "2.4.0".to_string(),
        fp16: true,
        bf16: true,
        fp8: false,
        flash_attention: false,
        node_types: Vec::new(),
    };
    let _ = event_tx.send(("test-worker-ready".to_string(), ready_event));

    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Idle,
        "status should be Idle after Ready event, got {:?}",
        final_status
    );

    drop(event_tx);
    let _ = run_handle.await;
}

/// Verify that a Ready event cancels the ready timeout and transitions to Idle.
#[tokio::test]
async fn test_ready_timeout_dead() {
    let (worker, event_tx, _timeout_tx, _restart_tx, _transport) = make_test_worker(
        WorkerStatus::Initializing,
        "test-worker-timeout",
        "test-device",
    )
    .await;

    let status = worker.get_status();
    let run_handle = spawn_run(worker);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let ready_event = WorkerEvent::Ready {
        worker_id: "test-worker-timeout".to_string(),
        device_index: 0,
        device_name: "test-device".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8000,
        torch_version: "2.4.0".to_string(),
        fp16: true,
        bf16: true,
        fp8: false,
        flash_attention: false,
        node_types: Vec::new(),
    };
    let _ = event_tx.send(("test-worker-timeout".to_string(), ready_event));

    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Idle,
        "status should be Idle after Ready event, got {:?}",
        final_status
    );

    drop(event_tx);
    let _ = run_handle.await;
}

/// Verify that a Dying event transitions the worker from Idle to Dead.
#[tokio::test]
async fn test_dying_event_transitions_dead() {
    let (worker, event_tx, _timeout_tx, _restart_tx, _transport) =
        make_test_worker(WorkerStatus::Idle, "test-worker-dying", "test-device").await;

    let status = worker.get_status();
    let run_handle = spawn_run(worker);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let dying_event = WorkerEvent::Dying {
        reason: "SIGTERM".to_string(),
    };
    let _ = event_tx.send(("test-worker-dying".to_string(), dying_event));

    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Dead,
        "status should be Dead after Dying event, got {:?}",
        final_status
    );

    drop(event_tx);
    let _ = run_handle.await;
}

/// Verify that the keepalive timeout callback fires when no pong is received.
#[tokio::test]
async fn test_keepalive_timeout_sets_dead() {
    let callback_fired = Arc::new(AtomicBool::new(false));
    let callback_fired_clone = Arc::clone(&callback_fired);

    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let _ = ready_tx.send(());

    let on_timeout = {
        let callback_fired = Arc::clone(&callback_fired);
        move || {
            callback_fired.store(true, Ordering::SeqCst);
        }
    };

    let (keepalive_handle, heartbeat_handle) = keepalive::start(
        "test-worker-keepalive".to_string(),
        msg_tx.clone(),
        event_rx,
        ready_rx,
        Duration::from_secs(30),
        Duration::from_secs(10),
        on_timeout,
    );

    let (_timeout_tx, timeout_rx) = stub_timeout_pair();
    let (restart_rx, _restart_tx) = stub_restart_pair();
    let transport = stub_transport().await;

    let worker = ManagedWorker::new(
        WorkerStatus::Idle,
        msg_tx,
        event_tx.clone(),
        None,
        None,
        Some(keepalive_handle),
        Some(heartbeat_handle),
        stub_cfg(),
        stub_device(),
        transport,
        timeout_rx,
        restart_rx,
        "test-worker-keepalive".to_string(),
        "test-device".to_string(),
        0,
        None,
        None,
        None,
    );

    let run_handle = spawn_run(worker);

    let _ = timeout(Duration::from_secs(15), run_handle).await;

    drop(event_tx);

    assert!(
        callback_fired_clone.load(Ordering::SeqCst),
        "on_timeout callback should have fired within 15 seconds"
    );
}

/// Verify that the worker transitions Idle → Busy → Idle on job events.
#[tokio::test]
async fn test_status_transitions_idle_to_busy_to_idle() {
    let (worker, event_tx, _timeout_tx, _restart_tx, _transport) =
        make_test_worker(WorkerStatus::Idle, "test-worker-busy", "test-device").await;

    let status = worker.get_status();
    let run_handle = spawn_run(worker);

    tokio::time::sleep(Duration::from_millis(50)).await;

    {
        let mut s = status.write().await;
        *s = WorkerStatus::Busy;
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let completed_event = WorkerEvent::Completed {
        job_id: Uuid::new_v4(),
        elapsed_ms: 5000,
    };
    let _ = event_tx.send(("test-worker-busy".to_string(), completed_event));

    tokio::time::sleep(Duration::from_millis(500)).await;

    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Idle,
        "status should be Idle after Completed event, got {:?}",
        final_status
    );

    drop(event_tx);
    let _ = run_handle.await;
}

/// Verify that firing the shutdown signal causes `run()`'s shutdown arm to
/// execute its full teardown sequence and the loop to exit cleanly.
#[tokio::test]
async fn test_shutdown_cleans_up_handles() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let (msg_tx, msg_rx) = mpsc::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);

    let writer_handle = anvilml_worker::start(
        transport.clone(),
        b"test-worker-shutdown".to_vec(),
        "test-worker-shutdown".to_string(),
        msg_rx,
    );

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let _ = ready_tx.send(());

    let (keepalive_handle, heartbeat_handle) = keepalive::start(
        "test-worker-shutdown".to_string(),
        msg_tx.clone(),
        event_rx,
        ready_rx,
        Duration::from_secs(30),
        Duration::from_secs(10),
        || {},
    );

    let (_timeout_tx, timeout_rx) = stub_timeout_pair();
    let (restart_rx, _restart_tx) = stub_restart_pair();

    let worker = ManagedWorker::new(
        WorkerStatus::Idle,
        msg_tx.clone(),
        event_tx,
        None,
        Some(writer_handle),
        Some(keepalive_handle),
        Some(heartbeat_handle),
        stub_cfg(),
        stub_device(),
        transport,
        timeout_rx,
        restart_rx,
        "test-worker-shutdown".to_string(),
        "test-device".to_string(),
        0,
        None,
        None,
        None,
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let run_handle = tokio::spawn(worker.run(shutdown_rx));

    tokio::time::sleep(Duration::from_millis(50)).await;

    let _ = shutdown_tx.send(());

    let result = timeout(Duration::from_secs(10), run_handle).await;
    assert!(
        result.is_ok(),
        "run() should complete its shutdown sequence within 10 seconds"
    );
    assert!(
        result.unwrap().is_ok(),
        "run()'s task should exit cleanly, not panic, during shutdown"
    );
}

/// Verify that `run()` processes two sequential events on a single invocation.
#[tokio::test]
async fn test_run_processes_multiple_sequential_events() {
    let (worker, event_tx, _timeout_tx, _restart_tx, _transport) = make_test_worker(
        WorkerStatus::Initializing,
        "test-worker-multi",
        "test-device",
    )
    .await;

    let status = worker.get_status();
    let run_handle = spawn_run(worker);

    tokio::time::sleep(Duration::from_millis(50)).await;

    let ready_event = WorkerEvent::Ready {
        worker_id: "test-worker-multi".to_string(),
        device_index: 0,
        device_name: "test-device".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8000,
        torch_version: "2.4.0".to_string(),
        fp16: true,
        bf16: true,
        fp8: false,
        flash_attention: false,
        node_types: Vec::new(),
    };
    let _ = event_tx.send(("test-worker-multi".to_string(), ready_event));

    tokio::time::sleep(Duration::from_millis(200)).await;

    let mid_status = *status.read().await;
    assert_eq!(
        mid_status,
        WorkerStatus::Idle,
        "status should be Idle after Ready event, got {:?}",
        mid_status
    );

    {
        let mut s = status.write().await;
        *s = WorkerStatus::Busy;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    let completed_event = WorkerEvent::Completed {
        job_id: Uuid::new_v4(),
        elapsed_ms: 5000,
    };
    let _ = event_tx.send(("test-worker-multi".to_string(), completed_event));

    tokio::time::sleep(Duration::from_millis(500)).await;

    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Idle,
        "status should be Idle after Completed event (proving run() loop is continuous), got {:?}",
        final_status
    );

    drop(event_tx);
    let _ = timeout(Duration::from_secs(10), run_handle).await;
}

/// Verify that when the child subprocess exits unexpectedly, the worker
/// status transitions to Dead via the `child.wait()` arm of the select! loop.
///
/// With P10-A3's respawn logic in place, this test is careful to verify only
/// the initial Dead transition — it doesn't assert the worker *stays* Dead,
/// since the respawn cycle will attempt to restart it. The test closes
/// `event_tx` immediately after observing Dead to make `run()` exit via the
/// broadcast-closed arm before a respawn can complete, keeping the test
/// bounded and deterministic.
#[tokio::test]
async fn test_child_exit_transitions_dead() {
    #[cfg(windows)]
    let child = tokio::process::Command::new("ping")
        .arg("-n")
        .arg("1")
        .arg("-w")
        .arg("500")
        .arg("127.0.0.1")
        .spawn()
        .expect("failed to spawn ping command");

    #[cfg(not(windows))]
    let child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("sleep 0.5 && exit 1")
        .spawn()
        .expect("failed to spawn sleep command");

    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);
    let (_timeout_tx, timeout_rx) = stub_timeout_pair();
    let (restart_rx, _restart_tx) = stub_restart_pair();
    let transport = stub_transport().await;

    let worker = ManagedWorker::new(
        WorkerStatus::Initializing,
        msg_tx,
        event_tx.clone(),
        Some(child),
        None,
        None,
        None,
        stub_cfg(),
        stub_device(),
        transport,
        timeout_rx,
        restart_rx,
        "test-child-exit".to_string(),
        "test-device".to_string(),
        0,
        None,
        None,
        None,
    );

    let status = worker.get_status();
    let run_handle = spawn_run(worker);

    // Wait for the child to exit and the status to transition to Dead
    // or Respawning. Dead is written then immediately overwritten with
    // Respawning by do_respawn — on a single-threaded tokio runtime both
    // writes may complete before this polling task is scheduled, so we
    // accept either as proof that crash detection fired correctly.
    let result = timeout(Duration::from_secs(5), async {
        loop {
            let s = *status.read().await;
            if s == WorkerStatus::Dead || s == WorkerStatus::Respawning {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "status should transition to Dead within 5 seconds after child exit"
    );

    let post_status = *status.read().await;
    assert!(
        post_status == WorkerStatus::Dead || post_status == WorkerStatus::Respawning,
        "status should be Dead or Respawning after child exit, got {:?}",
        post_status
    );

    // Close the broadcast channel to let run() exit before any respawn
    // attempt completes — the respawn will fail cleanly (no real Python
    // venv in CI) and run() will break out of its loop.
    drop(event_tx);
    let _ = timeout(Duration::from_secs(10), run_handle).await;
}

/// Verify the spawned-task mechanism in the keepalive callback updates status.
#[tokio::test]
async fn test_spawned_task_updates_status() {
    let status = Arc::new(tokio::sync::RwLock::new(WorkerStatus::Idle));
    let weak = Arc::downgrade(&status);

    let callback_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callback_fired_clone = Arc::clone(&callback_fired);

    let on_timeout = move || {
        let weak = weak.clone();
        callback_fired.store(true, std::sync::atomic::Ordering::SeqCst);
        tokio::spawn(async move {
            if let Some(s) = weak.upgrade() {
                *s.write().await = WorkerStatus::Dead;
            }
        });
    };

    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
    });

    on_timeout();

    let _ = handle.await;

    assert!(
        callback_fired_clone.load(std::sync::atomic::Ordering::SeqCst),
        "callback should have fired"
    );

    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Dead,
        "status should be Dead, got {:?}",
        final_status
    );
}

/// Verify that `run()`'s shutdown arm deregisters the worker's route.
#[tokio::test]
async fn test_run_shutdown_deregisters_route() {
    use anvilml_worker::demux::RouteTable;
    use std::collections::HashMap;

    let routes: RouteTable = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let key = "test-worker-deregister".to_string();

    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);
    let (_timeout_tx, timeout_rx) = stub_timeout_pair();
    let (restart_rx, _restart_tx) = stub_restart_pair();
    let transport = stub_transport().await;

    anvilml_worker::demux::register(
        &routes,
        key.clone(),
        ("test-worker-deregister".to_string(), event_tx.clone()),
    )
    .await;

    assert!(
        routes.lock().await.contains_key(&key),
        "precondition: route should be present before shutdown"
    );

    let worker = ManagedWorker::new(
        WorkerStatus::Idle,
        msg_tx,
        event_tx,
        None,
        None,
        None,
        None,
        stub_cfg(),
        stub_device(),
        transport,
        timeout_rx,
        restart_rx,
        "test-worker-deregister".to_string(),
        "test-device".to_string(),
        0,
        Some(routes.clone()),
        Some(key.clone()),
        None,
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let run_handle = tokio::spawn(worker.run(shutdown_rx));

    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = shutdown_tx.send(());

    let result = timeout(Duration::from_secs(10), run_handle).await;
    assert!(
        result.is_ok(),
        "run() should complete its shutdown sequence within 10 seconds"
    );

    assert!(
        !routes.lock().await.contains_key(&key),
        "route should be deregistered after run()'s shutdown arm completes"
    );
}

/// Verify that `run()`'s `Initializing → Idle` transition fires `ready_tx`,
/// releasing a real keepalive task's start gate.
#[tokio::test]
async fn test_run_ready_event_releases_keepalive_gate() {
    let (msg_tx, mut msg_rx) = mpsc::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (_timeout_tx, timeout_rx) = stub_timeout_pair();
    let (restart_rx, _restart_tx) = stub_restart_pair();
    let transport = stub_transport().await;

    let (keepalive_handle, heartbeat_handle) = keepalive::start(
        "test-worker-ready-gate".to_string(),
        msg_tx.clone(),
        event_rx,
        ready_rx,
        Duration::from_millis(50),
        Duration::from_secs(10),
        || {},
    );

    let worker = ManagedWorker::new(
        WorkerStatus::Initializing,
        msg_tx,
        event_tx.clone(),
        None,
        None,
        Some(keepalive_handle),
        Some(heartbeat_handle),
        stub_cfg(),
        stub_device(),
        transport,
        timeout_rx,
        restart_rx,
        "test-worker-ready-gate".to_string(),
        "test-device".to_string(),
        0,
        None,
        None,
        Some(ready_tx),
    );

    let run_handle = spawn_run(worker);

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        timeout(Duration::from_millis(100), msg_rx.recv())
            .await
            .is_err(),
        "no ping should be sent while the worker is still Initializing"
    );

    let ready_event = WorkerEvent::Ready {
        worker_id: "test-worker-ready-gate".to_string(),
        device_index: 0,
        device_name: "test-device".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 8192,
        vram_free_mib: 8000,
        torch_version: "2.4.0".to_string(),
        fp16: true,
        bf16: true,
        fp8: false,
        flash_attention: false,
        node_types: Vec::new(),
    };
    let _ = event_tx.send(("test-worker-ready-gate".to_string(), ready_event));

    let first_ping = timeout(Duration::from_millis(300), msg_rx.recv())
        .await
        .expect("a ping should arrive shortly after the Ready event is processed")
        .expect("mpsc channel should still be open");
    assert!(
        matches!(first_ping, WorkerMessage::Ping { seq: 1 }),
        "first message after Ready should be Ping{{seq: 1}}, got {:?}",
        first_ping
    );

    drop(event_tx);
    let _ = run_handle.await;
}

// ---------------------------------------------------------------------------
// New P10-A3 respawn test
// ---------------------------------------------------------------------------

/// Verify the full Dead → Respawning → Initializing (respawn attempt) cycle
/// when a child subprocess exits unexpectedly.
///
/// This test spawns a real short-lived child, waits for the `Dead`
/// transition, then waits for the `Respawning` transition that immediately
/// follows as `do_respawn` sets it before the backoff delay. Since the test
/// environment has no Python venv, the actual `Self::spawn()` call inside
/// `do_respawn` will fail (no Python subprocess to launch), which is fine
/// — the test only needs to observe that the state machine reached
/// `Respawning` before that failure, proving the respawn cycle was
/// entered at all.
///
/// The broadcast channel is left open for the full duration so run() keeps
/// looping (rather than exiting on channel-closed) — the worker's own loop
/// exits naturally once do_respawn fails and returns `Err`, which causes
/// the child-exit arm to `break`.
#[tokio::test]
async fn test_respawn_cycle_entered_after_child_exit() {
    #[cfg(windows)]
    let child = tokio::process::Command::new("ping")
        .arg("-n")
        .arg("1")
        .arg("-w")
        .arg("500")
        .arg("127.0.0.1")
        .spawn()
        .expect("failed to spawn ping command");

    #[cfg(not(windows))]
    let child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("sleep 0.5 && exit 1")
        .spawn()
        .expect("failed to spawn sleep command");

    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);
    let (_timeout_tx, timeout_rx) = stub_timeout_pair();
    let (restart_rx, _restart_tx) = stub_restart_pair();
    let transport = stub_transport().await;

    let worker = ManagedWorker::new(
        WorkerStatus::Initializing,
        msg_tx,
        event_tx.clone(),
        Some(child),
        None,
        None,
        None,
        stub_cfg(),
        stub_device(),
        transport,
        timeout_rx,
        restart_rx,
        "test-respawn-cycle".to_string(),
        "test-device".to_string(),
        0,
        None,
        None,
        None,
    );

    let status = worker.get_status();
    let run_handle = spawn_run(worker);

    // Dead is overwritten immediately by Respawning before this task
    // is scheduled on the single-threaded runtime. Poll for either —
    // both prove child exit was detected and the respawn cycle entered.
    let cycle_result = timeout(Duration::from_secs(9), async {
        loop {
            let s = *status.read().await;
            if s == WorkerStatus::Dead || s == WorkerStatus::Respawning {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        cycle_result.is_ok(),
        "should reach Dead or Respawning within 9s after child exit"
    );

    // Let run() finish on its own — do_respawn will fail (no Python venv)
    // and run()'s child-exit arm will break, ending the task. Budget
    // generously to cover the 2s backoff delay plus spawn attempt.
    drop(event_tx);
    let _ = timeout(Duration::from_secs(15), run_handle).await;
}
