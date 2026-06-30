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

// ---------------------------------------------------------------------------
// WorkerEvent msgpack roundtrip tests
// ---------------------------------------------------------------------------

use anvilml_core::NodeTypeDescriptor;
use anvilml_ipc::messages::WorkerEvent;

/// `WorkerEvent::Ready` with all 13 fields roundtrips via rmp-serde.
///
/// Constructs a realistic Ready event with representative GPU capability
/// values, two registered node types, and verifies the deserialised event
/// is byte-for-byte equal to the original. The msgpack dict contains
/// `"_type": "Ready"` plus all 13 field keys.
#[test]
fn test_ready_roundtrip() {
    let event = WorkerEvent::Ready {
        worker_id: "gpu:0".to_string(),
        device_index: 0,
        device_name: "NVIDIA RTX 4090".to_string(),
        device_type: "cuda".to_string(),
        vram_total_mib: 24576,
        vram_free_mib: 20480,
        torch_version: "2.5.1+cu124".to_string(),
        fp16: true,
        bf16: true,
        fp8: true,
        flash_attention: true,
        capabilities_source: "pytorch".to_string(),
        node_types: vec![
            NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Checkpoint".to_string(),
                category: "loaders".to_string(),
                description: "Loads a model checkpoint from disk.".to_string(),
                inputs: vec![],
                outputs: vec![],
            },
            NodeTypeDescriptor {
                type_name: "KSampler".to_string(),
                display_name: "K-Sampler".to_string(),
                category: "sampling".to_string(),
                description: "Samples from a latent space using a diffusion model.".to_string(),
                inputs: vec![],
                outputs: vec![],
            },
        ],
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Ready");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Ready");

    assert_eq!(
        event, decoded,
        "Ready roundtrip must preserve all 13 fields"
    );
}

/// `WorkerEvent::Pong { seq: 42 }` roundtrips via rmp-serde.
/// The msgpack dict contains `"_type": "Pong"` and `"seq": 42`.
#[test]
fn test_pong_roundtrip() {
    let event = WorkerEvent::Pong { seq: 42 };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Pong");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Pong");

    assert_eq!(event, decoded, "Pong roundtrip must preserve seq");
}

/// `WorkerEvent::Dying { reason: "OOM" }` roundtrips via rmp-serde.
/// The msgpack dict contains `"_type": "Dying"` and `"reason": "OOM"`.
#[test]
fn test_dying_roundtrip() {
    let event = WorkerEvent::Dying {
        reason: "OOM".to_string(),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Dying");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Dying");

    assert_eq!(event, decoded, "Dying roundtrip must preserve reason");
}

/// `WorkerEvent::MemoryReport { vram_used_mib: 4096, ram_used_mib: 8589934592 }`
/// roundtrips via rmp-serde. The msgpack dict contains `"_type": "MemoryReport"`
/// plus the two memory fields.
#[test]
fn test_memory_report_roundtrip() {
    let event = WorkerEvent::MemoryReport {
        vram_used_mib: 4096,
        ram_used_mib: 8589934592,
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize MemoryReport");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize MemoryReport");

    assert_eq!(
        event, decoded,
        "MemoryReport roundtrip must preserve vram_used_mib and ram_used_mib"
    );
}

/// `WorkerEvent::Progress { job_id, step: 3, total_steps: 20, preview_b64: Some(...) }`
/// roundtrips via rmp-serde. All four fields (`job_id`, `step`, `total_steps`,
/// `preview_b64`) are preserved with correct types. The msgpack dict contains
/// `"_type": "Progress"` plus all field keys.
#[test]
fn test_progress_roundtrip() {
    let event = WorkerEvent::Progress {
        job_id: Uuid::new_v4(),
        step: 3,
        total_steps: 20,
        preview_b64: Some("iVBORw0KGgo...".into()),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Progress");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Progress");

    assert_eq!(
        event, decoded,
        "Progress roundtrip must preserve all four fields"
    );
}

/// `WorkerEvent::ImageReady { job_id, image_b64, width: 512, height: 512,
/// format: "png", seed: 42, steps: 20 }` roundtrips via rmp-serde. All seven
/// fields (`job_id`, `image_b64`, `width`, `height`, `format`, `seed`, `steps`)
/// are preserved with correct types. The msgpack dict contains `"_type":
/// "ImageReady"` plus all field keys.
#[test]
fn test_image_ready_roundtrip() {
    let event = WorkerEvent::ImageReady {
        job_id: Uuid::new_v4(),
        image_b64: "iVBORw0KGgo...".into(),
        width: 512,
        height: 512,
        format: "png".into(),
        seed: 42,
        steps: 20,
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize ImageReady");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize ImageReady");

    assert_eq!(
        event, decoded,
        "ImageReady roundtrip must preserve all seven fields"
    );
}

/// `WorkerEvent::Completed { job_id, elapsed_ms: 5432 }` roundtrips via
/// rmp-serde. The msgpack dict contains `"_type": "Completed"` plus the
/// `job_id` and `elapsed_ms` fields.
#[test]
fn test_completed_roundtrip() {
    let event = WorkerEvent::Completed {
        job_id: Uuid::new_v4(),
        elapsed_ms: 5432,
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Completed");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Completed");

    assert_eq!(
        event, decoded,
        "Completed roundtrip must preserve job_id and elapsed_ms"
    );
}

/// `WorkerEvent::Failed { job_id, error: "CUDA out of memory",
/// traceback: Some("Traceback...") }` roundtrips via rmp-serde. All three
/// fields (`job_id`, `error`, `traceback`) are preserved with correct types.
/// The msgpack dict contains `"_type": "Failed"` plus all field keys.
#[test]
fn test_failed_roundtrip() {
    let event = WorkerEvent::Failed {
        job_id: Uuid::new_v4(),
        error: "CUDA out of memory".into(),
        traceback: Some("Traceback...".into()),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Failed");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Failed");

    assert_eq!(
        event, decoded,
        "Failed roundtrip must preserve job_id, error, and traceback"
    );
}

/// `WorkerEvent::Cancelled { job_id }` roundtrips via rmp-serde. The
/// msgpack dict contains `"_type": "Cancelled"` and the `job_id` field.
#[test]
fn test_cancelled_roundtrip() {
    let event = WorkerEvent::Cancelled {
        job_id: Uuid::new_v4(),
    };

    let bytes = rmp_serde::to_vec_named(&event).expect("serialize Cancelled");
    let decoded: WorkerEvent = rmp_serde::from_slice(&bytes).expect("deserialize Cancelled");

    assert_eq!(event, decoded, "Cancelled roundtrip must preserve job_id");
}
