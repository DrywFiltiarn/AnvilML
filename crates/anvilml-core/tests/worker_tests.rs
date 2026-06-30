//! Tests for `WorkerStatus`, `WorkerInfo`, `EnvReport`, `ProvisioningState`,
//! and `NodeTypeDescriptor` serde roundtrips.
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
        (ProvisioningState::Provisioning, "provisioning"),
        (ProvisioningState::Ready, "ready"),
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

/// An `EnvReport` with all 7 fields set serialises to JSON and roundtrips
/// back to an equal value. The JSON payload is also parsed to verify all
/// seven field names (`python_path`, `python_version`, `torch_version`,
/// `provisioning`, `preflight_ok`, `reason`, `node_types`) appear with
/// the correct types.
#[test]
fn test_env_report_serde_roundtrip() {
    let report = EnvReport {
        python_path: Some("/usr/bin/python3".to_string()),
        python_version: Some("3.12.3".to_string()),
        torch_version: Some("2.5.1".to_string()),
        provisioning: ProvisioningState::NotStarted,
        preflight_ok: true,
        reason: None,
        node_types: vec![NodeTypeDescriptor {
            type_name: "LoadModel".to_string(),
            display_name: "Load Model".to_string(),
            category: "loaders".to_string(),
            description: "Loads a model checkpoint.".to_string(),
            inputs: vec![],
            outputs: vec![],
        }],
    };

    let json = serde_json::to_string(&report).expect("failed to serialise EnvReport");
    let roundtripped: EnvReport =
        serde_json::from_str(&json).expect("failed to deserialise EnvReport");

    assert_eq!(
        report, roundtripped,
        "roundtripped EnvReport does not equal original"
    );

    // Verify the JSON contains all seven expected field names with correct types.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert!(parsed["python_path"].is_string());
    assert_eq!(parsed["python_path"], "/usr/bin/python3");
    assert!(parsed["python_version"].is_string());
    assert_eq!(parsed["python_version"], "3.12.3");
    assert!(parsed["torch_version"].is_string());
    assert_eq!(parsed["torch_version"], "2.5.1");
    assert!(parsed["provisioning"].is_string());
    assert_eq!(parsed["provisioning"], "not_started");
    assert!(parsed["preflight_ok"].is_boolean());
    assert_eq!(parsed["preflight_ok"], true);
    assert!(parsed["reason"].is_null());
    assert!(parsed["node_types"].is_array());
    assert_eq!(parsed["node_types"].as_array().unwrap().len(), 1);
}
