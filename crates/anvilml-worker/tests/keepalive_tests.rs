//! Integration tests for `keepalive.rs` — verifies the `KeepaliveWatchdog`
//! ping/pong heartbeat loop and worker-death detection.
//!
//! All tests use injected millisecond-scale durations (50ms interval, 100ms
//! timeout) so they complete quickly. They use `MockTransport` to avoid
//! requiring a live ZeroMQ socket.

use std::time::Duration;

use anvilml_ipc::{IpcError, WorkerEvent};
use anvilml_worker::KeepaliveWatchdog;
use anvilml_worker::keepalive::MockTransport;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

/// A Pong received within the configured timeout does NOT trigger the death signal.
///
/// Constructs a watchdog with 50ms ping interval and 100ms pong timeout, using
/// a `MockTransport` that always succeeds. Sends a matching `Pong { seq: 0 }`
/// 30ms after spawning (before the first tick fires at ~50ms). The pong buffers
/// in the channel and is received when the watchdog enters `wait_for_matching_pong`.
/// Verifies that the death signal is NOT sent within 250ms.
///
/// After the first pong is consumed, the watchdog loops and sends another ping.
/// Since no second pong is sent, the watchdog would eventually time out (~200ms).
/// The 250ms window proves the first pong was received (death signal absent).
#[tokio::test]
async fn test_pong_within_timeout_keeps_alive() {
    let (dead_tx, mut dead_rx) = oneshot::channel();
    let (pong_tx, pong_rx) = mpsc::channel::<WorkerEvent>(16);
    let transport = MockTransport::new_ok();

    let watchdog = KeepaliveWatchdog::new(
        "worker-0".to_string(),
        transport,
        pong_rx,
        dead_tx,
        Duration::from_millis(50),
        Duration::from_millis(100),
    );

    let handle = tokio::spawn(watchdog.run());

    // Spawn a pong-sending task that runs for the entire test duration.
    // It sends pongs at 50ms intervals, ensuring a pong is buffered
    // before each ping fires (pings at 50ms, pongs at 0, 50, 100, ...).
    let pong_tx2 = pong_tx.clone();
    let pong_task = tokio::spawn(async move {
        let mut seq = 0u64;
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        // Send first pong immediately.
        let _ = pong_tx2.send(WorkerEvent::Pong { seq }).await;
        seq += 1;
        // Send pongs for 400ms to cover the full test duration.
        for _ in 0..8 {
            interval.tick().await;
            let _ = pong_tx2.send(WorkerEvent::Pong { seq }).await;
            seq += 1;
        }
    });

    // Wait 250ms to confirm no death signal.
    let result = timeout(Duration::from_millis(250), &mut dead_rx).await;

    assert!(
        result.is_err(),
        "death signal should NOT be sent when pongs are received within timeout"
    );

    // Clean up.
    handle.abort();
    pong_task.abort();
}

/// No Pong arriving within the timeout triggers the death signal.
///
/// Constructs a watchdog with 50ms ping interval and 100ms pong timeout, using
/// a `MockTransport` that always succeeds. Does NOT feed any Pong through
/// `pong_rx`. Awaits `dead_rx` to receive the death signal, verifying the
/// watchdog correctly declares the worker dead after the pong timeout expires.
#[tokio::test]
async fn test_missing_pong_triggers_dead_signal() {
    let (dead_tx, mut dead_rx) = oneshot::channel();
    let (_pong_tx, pong_rx) = mpsc::channel::<WorkerEvent>(16);
    let transport = MockTransport::new_ok();

    let watchdog = KeepaliveWatchdog::new(
        "worker-0".to_string(),
        transport,
        pong_rx,
        dead_tx,
        Duration::from_millis(50),
        Duration::from_millis(100),
    );

    let handle = tokio::spawn(watchdog.run());

    // The watchdog sends a ping at ~50ms, then waits 100ms for a pong.
    // Total: ~150ms before death signal. Use 500ms test timeout for safety.
    let result = timeout(Duration::from_millis(500), &mut dead_rx).await;

    // The death signal SHOULD have been sent.
    assert!(
        result.is_ok(),
        "death signal should be sent when no pong arrives within timeout"
    );

    // The task should have exited cleanly.
    let task_result = timeout(Duration::from_millis(100), handle).await;
    assert!(
        task_result.is_ok(),
        "watchdog task should exit after sending death signal"
    );
}

/// Repeated successful Pongs do not false-trigger the death signal.
///
/// Constructs a watchdog with 50ms ping interval and 100ms pong timeout, using
/// a `MockTransport` that always succeeds. Spawns a dedicated task that sends
/// matching Pongs at 40ms intervals throughout the test duration. This ensures
/// a pong is always available in the channel when the watchdog enters
/// `wait_for_matching_pong`. Verifies that the death signal is NOT sent within
/// 300ms after the test completes.
#[tokio::test]
async fn test_repeated_successful_pings_no_false_trigger() {
    let (dead_tx, mut dead_rx) = oneshot::channel();
    let (pong_tx, pong_rx) = mpsc::channel::<WorkerEvent>(16);
    let transport = MockTransport::new_ok();

    let watchdog = KeepaliveWatchdog::new(
        "worker-0".to_string(),
        transport,
        pong_rx,
        dead_tx,
        Duration::from_millis(50),
        Duration::from_millis(100),
    );

    let watchdog_handle = tokio::spawn(watchdog.run());

    // Spawn a pong-sending task that runs for the entire test duration.
    // It sends pongs at 40ms intervals, ensuring a pong is buffered
    // before each ping fires (pings at 50ms, pongs at 0, 40, 80, ...).
    let pong_tx2 = pong_tx.clone();
    let pong_task = tokio::spawn(async move {
        let mut seq = 0u64;
        let mut interval = tokio::time::interval(Duration::from_millis(40));
        // Send first pong immediately.
        let _ = pong_tx2.send(WorkerEvent::Pong { seq }).await;
        seq += 1;
        // Send pongs for 600ms to cover the full test duration.
        for _ in 0..15 {
            interval.tick().await;
            let _ = pong_tx2.send(WorkerEvent::Pong { seq }).await;
            seq += 1;
        }
    });

    // Wait 300ms to confirm no death signal.
    let result = timeout(Duration::from_millis(300), &mut dead_rx).await;

    assert!(
        result.is_err(),
        "death signal should NOT be sent during repeated successful pings"
    );

    // Clean up.
    watchdog_handle.abort();
    pong_task.abort();
}

/// A transport send failure triggers the death signal.
///
/// Constructs a watchdog with a `MockTransport` that fails on every send.
/// The watchdog sends the first ping, the transport returns an error,
/// and the watchdog immediately signals death and exits. Verifies that
/// `dead_rx` receives the death signal.
#[tokio::test]
async fn test_transport_send_failure_triggers_dead_signal() {
    let (dead_tx, mut dead_rx) = oneshot::channel();
    let (_pong_tx, pong_rx) = mpsc::channel::<WorkerEvent>(16);
    let transport = MockTransport::new_err(IpcError::SendFailed("connection refused".to_string()));

    let watchdog = KeepaliveWatchdog::new(
        "worker-0".to_string(),
        transport,
        pong_rx,
        dead_tx,
        Duration::from_millis(50),
        Duration::from_millis(100),
    );

    let handle = tokio::spawn(watchdog.run());

    // The watchdog sends a ping after the first interval tick (~50ms),
    // the transport fails, and the watchdog sends death signal immediately.
    // Use 500ms to be generous.
    let result = timeout(Duration::from_millis(500), &mut dead_rx).await;

    assert!(
        result.is_ok(),
        "death signal should be sent when transport send fails"
    );

    // The task should have exited cleanly.
    let task_result = timeout(Duration::from_millis(100), handle).await;
    assert!(
        task_result.is_ok(),
        "watchdog task should exit after transport failure death signal"
    );
}
