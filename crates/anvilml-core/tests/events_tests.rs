//! Tests for `WsEvent` serde roundtrips — one test per variant.
//!
//! All tests construct the variant with concrete field values, serialise to JSON,
//! assert the `"type"` key equals the snake_case variant name, deserialise back,
//! and verify equality. No I/O or env vars are used.

use anvilml_core::types::WsEvent;
use uuid::Uuid;

/// A `WsEvent::JobQueued` variant serialises with `"type": "job_queued"`, all fields
/// roundtrip, and the tag key is `"type"` (not a variant-name key).
#[test]
fn test_ws_event_job_queued_serde_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobQueued {
        job_id,
        queue_position: 3,
    };

    let json = serde_json::to_string(&event).expect("failed to serialise JobQueued");

    // Verify the tag key is "type" and the tag value is "job_queued".
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type"], "job_queued");
    assert_eq!(parsed["job_id"], "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(parsed["queue_position"], 3);

    let roundtripped: WsEvent = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(
        event, roundtripped,
        "roundtripped JobQueued does not equal original"
    );
}

/// A `WsEvent::JobStarted` variant serialises with `"type": "job_started"`, all fields
/// roundtrip, and the tag key is `"type"`.
#[test]
fn test_ws_event_job_started_serde_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobStarted {
        job_id,
        worker_id: "gpu:0".to_string(),
    };

    let json = serde_json::to_string(&event).expect("failed to serialise JobStarted");

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type"], "job_started");
    assert_eq!(parsed["worker_id"], "gpu:0");

    let roundtripped: WsEvent = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(
        event, roundtripped,
        "roundtripped JobStarted does not equal original"
    );
}

/// A `WsEvent::JobProgress` variant with `preview_b64: None` serialises with
/// `"type": "job_progress"`, all fields roundtrip, and the tag key is `"type"`.
#[test]
fn test_ws_event_job_progress_serde_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobProgress {
        job_id,
        step: 3,
        total_steps: 20,
        preview_b64: None,
    };

    let json = serde_json::to_string(&event).expect("failed to serialise JobProgress");

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type"], "job_progress");
    assert_eq!(parsed["step"], 3);
    assert_eq!(parsed["total_steps"], 20);
    assert_eq!(parsed["preview_b64"], serde_json::Value::Null);

    let roundtripped: WsEvent = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(
        event, roundtripped,
        "roundtripped JobProgress does not equal original"
    );
}

/// A `WsEvent::JobImageReady` variant serialises with `"type": "job_image_ready"`,
/// all fields (including `seed: i64`) roundtrip, and the tag key is `"type"`.
#[test]
fn test_ws_event_job_image_ready_serde_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobImageReady {
        job_id,
        artifact_hash: "abc123def456".to_string(),
        width: 512,
        height: 512,
        seed: 42,
        steps: 20,
    };

    let json = serde_json::to_string(&event).expect("failed to serialise JobImageReady");

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type"], "job_image_ready");
    assert_eq!(parsed["seed"], 42);
    assert_eq!(parsed["steps"], 20);

    let roundtripped: WsEvent = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(
        event, roundtripped,
        "roundtripped JobImageReady does not equal original"
    );
}

/// A `WsEvent::JobCompleted` variant serialises with `"type": "job_completed"`,
/// `elapsed_ms: u64` roundtrips, and the tag key is `"type"`.
#[test]
fn test_ws_event_job_completed_serde_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobCompleted {
        job_id,
        elapsed_ms: 15000,
    };

    let json = serde_json::to_string(&event).expect("failed to serialise JobCompleted");

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type"], "job_completed");
    assert_eq!(parsed["elapsed_ms"], 15000);

    let roundtripped: WsEvent = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(
        event, roundtripped,
        "roundtripped JobCompleted does not equal original"
    );
}

/// A `WsEvent::JobFailed` variant serialises with `"type": "job_failed"`,
/// the error string roundtrips, and the tag key is `"type"`.
#[test]
fn test_ws_event_job_failed_serde_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobFailed {
        job_id,
        error: "CUDA out of memory".to_string(),
    };

    let json = serde_json::to_string(&event).expect("failed to serialise JobFailed");

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type"], "job_failed");
    assert_eq!(parsed["error"], "CUDA out of memory");

    let roundtripped: WsEvent = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(
        event, roundtripped,
        "roundtripped JobFailed does not equal original"
    );
}

/// A `WsEvent::JobCancelled` variant serialises with `"type": "job_cancelled"`,
/// the single `job_id` field roundtrips, and the tag key is `"type"`.
#[test]
fn test_ws_event_job_cancelled_serde_roundtrip() {
    let job_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let event = WsEvent::JobCancelled { job_id };

    let json = serde_json::to_string(&event).expect("failed to serialise JobCancelled");

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["type"], "job_cancelled");

    let roundtripped: WsEvent = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(
        event, roundtripped,
        "roundtripped JobCancelled does not equal original"
    );
}
