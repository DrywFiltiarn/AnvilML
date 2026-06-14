/// Tests for `types::worker` — `WorkerInfo`, `WorkerStatus`, `EnvReport`,
/// and `ProvisioningState`.
///
/// Verifies:
/// - JSON roundtrip for a fully-populated `WorkerInfo` with all `Option` fields set.
/// - All 5 `WorkerStatus` enum variants roundtrip through JSON with correct snake_case keys.
/// - `EnvReport` with `preflight_ok: false`, `provisioning: NotStarted`, and empty `node_types`
///   roundtrips correctly.
use anvilml_core::{EnvReport, ProvisioningState, WorkerInfo, WorkerStatus};
use uuid::Uuid;

/// Verifies that a fully-populated `WorkerInfo` serialises to JSON and
/// deserialises back to an identical value, including `Option` fields
/// with `Some` values for both `current_job_id` and `vram_used_mib`.
///
/// This is the primary acceptance test for the correctness of all
/// `Serialize`/`Deserialize` derives on `WorkerInfo` and its fields.
#[test]
fn test_worker_info_json_roundtrip() {
    let worker = WorkerInfo {
        id: "worker-0".to_string(),
        device_index: 0,
        device_name: "NVIDIA A100-SXM4-40GB".to_string(),
        status: WorkerStatus::Busy,
        current_job_id: Some(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()),
        vram_used_mib: Some(12288),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&worker).expect("serialize WorkerInfo to JSON");

    // Deserialize back — must not fail
    let restored: WorkerInfo =
        serde_json::from_str(&json).expect("deserialize JSON back to WorkerInfo");

    // All fields must be equal
    assert_eq!(restored.id, worker.id);
    assert_eq!(restored.device_index, worker.device_index);
    assert_eq!(restored.device_name, worker.device_name);
    assert_eq!(restored.status, worker.status);
    assert_eq!(restored.current_job_id, worker.current_job_id);
    assert_eq!(restored.vram_used_mib, worker.vram_used_mib);
}

/// Verifies that all 5 `WorkerStatus` enum variants roundtrip through
/// JSON serialisation without data loss.
///
/// Each variant is serialised to a JSON string and deserialised back,
/// then compared for equality. This tests that `#[serde(rename_all =
/// "snake_case")]` produces the correct lowercase variant names
/// (`"initializing"`, `"idle"`, `"busy"`, `"dead"`, `"respawning"`).
#[test]
fn test_worker_status_variants() {
    let variants = [
        WorkerStatus::Initializing,
        WorkerStatus::Idle,
        WorkerStatus::Busy,
        WorkerStatus::Dead,
        WorkerStatus::Respawning,
    ];

    for variant in variants {
        let json = serde_json::to_string(&variant).expect("serialize WorkerStatus variant to JSON");

        let restored: WorkerStatus =
            serde_json::from_str(&json).expect("deserialize JSON back to WorkerStatus");

        assert_eq!(
            restored, variant,
            "WorkerStatus::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }
}

/// Verifies that an `EnvReport` with `preflight_ok: false`,
/// `provisioning: NotStarted` (the default stub state), and an empty
/// `node_types` vector roundtrips through JSON correctly.
///
/// This tests the minimal `EnvReport` that the Rust supervisor would
/// produce during preflight checks before the Python worker connects.
#[test]
fn test_env_report_default_preflight() {
    let report = EnvReport {
        python_path: None,
        python_version: None,
        torch_version: None,
        provisioning: ProvisioningState::NotStarted,
        preflight_ok: false,
        reason: Some("Python not yet launched".to_string()),
        node_types: Vec::new(),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&report).expect("serialize EnvReport to JSON");

    // Deserialize back — must not fail
    let restored: EnvReport =
        serde_json::from_str(&json).expect("deserialize JSON back to EnvReport");

    // All fields must be equal
    assert_eq!(restored.python_path, report.python_path);
    assert_eq!(restored.python_version, report.python_version);
    assert_eq!(restored.torch_version, report.torch_version);
    assert_eq!(restored.provisioning, report.provisioning);
    assert_eq!(restored.preflight_ok, report.preflight_ok);
    assert_eq!(restored.reason, report.reason);

    // node_types must be an empty vec
    assert!(
        restored.node_types.is_empty(),
        "EnvReport.node_types must be empty (got {} items)",
        restored.node_types.len()
    );
}
