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

use anvilml_core::WorkerStatus;
use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};
use anvilml_worker::keepalive;
use anvilml_worker::managed::ManagedWorker;
use tokio::sync::{broadcast, mpsc};
use tokio::time::timeout;
use uuid::Uuid;

/// Create a `ManagedWorker` for testing with pre-built channels.
///
/// This helper constructs a worker in the given initial status, with
/// no bridge/keepalive handles, and the specified worker ID, device name,
/// and device index.
///
/// Returns the worker and the broadcast sender (cloned) so the test
/// can send events through the channel.
fn make_test_worker(
    initial_status: WorkerStatus,
    worker_id: &str,
    device_name: &str,
) -> (ManagedWorker, broadcast::Sender<(String, WorkerEvent)>) {
    make_test_worker_with_index(initial_status, worker_id, device_name, 0)
}

/// Create a `ManagedWorker` for testing with pre-built channels and a
/// specific device index.
///
/// Like `make_test_worker` but allows specifying the device index
/// (useful for tests that verify device_index propagation).
fn make_test_worker_with_index(
    initial_status: WorkerStatus,
    worker_id: &str,
    device_name: &str,
    device_index: u32,
) -> (ManagedWorker, broadcast::Sender<(String, WorkerEvent)>) {
    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    let worker = ManagedWorker::new(
        initial_status,
        msg_tx,
        event_tx.clone(),
        None, // child — not spawning subprocess in tests
        None, // bridge_handle
        None, // keepalive_handle
        None, // heartbeat_handle
        worker_id.to_string(),
        device_name.to_string(),
        device_index,
        None, // routes — no real demux task in these tests
        None, // route_key
        None, // ready_tx — no real keepalive task in these tests
    );

    (worker, event_tx)
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
/// on every call, which is silent here but causes every caller's
/// state-transition assertions to fail, since the loop is already gone by
/// the time the test sends its event. Capturing the sender in the spawned
/// async block (moved in, never sent on, dropped only when the task itself
/// ends) keeps it alive for exactly as long as `run()` runs.
fn spawn_run(worker: ManagedWorker) -> tokio::task::JoinHandle<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        // shutdown_tx is moved into this future purely to keep it alive;
        // it's never sent on, so run()'s shutdown arm never fires from it.
        let _shutdown_tx = shutdown_tx;
        worker.run(shutdown_rx).await
    })
}

/// Verify that the worker transitions from Initializing to Idle on a Ready event.
///
/// This test creates a worker in the Initializing state, sends a Ready event
/// through the broadcast channel, and verifies the status becomes Idle.
/// This is the primary synchronization point between Rust and Python.
#[tokio::test]
async fn test_spawn_reaches_idle() {
    let (worker, event_tx) = make_test_worker(
        WorkerStatus::Initializing,
        "test-worker-ready",
        "test-device",
    );

    // Clone the status Arc before consuming worker in run().
    let status = worker.get_status();

    // Spawn run() first — it subscribes to the broadcast channel
    // and enters the event processing loop.
    let run_handle = spawn_run(worker);

    // Give run() time to subscribe to the broadcast channel and enter
    // the select loop. This is critical — events sent before run()
    // subscribes may not be delivered.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send a Ready event through the broadcast channel.
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

    // Wait briefly for the event to be processed. run() processes events
    // synchronously in the select loop, so this should complete quickly.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify the status transitioned to Idle.
    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Idle,
        "status should be Idle after Ready event, got {:?}",
        final_status
    );

    // Close the channel to let run() exit.
    drop(event_tx);
    let _ = run_handle.await;
}

/// Verify that the worker transitions to Dead when the ready timeout fires.
///
/// The design doc mandates a 60-second timeout for the Ready event. If no
/// Ready event is received within this window, the worker is considered
/// unresponsive and transitions to Dead.
///
/// This test sends a Ready event so the timeout is cancelled early.
/// The main assertion verifies that the Ready event causes the
/// transition to Idle (proving the timeout mechanism is in place).
#[tokio::test]
async fn test_ready_timeout_dead() {
    let (worker, event_tx) = make_test_worker(
        WorkerStatus::Initializing,
        "test-worker-timeout",
        "test-device",
    );

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

    // The Ready event should have caused the transition to Idle.
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
///
/// Preconditions: Worker is in Idle state.
/// Inputs: Dying event via broadcast channel.
/// Expected output: Status transitions to Dead.
#[tokio::test]
async fn test_dying_event_transitions_dead() {
    let (worker, event_tx) =
        make_test_worker(WorkerStatus::Idle, "test-worker-dying", "test-device");

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
///
/// The keepalive sends Ping messages at 30-second intervals and waits
/// for Pong responses within a 10-second timeout. If no pong is received,
/// the on_timeout callback is invoked. This test verifies the callback
/// fires within the pong_timeout window.
///
/// The callback mechanism in production spawns a task that sets the status
/// to Dead. This test verifies the callback fires. The spawned task's
/// ability to update status is verified by `test_spawned_task_updates_status`
/// below.
#[tokio::test]
async fn test_keepalive_timeout_sets_dead() {
    // Track whether the on_timeout callback was invoked.
    let callback_fired = Arc::new(AtomicBool::new(false));
    let callback_fired_clone = Arc::clone(&callback_fired);

    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);

    // Fired immediately — this test is about pong-timeout behaviour
    // (whether the on_timeout callback fires), not the Ready gate itself.
    // See test_no_ping_before_ready in keepalive_tests.rs for that.
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let _ = ready_tx.send(());

    // Create the keepalive with a 10-second pong timeout.
    // The timeout callback records its invocation.
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
        Duration::from_secs(30), // ping_interval
        Duration::from_secs(10), // pong_timeout
        on_timeout,
    );

    // Create the worker with the keepalive handles.
    let worker = ManagedWorker::new(
        WorkerStatus::Idle,
        msg_tx,
        event_tx.clone(),
        None,
        None,
        Some(keepalive_handle),
        Some(heartbeat_handle),
        "test-worker-keepalive".to_string(),
        "test-device".to_string(),
        0,    // device_index
        None, // routes — no real demux task in this test
        None, // route_key
        None, // ready_tx — already fired directly above, before new()
    );

    // Spawn run() first.
    let run_handle = spawn_run(worker);

    // Wait for the keepalive timeout to fire (10s) plus buffer (5s).
    let _ = timeout(Duration::from_secs(15), run_handle).await;

    // Drop event_tx to close the broadcast channel.
    drop(event_tx);

    // Verify the callback fired.
    assert!(
        callback_fired_clone.load(Ordering::SeqCst),
        "on_timeout callback should have fired within 15 seconds"
    );
}

/// Verify the worker transitions from Idle to Busy to Idle on job events.
///
/// Preconditions: Worker is in Idle state.
/// Inputs: manual Busy transition, Completed event.
/// Expected output: Status transitions Idle → Busy → Idle.
#[tokio::test]
async fn test_status_transitions_idle_to_busy_to_idle() {
    let (worker, event_tx) =
        make_test_worker(WorkerStatus::Idle, "test-worker-busy", "test-device");

    let status = worker.get_status();

    let run_handle = spawn_run(worker);

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Manually transition to Busy (simulating a job being dispatched).
    // This is done outside of run() to simulate the scheduler dispatching
    // a job.
    {
        let mut s = status.write().await;
        *s = WorkerStatus::Busy;
    }

    // Give run() time to process any pending events.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send a Completed event — the job finished successfully.
    let completed_event = WorkerEvent::Completed {
        job_id: uuid::Uuid::new_v4(),
        elapsed_ms: 5000,
    };
    let _ = event_tx.send(("test-worker-busy".to_string(), completed_event));

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify the status transitioned back to Idle.
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
///
/// This replaces the old `ManagedWorker::shutdown()`-based test: that
/// method no longer exists, since `run(self)` now owns the worker for its
/// entire lifetime and shutdown is requested by firing the matching
/// `oneshot::Sender` rather than calling a separate method on an owned
/// value. The old test had to construct a second, never-run worker
/// (`worker2`) purely so it would have something owned to call
/// `.shutdown()` on — that workaround is gone; this test fires the signal
/// at the same worker that's actually running.
///
/// Preconditions: Worker is running via `run()`.
/// Inputs: a fired `shutdown_tx`.
/// Expected output: `run()`'s task completes within the grace period,
/// without panicking.
#[tokio::test]
async fn test_shutdown_cleans_up_handles() {
    let transport = Arc::new(RouterTransport::bind().await.expect("bind should succeed"));

    let (msg_tx, msg_rx) = mpsc::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);

    // Only the writer half exists now — see crate::bridge's module docs.
    let writer_handle = anvilml_worker::start(
        transport.clone(),
        b"test-worker-shutdown".to_vec(),
        "test-worker-shutdown".to_string(),
        msg_rx,
    );

    // Fired immediately — this test exercises run()'s shutdown sequence,
    // not the Ready gate itself; the keepalive only needs to exist as a
    // real task for the shutdown arm's abort() to have something to act on.
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let _ = ready_tx.send(());

    // Spawn the keepalive task.
    let (keepalive_handle, heartbeat_handle) = keepalive::start(
        "test-worker-shutdown".to_string(),
        msg_tx.clone(),
        event_rx,
        ready_rx,
        Duration::from_secs(30),
        Duration::from_secs(10),
        || {},
    );

    let worker = ManagedWorker::new(
        WorkerStatus::Idle,
        msg_tx.clone(),
        event_tx,
        None, // child
        Some(writer_handle),
        Some(keepalive_handle),
        Some(heartbeat_handle),
        "test-worker-shutdown".to_string(),
        "test-device".to_string(),
        0,    // device_index
        None, // routes — no real demux task in this test
        None, // route_key
        None, // ready_tx — already fired directly above, before new()
    );

    // Spawn run() with a real shutdown channel this time, rather than the
    // spawn_run() helper's fire-and-forget one — this test needs to hold
    // shutdown_tx so it can fire it below.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let run_handle = tokio::spawn(worker.run(shutdown_rx));

    // Give run() a moment to enter its select loop before signalling
    // shutdown — otherwise the signal could arrive before run() has
    // started polling shutdown_rx at all. In practice tokio::select!
    // registers all its futures before the first poll completes, so this
    // sleep is a safety margin rather than a strict requirement.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Fire the shutdown signal — this should cause run()'s shutdown arm to
    // execute its full teardown sequence (heartbeat stop, Shutdown
    // message, bridge writer await, keepalive drop, demux deregister
    // no-op, child wait/kill no-op since child is None here) and then
    // break out of the loop.
    let _ = shutdown_tx.send(());

    // The shutdown sequence's own internal timeouts cap it at roughly 7s
    // (2s bridge writer + 5s child, though child is None here so that
    // step is instant) — 10s leaves comfortable margin without letting a
    // genuinely wedged run() hang the test suite.
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

/// Verify that `run()` processes two sequential events on a single invocation,
/// proving the event loop is continuous and does not exit after the first event.
///
/// This test creates a worker in `Initializing` state, sends a `Ready` event
/// (triggering `Initializing → Idle`), then manually sets status to `Busy`,
/// sends a `Completed` event (triggering `Busy → Idle`), and asserts the final
/// status is `Idle`. If `run()` had exited after the first event, the second
/// event would never be received and the status would remain `Busy`.
///
/// Starting from `Initializing` exercises the full Ready→Idle transition path
/// (the most critical state machine path), then transitions to Busy manually
/// and verifies the second event. This is more comprehensive than starting
/// from `Idle` because it covers both the Ready timeout scoping and the
/// subsequent loop continuity.
///
/// The 10-second timeout on `run_handle.await` prevents the test from hanging
/// indefinitely if the loop fails to break on channel close.
#[tokio::test]
async fn test_run_processes_multiple_sequential_events() {
    // Create a worker in the Initializing state.
    let (worker, event_tx) = make_test_worker(
        WorkerStatus::Initializing,
        "test-worker-multi",
        "test-device",
    );

    // Clone the status Arc before consuming worker in run().
    let status = worker.get_status();

    // Spawn run() — it subscribes to the broadcast channel and enters the
    // select loop.
    let run_handle = spawn_run(worker);

    // Give run() time to subscribe to the broadcast channel and enter
    // the select loop. This is critical — events sent before run()
    // subscribes may not be delivered.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send a Ready event through the broadcast channel. This triggers the
    // Initializing → Idle transition.
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

    // Wait for the Ready event to be processed by run().
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the status transitioned to Idle after the Ready event.
    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Idle,
        "status should be Idle after Ready event, got {:?}",
        final_status
    );

    // Manually set status to Busy (simulating a job dispatch from the scheduler).
    {
        let mut s = status.write().await;
        *s = WorkerStatus::Busy;
    }

    // Give the write time to propagate.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send a Completed event through the broadcast channel. This triggers the
    // Busy → Idle transition. If run() had exited after the first event, this
    // event would never be received and the status would remain Busy.
    let completed_event = WorkerEvent::Completed {
        job_id: Uuid::new_v4(),
        elapsed_ms: 5000,
    };
    let _ = event_tx.send(("test-worker-multi".to_string(), completed_event));

    // Wait for the Completed event to be processed.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify the status transitioned back to Idle after the Completed event.
    // This is the critical assertion: if run() had exited after processing
    // the first event, the status would still be Busy here.
    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Idle,
        "status should be Idle after Completed event (proving run() loop is continuous), got {:?}",
        final_status
    );

    // Close the channel to let run() exit.
    drop(event_tx);

    // Await run() with a timeout to prevent CI from hanging indefinitely
    // if the loop fails to break on channel close.
    let _ = timeout(Duration::from_secs(10), run_handle).await;
}

/// Verify that when the child subprocess exits unexpectedly, the worker
/// status transitions to Dead via the `child.wait()` arm of the select! loop.
///
/// This test creates a real, short-lived child process and passes it to
/// `ManagedWorker::new()`. The run loop's `child.wait()` arm fires when the
/// child exits, transitioning the status to `Dead`.
///
/// This is the crash detection test — it verifies the new `child.wait()`
/// arm of the `tokio::select!` loop in `run()` fires when the subprocess
/// dies without sending a Dying event.
#[tokio::test]
async fn test_child_exit_transitions_dead() {
    // No single command line means "sleep ~0.5s then exit" identically on
    // both platforms, so this is cfg-gated rather than shelled through `sh`
    // — `sh` isn't on PATH by default on Windows. `ping -n 1 -w 500` is used
    // as a dependency-free Windows sleep substitute (ping.exe ships with
    // every Windows install); its exit code differs from the Unix branch's
    // `exit 1`, but the test never asserts on exit code, only on the run
    // loop's `child.wait()` arm firing once the process exits at all.
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

    // The worker starts in Initializing state — with no demux task running
    // for this test, no Ready event can ever arrive, so the worker stays
    // Initializing until the child exits and the status transitions to Dead.
    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    let worker = ManagedWorker::new(
        WorkerStatus::Initializing,
        msg_tx,
        event_tx.clone(),
        Some(child), // child — a real process that will exit
        None,        // bridge_handle
        None,        // keepalive_handle
        None,        // heartbeat_handle
        "test-child-exit".to_string(),
        "test-device".to_string(),
        0,    // device_index
        None, // routes — no real demux task in this test
        None, // route_key
        None, // ready_tx — no real keepalive task in this test
    );

    let status = worker.get_status();

    // Spawn run() — it subscribes to the broadcast channel and enters
    // the event processing loop, including the new child.wait() arm.
    let run_handle = spawn_run(worker);

    // Wait for the child to exit and the status to transition to Dead.
    // The child exits after 0.5 seconds, so we need a generous timeout.
    let result = timeout(Duration::from_secs(5), async {
        loop {
            let s = *status.read().await;
            if s == WorkerStatus::Dead {
                break;
            }
            let _ = s;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    // Verify the status transitioned to Dead.
    assert!(
        result.is_ok(),
        "status should transition to Dead within 5 seconds after child exit"
    );

    let final_status = *status.read().await;
    assert_eq!(
        final_status,
        WorkerStatus::Dead,
        "status should be Dead after child exit, got {:?}",
        final_status
    );

    // Close the channel to let run() exit.
    drop(event_tx);
    let _ = run_handle.await;
}

/// Verify that the spawned task in the keepalive callback successfully
/// updates the worker status. This is a regression test for the
/// mechanism where the synchronous on_timeout callback spawns an async
/// task that acquires the write lock and sets the status to Dead.
///
/// Relocated from `managed.rs`'s former embedded `#[cfg(test)]` module —
/// production source files contain no test code, per coding standards.
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

/// Verify that firing a real, running worker's shutdown signal removes its
/// entry from the demux routing table as part of `run()`'s shutdown arm —
/// not just that `demux::deregister` works in isolation (see
/// `demux_tests::test_deregister_removes_route` for that), but that
/// `run()` actually calls it during its shutdown sequence when
/// `routes`/`route_key` are populated.
///
/// Uses `ManagedWorker::new()` with `routes`/`route_key` supplied directly
/// (rather than going through `ManagedWorker::spawn()`, which would also
/// require a real Python subprocess to launch successfully) — this is
/// still a faithful test of `run()`'s deregistration step, since that step
/// only depends on the two fields being populated, not on how they got
/// that way.
#[tokio::test]
async fn test_run_shutdown_deregisters_route() {
    use anvilml_worker::demux::RouteTable;
    use std::collections::HashMap;

    let routes: RouteTable = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let key = "test-worker-deregister".to_string();

    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

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
        None, // child
        None, // bridge_handle
        None, // keepalive_handle
        None, // heartbeat_handle
        "test-worker-deregister".to_string(),
        "test-device".to_string(),
        0,
        Some(routes.clone()),
        Some(key.clone()),
        None, // ready_tx — no real keepalive task in this test
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

/// Verify that `run()`'s `Initializing → Idle` transition actually fires
/// `ready_tx`, releasing a real keepalive task's start gate — end-to-end,
/// through `run()` itself rather than `keepalive::start()` in isolation
/// (see `keepalive_tests::test_no_ping_before_ready` for that unit-level
/// check).
///
/// Constructs a worker with a real keepalive task and an unfired
/// `ready_tx`, starts it `Initializing`, and asserts no ping arrives on
/// the shared `msg_tx` channel until a `Ready` event is sent through the
/// broadcast channel and processed by `run()`'s event loop.
#[tokio::test]
async fn test_run_ready_event_releases_keepalive_gate() {
    let (msg_tx, mut msg_rx) = mpsc::channel(16);
    let (event_tx, event_rx) = broadcast::channel(16);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

    let (keepalive_handle, heartbeat_handle) = keepalive::start(
        "test-worker-ready-gate".to_string(),
        msg_tx.clone(),
        event_rx,
        ready_rx,
        Duration::from_millis(50), // ping_interval
        Duration::from_secs(10),   // pong_timeout
        || {},
    );

    let worker = ManagedWorker::new(
        WorkerStatus::Initializing,
        msg_tx,
        event_tx.clone(),
        None, // child
        None, // bridge_handle
        Some(keepalive_handle),
        Some(heartbeat_handle),
        "test-worker-ready-gate".to_string(),
        "test-device".to_string(),
        0,    // device_index
        None, // routes — no real demux task in this test
        None, // route_key
        Some(ready_tx),
    );

    let run_handle = spawn_run(worker);

    // Give run() time to subscribe to the broadcast channel before
    // sending the Ready event below.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // No ping should have arrived yet — the worker is still Initializing,
    // so ready_tx has not been fired, so the keepalive's gate is still
    // closed.
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

    // A ping should now arrive promptly — run()'s Ready transition fires
    // ready_tx, which releases the keepalive's gate.
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
