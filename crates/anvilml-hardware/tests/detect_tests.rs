/// Integration tests for `detect_all_devices` — the hardware detection entry point.
///
/// These tests verify the hardware_override short-circuit path (step 1 of the
/// priority chain per `ANVILML_DESIGN.md §6.4`) and the full detection chain
/// (steps 2–4: mock/Vulkan/fallback) implemented by `P5-A2`.
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
/// `Ok(HardwareInfo)` with host info populated and a GPU device list that depends
/// on the build feature and platform.
///
/// In mock-hardware builds: returns the mock-detected device from MockDetector.
/// In real-hardware builds: returns whatever Vulkan/platform detection finds.
/// Either way, the function always returns Ok (never Err) with valid host info.
#[tokio::test]
async fn test_override_absent_returns_hardware_info() {
    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg).await;

    // The function must always return Ok — it never returns Err.
    let result = result.expect("detect_all_devices should always return Ok");

    // Host info must be populated from environment variables.
    assert!(result.host.hostname.len() > 0, "hostname is non-empty");
    assert!(result.host.os.len() > 0, "OS is non-empty");

    // inference_caps must be default (all false) — P5-A3 will compute the union.
    assert_eq!(
        result.inference_caps,
        InferenceCaps::default(),
        "inference_caps is default (all false)"
    );
}

/// When `hardware_override` is `None` and the detection chain produces devices,
/// the returned `HardwareInfo` has `inference_caps` equal to `InferenceCaps::default()`
/// — the per-device caps union is deferred to P5-A3.
///
/// This test verifies the partial HardwareInfo contract: the function returns
/// detected GPUs with default inference_caps, leaving the final assembly to P5-A3.
#[tokio::test]
async fn test_partial_hardware_info_has_default_inference_caps() {
    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should always return Ok");

    // The function returns a partial HardwareInfo with default inference_caps.
    // P5-A3 will extend this to compute the union of per-device caps.
    assert_eq!(
        result.inference_caps,
        InferenceCaps::default(),
        "partial HardwareInfo has default inference_caps"
    );

    // Host info is always populated.
    assert!(!result.host.hostname.is_empty(), "hostname is populated");
    assert!(!result.host.os.is_empty(), "OS is populated");
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

/// When `mock-hardware` feature is compiled in and no override is set,
/// `detect_all_devices` returns exactly the mock-detected device with
/// fields derived from `ANVILML_MOCK_DEVICE_TYPE=cuda` and
/// `ANVILML_MOCK_VRAM_MIB=24576`.
///
/// Verifies: device_type is Cuda, vram_total_mib is 24576,
/// enumeration_source is Mock, and the device name is the mock default.
#[serial_test::serial]
#[cfg(feature = "mock-hardware")]
#[tokio::test]
async fn test_mock_hardware_feature_returns_mock_device() {
    // Capture and restore env vars for test isolation — std::env is process-global.
    let prior_type = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    let prior_vram = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    let prior_name = std::env::var("ANVILML_MOCK_DEVICE_NAME").ok();

    // SAFETY: setting env vars in tests is safe when done with proper
    // capture-and-restore isolation (serial_test ensures no concurrent access).
    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "24576");
    }
    // No ANVILML_MOCK_DEVICE_NAME — uses default "Mock GPU".

    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert_eq!(result.gpus.len(), 1, "mock returns exactly one device");

    let device = &result.gpus[0];
    assert_eq!(device.device_type, DeviceType::Cuda, "device_type is Cuda");
    assert_eq!(
        device.vram_total_mib, 24576,
        "vram_total_mib matches mock env var"
    );
    assert_eq!(
        device.enumeration_source,
        EnumerationSource::Mock,
        "enumeration_source is Mock"
    );
    assert_eq!(device.name, "Mock GPU", "device name is mock default");

    // Restore env vars unconditionally.
    // SAFETY: restoring env vars in tests is safe with proper isolation.
    unsafe {
        match prior_type {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE"),
        }
        match prior_vram {
            Some(v) => std::env::set_var("ANVILML_MOCK_VRAM_MIB", v),
            None => std::env::remove_var("ANVILML_MOCK_VRAM_MIB"),
        }
        match prior_name {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_NAME", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_NAME"),
        }
    }
}

/// When `mock-hardware` feature is compiled in and both mock env vars AND
/// `hardware_override` are set, the override short-circuit fires before
/// `MockDetector` is queried — proving override priority is preserved.
///
/// Verifies: the returned device has `device_type == Rocm` (from override)
/// and `vram_total_mib == 16384` (from override), not the mock values.
#[serial_test::serial]
#[cfg(feature = "mock-hardware")]
#[tokio::test]
async fn test_override_takes_priority_over_mock() {
    // Capture and restore env vars for test isolation.
    let prior_type = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    let prior_vram = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    let prior_name = std::env::var("ANVILML_MOCK_DEVICE_NAME").ok();

    // SAFETY: setting env vars in tests is safe with proper capture-and-restore
    // isolation (serial_test ensures no concurrent access).
    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "8192");
    }

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

    let device = &result.gpus[0];
    assert_eq!(
        device.device_type,
        DeviceType::Rocm,
        "device_type is Rocm (from override, not mock)"
    );
    assert_eq!(
        device.vram_total_mib, 16384,
        "vram_total_mib is 16384 (from override, not mock's 8192)"
    );
    assert_eq!(
        device.enumeration_source,
        EnumerationSource::Override,
        "enumeration_source is Override"
    );

    // Restore env vars unconditionally.
    // SAFETY: restoring env vars in tests is safe with proper isolation.
    unsafe {
        match prior_type {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE"),
        }
        match prior_vram {
            Some(v) => std::env::set_var("ANVILML_MOCK_VRAM_MIB", v),
            None => std::env::remove_var("ANVILML_MOCK_VRAM_MIB"),
        }
        match prior_name {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_NAME", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_NAME"),
        }
    }
}

/// When `mock-hardware` feature is compiled in, custom mock env vars
/// (`ANVILML_MOCK_DEVICE_NAME` and `ANVILML_MOCK_VRAM_MIB`) are correctly
/// read through the full detection chain — not just by `MockDetector` in
/// isolation.
///
/// Verifies: the returned device has name "Custom Mock GPU" and
/// vram_total_mib=16384, confirming the env vars propagate through
/// `detect_all_devices` → `MockDetector::detect()` → `GpuDevice` construction.
#[serial_test::serial]
#[cfg(feature = "mock-hardware")]
#[tokio::test]
async fn test_mock_detector_env_vars_propagate_through_detect_all_devices() {
    // Capture and restore env vars for test isolation.
    let prior_type = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    let prior_vram = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    let prior_name = std::env::var("ANVILML_MOCK_DEVICE_NAME").ok();

    // SAFETY: setting env vars in tests is safe with proper capture-and-restore
    // isolation (serial_test ensures no concurrent access).
    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_NAME", "Custom Mock GPU");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "16384");
    }

    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert_eq!(result.gpus.len(), 1, "mock returns exactly one device");

    let device = &result.gpus[0];
    assert_eq!(
        device.name, "Custom Mock GPU",
        "device name matches custom mock env var"
    );
    assert_eq!(
        device.vram_total_mib, 16384,
        "vram_total_mib matches custom mock env var"
    );

    // Restore env vars unconditionally.
    // SAFETY: restoring env vars in tests is safe with proper isolation.
    unsafe {
        match prior_type {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE"),
        }
        match prior_vram {
            Some(v) => std::env::set_var("ANVILML_MOCK_VRAM_MIB", v),
            None => std::env::remove_var("ANVILML_MOCK_VRAM_MIB"),
        }
        match prior_name {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_NAME", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_NAME"),
        }
    }
}
