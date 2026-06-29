/// Integration tests for `detect_all_devices` — the hardware detection entry point.
///
/// These tests verify the hardware_override short-circuit path (step 1 of the
/// priority chain per `ANVILML_DESIGN.md §6.4`) and the error path when no
/// override is configured.
use anvilml_core::{
    DeviceType, EnumerationSource, HardwareOverrideConfig, InferenceCaps, ServerConfig,
};
use anvilml_hardware::detect::detect_all_devices;

/// When `hardware_override` is `Some`, `detect_all_devices` returns `Ok(HardwareInfo)`
/// with exactly one synthesized `GpuDevice` matching the override config fields.
///
/// Verifies: device_type == Cuda, vram_total_mib == 24576, enumeration_source == Override,
/// capabilities_source == Fallback, and that the device name reflects the device type.
#[tokio::test]
async fn test_override_present_returns_device() {
    let cfg = ServerConfig {
        hardware_override: Some(HardwareOverrideConfig {
            device_type: "cuda".into(),
            vram_total_mib: 24576,
        }),
        ..ServerConfig::default()
    };

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert_eq!(result.gpus.len(), 1, "override returns exactly one device");

    let device = &result.gpus[0];
    assert_eq!(device.device_type, DeviceType::Cuda, "device_type is Cuda");
    assert_eq!(
        device.vram_total_mib, 24576,
        "vram_total_mib matches override config"
    );
    assert_eq!(
        device.enumeration_source,
        EnumerationSource::Override,
        "enumeration_source is Override"
    );
    assert_eq!(
        device.capabilities_source,
        anvilml_core::CapabilitySource::Fallback,
        "capabilities_source is Fallback"
    );
    assert_eq!(device.name, "CUDA", "device name reflects the device type");
    assert_eq!(
        device.driver_version, "override",
        "driver_version is 'override' for synthesized device"
    );
    assert_eq!(
        device.vram_free_mib, 24576,
        "free VRAM equals total VRAM for override"
    );
    assert!(result.host.hostname.len() > 0, "hostname is non-empty");
    assert!(result.host.os.len() > 0, "OS is non-empty");
}

/// When `hardware_override` is `None` (the default), `detect_all_devices` returns
/// `Err(AnvilError::Internal)` with a message indicating the full chain is not yet
/// implemented.
///
/// This proves the function is callable and returns the expected error type when
/// no override is configured — the full detection chain is deferred to P5-A2.
#[tokio::test]
async fn test_override_absent_returns_err() {
    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg).await;

    assert!(
        result.is_err(),
        "detect_all_devices returns Err when override is absent"
    );

    let err = result.unwrap_err();
    let err_msg = format!("{err:?}");
    assert!(
        err_msg.contains("not yet implemented"),
        "error message indicates the chain is not yet implemented — got: {err_msg}"
    );
}

/// When `hardware_override.device_type` is an unrecognized string, the function
/// falls back to `DeviceType::Cpu` with a warning log.
#[tokio::test]
async fn test_override_unrecognized_device_type_defaults_to_cpu() {
    let cfg = ServerConfig {
        hardware_override: Some(HardwareOverrideConfig {
            device_type: "metal".into(),
            vram_total_mib: 8192,
        }),
        ..ServerConfig::default()
    };

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert_eq!(result.gpus.len(), 1, "override returns exactly one device");
    assert_eq!(
        result.gpus[0].device_type,
        DeviceType::Cpu,
        "unrecognized device_type defaults to Cpu"
    );
    assert_eq!(
        result.gpus[0].name, "CPU",
        "device name is CPU for unrecognized type"
    );
}

/// When `hardware_override.device_type` is `"rocm"`, the function returns a
/// `GpuDevice` with `device_type == Rocm` and `name == "ROCm"`.
#[tokio::test]
async fn test_override_rocm_device_type() {
    let cfg = ServerConfig {
        hardware_override: Some(HardwareOverrideConfig {
            device_type: "rocm".into(),
            vram_total_mib: 16384,
        }),
        ..ServerConfig::default()
    };

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert_eq!(result.gpus.len(), 1, "override returns exactly one device");
    assert_eq!(
        result.gpus[0].device_type,
        DeviceType::Rocm,
        "device_type is Rocm"
    );
    assert_eq!(
        result.gpus[0].name, "ROCm",
        "device name is ROCm for rocm override"
    );
    assert_eq!(
        result.gpus[0].vram_total_mib, 16384,
        "vram_total_mib matches override config"
    );
}

/// When `hardware_override.device_type` is `"cpu"`, the function returns a
/// `GpuDevice` with `device_type == Cpu` and `name == "CPU"`.
#[tokio::test]
async fn test_override_cpu_device_type() {
    let cfg = ServerConfig {
        hardware_override: Some(HardwareOverrideConfig {
            device_type: "cpu".into(),
            vram_total_mib: 0,
        }),
        ..ServerConfig::default()
    };

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert_eq!(result.gpus.len(), 1, "override returns exactly one device");
    assert_eq!(
        result.gpus[0].device_type,
        DeviceType::Cpu,
        "device_type is Cpu"
    );
    assert_eq!(
        result.gpus[0].name, "CPU",
        "device name is CPU for cpu override"
    );
    assert_eq!(
        result.gpus[0].vram_total_mib, 0,
        "vram_total_mib matches override config (0 for CPU)"
    );
}

/// The returned `HardwareInfo` has `inference_caps` equal to `InferenceCaps::default()`
/// (all fields false) when the override device is synthesized — since override devices
/// have no real inference capabilities, this is the correct default.
#[tokio::test]
async fn test_override_inference_caps_is_default() {
    let cfg = ServerConfig {
        hardware_override: Some(HardwareOverrideConfig {
            device_type: "cuda".into(),
            vram_total_mib: 24576,
        }),
        ..ServerConfig::default()
    };

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert_eq!(
        result.inference_caps,
        InferenceCaps::default(),
        "inference_caps is default (all false) for override device"
    );
}
