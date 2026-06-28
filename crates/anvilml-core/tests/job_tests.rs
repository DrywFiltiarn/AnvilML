//! Tests for `Job`, `JobStatus`, and `JobSettings` serde roundtrips.
//!
//! All tests construct types via the public API, serialise to JSON,
//! deserialise back, and assert equality. No I/O or env vars are used.

use anvilml_core::types::*;
use chrono::Utc;
use uuid::Uuid;

/// A `Job` with all fields populated serialises and deserialises back to an equal value.
#[test]
fn test_job_serde_roundtrip() {
    let job = Job {
        id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        status: JobStatus::Queued,
        graph: serde_json::json!({"nodes": [{"op": "load"}]}),
        settings: JobSettings {
            device_preference: Some("cuda".to_string()),
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        worker_id: Some("gpu:0".to_string()),
        error: Some("some error".to_string()),
        queue_position: Some(42),
    };

    let json = serde_json::to_string(&job).expect("failed to serialise Job");
    let roundtripped: Job = serde_json::from_str(&json).expect("failed to deserialise Job");

    assert_eq!(
        job, roundtripped,
        "roundtripped Job does not equal original"
    );

    // Also verify the JSON contains the expected fields.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["id"], "550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(parsed["status"], "queued");
    assert_eq!(parsed["settings"]["device_preference"], "cuda");
}

/// Each of the five `JobStatus` variants roundtrips correctly through serde JSON.
#[test]
fn test_job_status_all_variants_roundtrip() {
    let variants = [
        (JobStatus::Queued, "queued"),
        (JobStatus::Running, "running"),
        (JobStatus::Completed, "completed"),
        (JobStatus::Failed, "failed"),
        (JobStatus::Cancelled, "cancelled"),
    ];

    for (status, expected_json) in variants {
        let json = serde_json::to_string(&status).expect("failed to serialise");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "JobStatus::{:?} JSON mismatch",
            status
        );

        let roundtripped: JobStatus = serde_json::from_str(&json).expect("failed to deserialise");
        assert_eq!(
            status, roundtripped,
            "JobStatus::{:?} roundtrip mismatch",
            status
        );
    }
}

/// A `JobSettings` with `device_preference: None` serialises to `"device_preference": null`.
#[test]
fn test_job_settings_default() {
    let settings = JobSettings {
        device_preference: None,
    };

    let json = serde_json::to_string(&settings).expect("failed to serialise");
    assert!(
        json.contains(r#""device_preference": null"#)
            || json.contains(r#""device_preference":null"#),
        "JSON should contain null for device_preference, got: {json}",
    );

    let roundtripped: JobSettings = serde_json::from_str(&json).expect("failed to deserialise");
    assert_eq!(settings, roundtripped);
}

/// A `Job` with all `Option` fields set to `None` roundtrips correctly.
#[test]
fn test_job_with_nulls_roundtrip() {
    let job = Job {
        id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        status: JobStatus::Queued,
        graph: serde_json::json!({"nodes": []}),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        worker_id: None,
        error: None,
        queue_position: None,
    };

    let json = serde_json::to_string(&job).expect("failed to serialise Job");
    let roundtripped: Job = serde_json::from_str(&json).expect("failed to deserialise Job");

    assert_eq!(
        job, roundtripped,
        "roundtripped Job does not equal original"
    );

    // Verify None fields are still None after roundtrip.
    assert!(
        roundtripped.started_at.is_none(),
        "started_at should be None"
    );
    assert!(
        roundtripped.completed_at.is_none(),
        "completed_at should be None"
    );
    assert!(roundtripped.worker_id.is_none(), "worker_id should be None");
    assert!(roundtripped.error.is_none(), "error should be None");
    assert!(
        roundtripped.queue_position.is_none(),
        "queue_position should be None"
    );
}
