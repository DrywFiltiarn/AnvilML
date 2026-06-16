//! Integration tests for `build_worker_env`.
//!
//! Each test constructs a known `GpuDevice` and `ServerConfig`, calls
//! `build_worker_env`, and asserts the resulting HashMap contains the
//! expected key-value pairs.

use anvilml_core::{DeviceType, GpuDevice, ServerConfig};
use anvilml_worker::build_worker_env;

/// A minimal `GpuDevice` fixture for tests.
///
/// Uses `index = 0` and `DeviceType::Cpu` by default; individual tests
/// override fields as needed.
fn make_device(device_type: DeviceType) -> GpuDevice {
    GpuDevice {
        index: 0,
        name: "test-device".to_string(),
        db_name: None,
        device_type,
        vram_total_mib: 0,
        vram_free_mib: 0,
        driver_version: "0.0".to_string(),
        pci_vendor_id: 0,
        pci_device_id: 0,
        arch: None,
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Mock,
        capabilities_source: anvilml_core::CapabilitySource::Fallback,
    }
}

/// Verify `ANVILML_IPC_PORT` equals the port argument as a decimal string.
///
/// Preconditions: None.
/// Inputs: port = 9000, device index = 0, any config.
/// Expected output: `map["ANVILML_IPC_PORT"] == "9000"`.
#[test]
fn test_ipc_port() {
    let device = make_device(DeviceType::Cpu);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 9000);
    assert_eq!(
        map.get("ANVILML_IPC_PORT").map(String::as_str),
        Some("9000")
    );
}

/// Verify `ANVILML_WORKER_ID` equals the device index as a string.
///
/// Preconditions: None.
/// Inputs: device.index = 0.
/// Expected output: `map["ANVILML_WORKER_ID"] == "0"`.
#[test]
fn test_worker_id() {
    let device = make_device(DeviceType::Cpu);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(map.get("ANVILML_WORKER_ID").map(String::as_str), Some("0"));
}

/// Verify `ANVILML_DEVICE_INDEX` equals the device index as a string.
///
/// Preconditions: None.
/// Inputs: device.index = 0.
/// Expected output: `map["ANVILML_DEVICE_INDEX"] == "0"`.
#[test]
fn test_device_index() {
    let device = make_device(DeviceType::Cpu);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(
        map.get("ANVILML_DEVICE_INDEX").map(String::as_str),
        Some("0")
    );
}

/// Verify `ANVILML_DEVICE_TYPE` is `"cuda"` for `DeviceType::Cuda`.
///
/// Preconditions: None.
/// Inputs: device_type = DeviceType::Cuda.
/// Expected output: `map["ANVILML_DEVICE_TYPE"] == "cuda"`.
#[test]
fn test_device_type_cuda() {
    let device = make_device(DeviceType::Cuda);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(
        map.get("ANVILML_DEVICE_TYPE").map(String::as_str),
        Some("cuda")
    );
}

/// Verify `ANVILML_DEVICE_TYPE` is `"rocm"` for `DeviceType::Rocm`.
///
/// Preconditions: None.
/// Inputs: device_type = DeviceType::Rocm.
/// Expected output: `map["ANVILML_DEVICE_TYPE"] == "rocm"`.
#[test]
fn test_device_type_rocm() {
    let device = make_device(DeviceType::Rocm);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(
        map.get("ANVILML_DEVICE_TYPE").map(String::as_str),
        Some("rocm")
    );
}

/// Verify `ANVILML_DEVICE_TYPE` is `"cpu"` for `DeviceType::Cpu`.
///
/// Preconditions: None.
/// Inputs: device_type = DeviceType::Cpu.
/// Expected output: `map["ANVILML_DEVICE_TYPE"] == "cpu"`.
#[test]
fn test_device_type_cpu() {
    let device = make_device(DeviceType::Cpu);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(
        map.get("ANVILML_DEVICE_TYPE").map(String::as_str),
        Some("cpu")
    );
}

/// Verify `ANVILML_LOG_LEVEL` matches `cfg.log_level`.
///
/// Preconditions: None.
/// Inputs: cfg.log_level = "debug".
/// Expected output: `map["ANVILML_LOG_LEVEL"] == "debug"`.
#[test]
fn test_log_level() {
    let device = make_device(DeviceType::Cpu);
    // Construct a config with a non-default log level.
    let cfg = ServerConfig {
        log_level: "debug".to_string(),
        ..ServerConfig::default()
    };
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(
        map.get("ANVILML_LOG_LEVEL").map(String::as_str),
        Some("debug")
    );
}

/// Verify `ANVILML_MAX_IPC_PAYLOAD_MIB` matches the config value.
///
/// Preconditions: None.
/// Inputs: cfg.max_ipc_payload_mib = 512.
/// Expected output: `map["ANVILML_MAX_IPC_PAYLOAD_MIB"] == "512"`.
#[test]
fn test_max_ipc_payload_mib() {
    let device = make_device(DeviceType::Cpu);
    let cfg = ServerConfig {
        max_ipc_payload_mib: 512,
        ..ServerConfig::default()
    };
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(
        map.get("ANVILML_MAX_IPC_PAYLOAD_MIB").map(String::as_str),
        Some("512")
    );
}

/// Verify `ANVILML_WORKER_MOCK` is `"1"` when compiled with `mock-hardware`.
///
/// Preconditions: The `mock-hardware` cargo feature must be enabled.
/// Inputs: Any device, any config, any port.
/// Expected output: `map["ANVILML_WORKER_MOCK"] == "1"`.
#[cfg(feature = "mock-hardware")]
#[test]
fn test_mock_hardware_flag() {
    let device = make_device(DeviceType::Cpu);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 8488);
    assert_eq!(
        map.get("ANVILML_WORKER_MOCK").map(String::as_str),
        Some("1")
    );
}

/// Verify the HashMap contains exactly 7 entries when compiled with
/// `mock-hardware` (6 without it).
///
/// Preconditions: None.
/// Inputs: Any device, any config, any port.
/// Expected output: `map.len() == 7` (with mock-hardware) or `6` (without).
#[test]
fn test_total_count() {
    let device = make_device(DeviceType::Cpu);
    let cfg = ServerConfig::default();
    let map = build_worker_env(&device, &cfg, 8488);

    // Six keys are always present; mock-hardware adds a seventh.
    #[cfg(feature = "mock-hardware")]
    assert_eq!(map.len(), 7);

    #[cfg(not(feature = "mock-hardware"))]
    assert_eq!(map.len(), 6);
}
