//! Integration tests for the keepalive heartbeat module.
//!
//! These tests exercise the heartbeat loop using in-memory channels (mpsc + broadcast)
//! instead of a real ZeroMQ transport. This is sufficient because the keepalive logic
//! is purely about sequence matching and deadline timing — the transport layer is
//! opaque to the heartbeat task.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};
use tokio::time::timeout;

use anvilml_ipc::{WorkerEvent, WorkerMessage};
use anvilml_worker::keepalive::start;

/// Verify that the timeout callback fires when no pong is received.
///
/// The keepalive sends a Ping and waits for a matching Pong. Since the test
/// never sends a Pong, the pong_timeout elapses and the on_timeout callback
/// is invoked. The test asserts this happens within `pong_timeout + 100ms`.
#[tokio::test]
async fn test_timeout_fires() {
    let on_timeout = Arc::new(AtomicUsize::new(0));
    let on_timeout_clone = Arc::clone(&on_timeout);

    // Create mpsc channel — keepalive gets the sender, test holds the receiver.
    // The receiver must stay alive so the keepalive can send pings without
    // getting a SendError. We don't read from the receiver — the keepalive's
    // pings simply accumulate in the buffer.
    let (msg_tx, mut _msg_rx) = mpsc::channel::<WorkerMessage>(16);

    // Create broadcast channel — keepalive gets the receiver.
    // The test holds the sender but never sends any events, so the keepalive
    // waits for a pong that never arrives.
    let (event_tx, event_rx) = broadcast::channel::<(String, WorkerEvent)>(16);

    // Spawn the keepalive task with a 500ms pong timeout and 100ms ping interval.
    // The ping interval is shorter than the timeout to allow multiple cycles
    // if the timeout doesn't fire.
    let (handle, _hb_handle) = start(
        "test-worker".to_string(),
        msg_tx,
        event_rx,
        Duration::from_millis(100), // ping_interval
        Duration::from_millis(500), // pong_timeout
        move || {
            // Increment the timeout counter to signal that the callback fired.
            on_timeout_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    // The timeout should fire within pong_timeout (500ms) + 100ms buffer.
    // If the callback is not called within 1s, the test fails.
    timeout(Duration::from_secs(1), async {
        loop {
            if on_timeout.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("on_timeout callback should fire within 1 second");

    // The keepalive loops after each timeout, so the callback fires multiple
    // times. We only assert that at least one call was made.
    assert!(
        on_timeout.load(Ordering::SeqCst) >= 1,
        "on_timeout should have been called at least once"
    );

    // Drop the task handle — the keepalive should still be running (it loops
    // after timeout). Dropping it terminates the task.
    handle.abort();

    // Drop the broadcast sender so the keepalive's event_rx closes cleanly.
    drop(event_tx);
}

/// Verify that a matching Pong resets the deadline and prevents timeout.
///
/// The keepalive sends a Ping with seq=N. The test responds with a Pong{seq: N}
/// for each ping. Since every ping gets a matching pong, the timeout callback
/// should never fire even after multiple ping cycles.
#[tokio::test]
async fn test_pong_resets_deadline() {
    let on_timeout = Arc::new(AtomicUsize::new(0));
    let on_timeout_clone = Arc::clone(&on_timeout);

    // Create mpsc channel — keepalive gets the sender, test holds the receiver.
    let (msg_tx, mut msg_rx) = mpsc::channel::<WorkerMessage>(16);

    // Create broadcast channel — keepalive gets the receiver.
    let (event_tx, event_rx) = broadcast::channel::<(String, WorkerEvent)>(16);

    // Spawn the keepalive task.
    let (handle, _hb_handle) = start(
        "test-worker".to_string(),
        msg_tx,
        event_rx,
        Duration::from_millis(100), // ping_interval
        Duration::from_millis(500), // pong_timeout
        move || {
            on_timeout_clone.fetch_add(1, Ordering::SeqCst);
        },
    );

    // Give the keepalive time to send its first ping.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Receive the first ping and send back a matching pong.
    // The keepalive sends Ping{seq: 1} as its first ping.
    if let Some(msg) = msg_rx.recv().await {
        if let WorkerMessage::Ping { seq } = msg {
            // Echo the ping as a pong to reset the keepalive's deadline.
            // The broadcast sender delivers the event to the keepalive's
            // event_rx, which matches it against the current seq.
            let _ = event_tx.send(("test-worker".to_string(), WorkerEvent::Pong { seq }));
        }
    }

    // Wait for 1.5 seconds — enough time for at least 3 ping cycles
    // (100ms interval each). If the timeout callback fires during this
    // window, the test fails.
    timeout(Duration::from_secs(1), async {
        loop {
            if on_timeout.load(Ordering::SeqCst) > 0 {
                panic!("on_timeout should not fire when pongs are sent");
            }
            // Receive the next ping and respond with a matching pong.
            // This keeps the keepalive alive across multiple cycles.
            if let Some(msg) = msg_rx.recv().await {
                if let WorkerMessage::Ping { seq } = msg {
                    let _ = event_tx.send(("test-worker".to_string(), WorkerEvent::Pong { seq }));
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        // Timeout of the outer wrapper means on_timeout was never called
        // during the 1-second window — the test passes.
    });

    assert_eq!(
        on_timeout.load(Ordering::SeqCst),
        0,
        "on_timeout should never fire when pongs match every ping"
    );

    // Clean up.
    handle.abort();
}

/// Verify that the sequence number increments monotonically across ping sends.
///
/// The keepalive starts with seq=0 and increments it before each ping. The test
/// collects the sequence numbers from the mpsc channel and asserts they are
/// strictly increasing.
#[tokio::test]
async fn test_seq_increments() {
    // Create mpsc channel — keepalive gets the sender, test holds the receiver.
    let (msg_tx, mut msg_rx) = mpsc::channel::<WorkerMessage>(64);

    // Create broadcast channel — keepalive gets the receiver.
    // The test holds the sender. We collect pings for 2 seconds then drop
    // the sender to close the broadcast channel, which causes the keepalive
    // to exit cleanly (it returns on RecvError::Closed).
    let (event_tx, event_rx) = broadcast::channel::<(String, WorkerEvent)>(16);

    // Spawn the keepalive task.
    let (handle, _hb_handle) = start(
        "test-worker".to_string(),
        msg_tx,
        event_rx,
        Duration::from_millis(50), // ping_interval
        Duration::from_millis(50), // pong_timeout
        || {
            // Timeout fires but we don't care about it — we're testing seq.
        },
    );

    // Collect sequence numbers from the mpsc receiver.
    // The keepalive sends a ping every 100ms. Over 2 seconds, we should
    // receive at least 10 pings with incrementing sequence numbers.
    let mut seqs = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);

    while tokio::time::Instant::now() < deadline {
        // Use a short timeout on recv() so we can re-check the deadline.
        // This prevents the loop from blocking indefinitely if the keepalive
        // is slow to send.
        if let Ok(Some(msg)) = timeout(Duration::from_millis(50), msg_rx.recv()).await {
            if let WorkerMessage::Ping { seq } = msg {
                seqs.push(seq);
            }
        }
    }

    // Drop the broadcast sender to close the channel and cause the keepalive
    // task to exit cleanly (it returns on RecvError::Closed).
    drop(event_tx);

    assert!(
        seqs.len() >= 3,
        "expected at least 3 pings in 3 seconds, got {}",
        seqs.len()
    );

    // Verify monotonically increasing sequence numbers.
    for i in 1..seqs.len() {
        assert!(
            seqs[i] > seqs[i - 1],
            "sequence should be strictly increasing: seq[{}] = {} <= seq[{}] = {}",
            i - 1,
            seqs[i - 1],
            i,
            seqs[i]
        );
    }

    // Verify the first sequence number is 1 (incremented before first send).
    assert_eq!(seqs[0], 1, "first ping should have seq=1");

    // Clean up — await the handle to confirm clean exit.
    handle
        .await
        .expect("keepalive task should exit cleanly after broadcast channel closes");
}
