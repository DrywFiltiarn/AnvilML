/// Tests for `types::job` — `Job`, `JobStatus`, `JobSettings`,
/// `SubmitJobRequest`, and `SubmitJobResponse`.
///
/// Verifies:
/// - JSON roundtrip for a fully-populated `Job`.
/// - `JobSettings::default()` produces `device_preference: None`.
/// - All five `JobStatus` variants roundtrip through JSON.
use anvilml_core::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};
use chrono::Utc;
use uuid::Uuid;

/// Verifies that a fully-populated `Job` serialises to JSON and
/// deserialises back to an identical value, including `Option` fields
/// with mixed `Some`/`None` values and the nested `JobSettings` struct.
///
/// This is the primary acceptance test for the correctness of all
/// `Serialize`/`Deserialize` derives on `Job` and its fields.
#[test]
fn test_job_json_roundtrip() {
    let job = Job {
        id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        status: JobStatus::Running,
        graph: serde_json::json!({"nodes": []}),
        settings: JobSettings {
            device_preference: Some("cuda".to_string()),
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        worker_id: Some("worker-0".to_string()),
        error: None,
        queue_position: Some(1),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&job).expect("serialize Job to JSON string");

    // Deserialize back — must not fail
    let restored: Job = serde_json::from_str(&json).expect("deserialize JSON back to Job");

    // All fields must be equal
    assert_eq!(restored.id, job.id);
    assert_eq!(restored.status, job.status);
    assert_eq!(restored.graph, job.graph);
    assert_eq!(restored.settings, job.settings);
    assert_eq!(restored.created_at, job.created_at);
    assert_eq!(restored.started_at, job.started_at);
    assert_eq!(restored.completed_at, job.completed_at);
    assert_eq!(restored.worker_id, job.worker_id);
    assert_eq!(restored.error, job.error);
    assert_eq!(restored.queue_position, job.queue_position);
}

/// Verifies that `JobSettings::default()` produces `device_preference: None`,
/// matching the documented convention that `None` means auto-select by VRAM.
///
/// This is a minimal but important test — the default value is the most
/// commonly used value in practice, and correctness here ensures
/// the scheduler's auto-selection path is the default.
#[test]
fn test_job_settings_default() {
    let settings = JobSettings::default();
    assert!(
        settings.device_preference.is_none(),
        "default JobSettings should have device_preference = None"
    );
}

/// Verifies that all five `JobStatus` enum variants roundtrip through
/// JSON serialisation without data loss.
///
/// Each variant is serialised to a JSON string and deserialised back,
/// then compared for equality. This tests that the serde derives produce
/// correct field mappings for the enum.
#[test]
fn test_job_status_variants() {
    let variants = [
        JobStatus::Queued,
        JobStatus::Running,
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Cancelled,
    ];

    for variant in variants {
        let json = serde_json::to_string(&variant).expect("serialize JobStatus variant to JSON");

        let restored: JobStatus =
            serde_json::from_str(&json).expect("deserialize JSON back to JobStatus");

        assert_eq!(
            restored, variant,
            "JobStatus::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }
}

/// Verifies that `SubmitJobRequest::default()` produces a `graph` field
/// set to `serde_json::Value::Null` and `settings` with `device_preference: None`.
///
/// This ensures the default request is a well-formed submission that will
/// be accepted by the scheduler without requiring client-specified values.
#[test]
fn test_submit_job_request_default() {
    let req = SubmitJobRequest::default();
    assert!(
        req.graph.is_null(),
        "default SubmitJobRequest graph must be null"
    );
    assert!(
        req.settings.device_preference.is_none(),
        "default SubmitJobRequest settings must have device_preference = None"
    );
}

/// Verifies that `SubmitJobResponse::default()` produces `job_id` set to
/// the UUID zero value and `queue_position` set to `0`.
///
/// The default response is used as a placeholder before the scheduler
/// assigns real values — this test ensures the defaults are well-formed.
#[test]
fn test_submit_job_response_default() {
    let resp = SubmitJobResponse::default();
    assert!(
        resp.job_id == Uuid::default(),
        "default SubmitJobResponse job_id must be the UUID zero value"
    );
    assert_eq!(resp.queue_position, 0);
}
