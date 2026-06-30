//! Integration tests for `EventBroadcaster` — publish/subscribe behaviour
//! of the tokio::sync::broadcast wrapper.
//!
//! All tests use `#[tokio::test]` for async support. No env vars, files, or
//! I/O are used; each test constructs its own `EventBroadcaster` instance.

use anvilml_core::WsEvent;
use anvilml_ipc::EventBroadcaster;
use uuid::Uuid;

/// Publishing an event with zero subscribers does not panic — the internal
/// `send()` returns `Err(SendError)` which `publish()` silently discards.
#[tokio::test]
async fn test_publish_zero_subscribers() {
    let broadcaster = EventBroadcaster::new();
    let event = WsEvent::JobQueued {
        job_id: Uuid::new_v4(),
        queue_position: 1,
    };

    // publish() with zero subscribers: must not panic; SendError is ignored.
    broadcaster.publish(event);
}

/// Publishing an event with one subscriber delivers the event to that subscriber.
#[tokio::test]
async fn test_publish_one_subscriber_delivers() {
    let broadcaster = EventBroadcaster::new();
    let mut receiver = broadcaster.subscribe();
    let expected = WsEvent::JobStarted {
        job_id: Uuid::new_v4(),
        worker_id: "gpu:0".to_string(),
    };

    broadcaster.publish(expected.clone());
    let received = receiver
        .recv()
        .await
        .expect("receiver should deliver the event");

    assert_eq!(
        expected, received,
        "received event does not match published event"
    );
}

/// Publishing one event to multiple subscribers gives each subscriber an
/// independent copy of the event.
#[tokio::test]
async fn test_publish_multiple_subscribers_independent_copies() {
    let broadcaster = EventBroadcaster::new();
    let mut rx1 = broadcaster.subscribe();
    let mut rx2 = broadcaster.subscribe();
    let expected = WsEvent::JobCompleted {
        job_id: Uuid::new_v4(),
        elapsed_ms: 42,
    };

    broadcaster.publish(expected.clone());

    let from_rx1 = rx1.recv().await.expect("rx1 should receive the event");
    let from_rx2 = rx2.recv().await.expect("rx2 should receive the event");

    assert_eq!(
        expected, from_rx1,
        "rx1 received event does not match published event"
    );
    assert_eq!(
        expected, from_rx2,
        "rx2 received event does not match published event"
    );
}

/// subscribe() returns a receiver that is valid — calling recv().await does
/// not immediately return `RecvError::Closed` before any publish occurs.
#[tokio::test]
async fn test_subscribe_returns_valid_receiver() {
    let broadcaster = EventBroadcaster::new();
    let mut receiver = broadcaster.subscribe();

    // Wait with a timeout to confirm the receiver is open (not closed).
    // A timeout elapsing means recv() is still blocked waiting for events,
    // which proves the channel is open. If the channel were closed, recv()
    // would return Err(RecvError::Closed) immediately, completing within
    // the timeout window.
    let result = tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv()).await;

    // Err(Elapsed) means recv() was still blocked when the timeout fired —
    // the channel is open and waiting for events. This is the expected state.
    assert!(
        result.is_err(),
        "recv() should still be blocked (channel open), not closed; got {:?}",
        result
    );
}
