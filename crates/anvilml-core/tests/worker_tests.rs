//! Tests for `WorkerStatus`, `WorkerInfo`, `EnvReport`, and `ProvisioningState`
//! serde roundtrips.
//!
//! All tests construct types via the public API, serialise to JSON,
//! deserialise back, and assert equality. No I/O or env vars are used.

use anvilml_core::types::*;
use uuid::Uuid;

/// A `WorkerInfo` with all fields populated serialises to JSON and roundtrips
/// back to an equal value. The JSON payload is also parsed to verify all six
/// field names (`worker_id`, `status`, `device_index`, `device_type`, `pid`,
/// `current_job_id`) appear with the correct types.
#[test]
fn test_worker_info_construction_and_serde_roundtrip() {
    let job_id = Uuid::new_v4();

    let info = WorkerInfo {
        worker_id: "gpu:0".to_string(),
        status: WorkerStatus::Idle,
        device_index: 0,
        device_type: DeviceType::Cuda,
        pid: Some(1234),
        current_job_id: Some(job_id),
    };

    let json = serde_json::to_string(&info).expect("failed to serialise WorkerInfo");
    let roundtripped: WorkerInfo =
        serde_json::from_str(&json).expect("failed to deserialise WorkerInfo");

    assert_eq!(
        info, roundtripped,
        "roundtripped WorkerInfo does not equal original"
    );

    // Verify the JSON contains the expected field names.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["worker_id"], "gpu:0");
    assert_eq!(parsed["status"], "idle");
    assert_eq!(parsed["device_index"], 0);
    assert_eq!(parsed["device_type"], "cuda");
    assert_eq!(parsed["pid"], 1234);
    assert_eq!(parsed["current_job_id"], job_id.to_string());
}

/// Each of the five `WorkerStatus` variants serialises to the correct
/// `snake_case` JSON string and roundtrips back to an equal value.
#[test]
fn test_worker_status_serde_snake_case() {
    let variants: [(WorkerStatus, &str); 5] = [
        (WorkerStatus::Spawning, "spawning"),
        (WorkerStatus::Idle, "idle"),
        (WorkerStatus::Busy, "busy"),
        (WorkerStatus::Dying, "dying"),
        (WorkerStatus::Dead, "dead"),
    ];

    for (status, expected_json) in variants {
        let json = serde_json::to_string(&status).expect("failed to serialise WorkerStatus");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "WorkerStatus::{:?} JSON mismatch",
            status
        );

        let roundtripped: WorkerStatus =
            serde_json::from_str(&json).expect("failed to deserialise WorkerStatus");
        assert_eq!(
            status, roundtripped,
            "WorkerStatus::{:?} roundtrip mismatch",
            status
        );
    }
}

/// Each of the four `ProvisioningState` variants serialises to the correct
/// `snake_case` JSON string and roundtrips back to an equal value.
#[test]
fn test_provisioning_state_serde_snake_case() {
    let variants: [(ProvisioningState, &str); 4] = [
        (ProvisioningState::NotStarted, "not_started"),
        (ProvisioningState::InProgress, "in_progress"),
        (ProvisioningState::Complete, "complete"),
        (ProvisioningState::Failed, "failed"),
    ];

    for (state, expected_json) in variants {
        let json = serde_json::to_string(&state).expect("failed to serialise ProvisioningState");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "ProvisioningState::{:?} JSON mismatch",
            state
        );

        let roundtripped: ProvisioningState =
            serde_json::from_str(&json).expect("failed to deserialise ProvisioningState");
        assert_eq!(
            state, roundtripped,
            "ProvisioningState::{:?} roundtrip mismatch",
            state
        );
    }
}

/// An `EnvReport` with all fields set serialises to JSON and roundtrips
/// back to an equal value. The JSON payload is also parsed to verify all
/// three field names (`python_version`, `torch_version`, `torch_importable`)
/// appear with the correct types.
#[test]
fn test_env_report_serde_roundtrip() {
    let report = EnvReport {
        python_version: "3.12.3".to_string(),
        torch_version: Some("2.5.1".to_string()),
        torch_importable: true,
    };

    let json = serde_json::to_string(&report).expect("failed to serialise EnvReport");
    let roundtripped: EnvReport =
        serde_json::from_str(&json).expect("failed to deserialise EnvReport");

    assert_eq!(
        report, roundtripped,
        "roundtripped EnvReport does not equal original"
    );

    // Verify the JSON contains the expected field names.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["python_version"], "3.12.3");
    assert_eq!(parsed["torch_version"], "2.5.1");
    assert_eq!(parsed["torch_importable"], true);
}
