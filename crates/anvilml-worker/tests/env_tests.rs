//! Integration tests for `WorkerEnv::build()` — verifies the environment variable
//! map that the supervisor injects into every Python worker subprocess.
//!
//! All tests use `HashMap` lookups to verify individual keys; no env vars are
//! mutated, so no `#[serial]` is needed.

use anvilml_core::DeviceType;
use anvilml_worker::WorkerEnv;

/// All six builder-set env vars are present with correct string values.
///
/// Verifies the complete happy-path: every parameter maps to the correct key
/// and its string representation matches the expected value exactly.
#[test]
fn test_build_all_vars_present() {
    let map = WorkerEnv::build(5555, "0", 1, DeviceType::Cuda, false, "debug", 512);

    assert_eq!(map.get("ANVILML_IPC_PORT"), Some(&"5555".to_string()));
    assert_eq!(map.get("ANVILML_WORKER_ID"), Some(&"0".to_string()));
    assert_eq!(map.get("ANVILML_DEVICE_INDEX"), Some(&"1".to_string()));
    assert_eq!(map.get("ANVILML_DEVICE_TYPE"), Some(&"cuda".to_string()));
    assert_eq!(map.get("ANVILML_LOG_LEVEL"), Some(&"debug".to_string()));
    assert_eq!(
        map.get("ANVILML_MAX_IPC_PAYLOAD_MIB"),
        Some(&"512".to_string())
    );
}

/// `ANVILML_WORKER_MOCK` is absent from the map when `mock = false`.
///
/// Its absence signals real-mode hardware execution to the Python worker.
#[test]
fn test_worker_mock_absent_when_false() {
    let map = WorkerEnv::build(5555, "0", 0, DeviceType::Cpu, false, "info", 256);

    assert!(
        !map.contains_key("ANVILML_WORKER_MOCK"),
        "ANVILML_WORKER_MOCK should be absent when mock=false"
    );
}

/// `ANVILML_WORKER_MOCK = "1"` when `mock = true`.
///
/// This is the primary mechanism by which the supervisor tells the Python
/// worker to use mock hardware instead of real torch-level probing.
#[test]
fn test_worker_mock_present_when_true() {
    let map = WorkerEnv::build(5555, "0", 0, DeviceType::Cpu, true, "info", 256);

    assert_eq!(
        map.get("ANVILML_WORKER_MOCK"),
        Some(&"1".to_string()),
        "ANVILML_WORKER_MOCK must be \"1\" when mock=true"
    );
}

/// `DeviceType::Cuda` maps to `"cuda"` in `ANVILML_DEVICE_TYPE`.
#[test]
fn test_device_type_cuda() {
    let map = WorkerEnv::build(5555, "0", 0, DeviceType::Cuda, false, "info", 256);

    assert_eq!(map.get("ANVILML_DEVICE_TYPE"), Some(&"cuda".to_string()));
}

/// `DeviceType::Rocm` maps to `"rocm"` in `ANVILML_DEVICE_TYPE`.
#[test]
fn test_device_type_rocm() {
    let map = WorkerEnv::build(5555, "0", 0, DeviceType::Rocm, false, "info", 256);

    assert_eq!(map.get("ANVILML_DEVICE_TYPE"), Some(&"rocm".to_string()));
}

/// `DeviceType::Cpu` maps to `"cpu"` in `ANVILML_DEVICE_TYPE`.
#[test]
fn test_device_type_cpu() {
    let map = WorkerEnv::build(5555, "0", 0, DeviceType::Cpu, false, "info", 256);

    assert_eq!(map.get("ANVILML_DEVICE_TYPE"), Some(&"cpu".to_string()));
}

/// `ANVILML_FORCE_WORKER_MOCK` is never set by the builder, even when
/// `mock = true`. That variable is handled separately by the caller
/// (the supervisor) as an independent runtime trigger.
#[test]
fn test_force_worker_mock_absent() {
    let map = WorkerEnv::build(5555, "1", 2, DeviceType::Rocm, true, "trace", 1024);

    assert!(
        !map.contains_key("ANVILML_FORCE_WORKER_MOCK"),
        "ANVILML_FORCE_WORKER_MOCK must never appear in the builder output"
    );
}
