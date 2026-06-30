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

// ---------------------------------------------------------------------------
// WorkerMessage msgpack roundtrip tests
// ---------------------------------------------------------------------------

use anvilml_core::JobSettings;
use anvilml_ipc::messages::WorkerMessage;

/// `WorkerMessage::Ping { seq: 42 }` serialises via rmp-serde and roundtrips
/// to an equal value. The msgpack dict contains `"_type": "Ping"` and
/// `"seq": 42`.
#[test]
fn test_ping_roundtrip() {
    let msg = WorkerMessage::Ping { seq: 42 };

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize Ping");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize Ping");

    assert_eq!(msg, decoded, "Ping roundtrip must preserve seq");
}

/// `WorkerMessage::Shutdown` (unit variant, no fields) roundtrips via
/// rmp-serde. The msgpack dict contains only `"_type": "Shutdown"`.
#[test]
fn test_shutdown_roundtrip() {
    let msg = WorkerMessage::Shutdown;

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize Shutdown");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize Shutdown");

    assert_eq!(msg, decoded, "Shutdown roundtrip must be identity");
}

/// `WorkerMessage::Execute { job_id, graph, settings, device_index }` roundtrips
/// via rmp-serde. All four fields (`job_id`, `graph`, `settings`, `device_index`)
/// are preserved with correct types (Uuid→string, Value→dict, JobSettings→dict,
/// u32→int).
#[test]
fn test_execute_roundtrip() {
    let msg = WorkerMessage::Execute {
        job_id: Uuid::new_v4(),
        graph: serde_json::json!({}),
        settings: JobSettings {
            device_preference: None,
        },
        device_index: 0,
    };

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize Execute");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize Execute");

    assert_eq!(
        msg, decoded,
        "Execute roundtrip must preserve all four fields"
    );
}

/// `WorkerMessage::CancelJob { job_id }` roundtrips via rmp-serde. The
/// `job_id` field is preserved correctly across serialisation.
#[test]
fn test_cancel_job_roundtrip() {
    let msg = WorkerMessage::CancelJob {
        job_id: Uuid::new_v4(),
    };

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize CancelJob");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize CancelJob");

    assert_eq!(msg, decoded, "CancelJob roundtrip must preserve job_id");
}

/// `WorkerMessage::MemoryQuery` (unit variant, no fields) roundtrips via
/// rmp-serde. The msgpack dict contains only `"_type": "MemoryQuery"`.
#[test]
fn test_memory_query_roundtrip() {
    let msg = WorkerMessage::MemoryQuery;

    let bytes = rmp_serde::to_vec_named(&msg).expect("serialize MemoryQuery");
    let decoded: WorkerMessage = rmp_serde::from_slice(&bytes).expect("deserialize MemoryQuery");

    assert_eq!(msg, decoded, "MemoryQuery roundtrip must be identity");
}
