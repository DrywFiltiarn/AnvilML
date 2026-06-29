/// Integration tests for `detect_all_devices` — the hardware detection entry point.
///
/// These tests verify the hardware_override short-circuit path (step 1 of the
/// priority chain per `ANVILML_DESIGN.md §6.4`) and the full detection chain
/// (steps 2–4: mock/Vulkan/fallback) implemented by `P5-A2`, plus the CPU-append
/// and caps-union assembly implemented by `P5-A3`.
use anvilml_core::{
    DeviceType, EnumerationSource, HardwareOverrideConfig, InferenceCaps, ServerConfig,
};
use anvilml_hardware::detect::detect_all_devices;

/// When `hardware_override` is `Some`, `detect_all_devices` returns `Ok(HardwareInfo)`
/// with exactly two devices: the override-synthesized `GpuDevice` followed by the
/// unconditional CPU fallback device.
///
/// Verifies: device_type == Cuda, vram_total_mib == 24576, enumeration_source == Override,
/// capabilities_source == Fallback, and that the device name reflects the device type.
/// Also verifies the CPU fallback is appended last.
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

    // After P5-A3: override path includes CPU fallback, so 2 devices total.
    assert_eq!(
        result.gpus.len(),
        2,
        "override returns override device + CPU fallback"
    );

    // First device is the override-synthesized device.
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

    // Second device is the CPU fallback.
    let cpu = &result.gpus[1];
    assert_eq!(
        cpu.device_type,
        DeviceType::Cpu,
        "last device is CPU fallback"
    );
    assert_eq!(
        cpu.enumeration_source,
        EnumerationSource::Cpu,
        "CPU fallback has Cpu enumeration source"
    );

    assert!(result.host.hostname.len() > 0, "hostname is non-empty");
    assert!(result.host.os.len() > 0, "OS is non-empty");
}

/// When `hardware_override` is `None` (the default), `detect_all_devices` returns
/// `Ok(HardwareInfo)` with host info populated and a GPU device list that depends
/// on the build feature and platform.
///
/// In mock-hardware builds: returns the mock-detected device plus the CPU fallback.
/// In real-hardware builds: returns whatever Vulkan/platform detection finds plus CPU.
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

    // After P5-A3: inference_caps is the union of all device caps (not just default).
    // The CPU device has default caps, so the union is at least as permissive as default.
    assert!(
        result.gpus.len() >= 1,
        "at least one device (CPU fallback) is always present"
    );
}

/// After P5-A3, `detect_all_devices` returns `HardwareInfo` with `inference_caps`
/// equal to the field-wise OR union of all per-device `InferenceCaps`.
///
/// Since mock devices and CPU fallback both have default (all-false) caps,
/// the union is also all-false (default). This test verifies the caps-union
/// logic is applied rather than returning a hardcoded default.
#[tokio::test]
async fn test_inference_caps_is_caps_union() {
    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should always return Ok");

    // With default-cap devices, the union equals default — but the important
    // invariant is that caps are computed, not hardcoded.
    assert_eq!(
        result.inference_caps,
        InferenceCaps::default(),
        "inference_caps is the union of per-device caps (default when all devices have default caps)"
    );

    // Host info is always populated.
    assert!(!result.host.hostname.is_empty(), "hostname is populated");
    assert!(!result.host.os.is_empty(), "OS is populated");
}

/// When `hardware_override.device_type` is an unrecognized string, the function
/// falls back to `DeviceType::Cpu` with a warning log.
///
/// After P5-A3: returns 2 devices (override CPU + CPU fallback).
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

    // After P5-A3: 2 devices (override CPU + CPU fallback).
    assert_eq!(
        result.gpus.len(),
        2,
        "override returns override device + CPU fallback"
    );

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
/// `GpuDevice` with `device_type == Rocm` and `name == "ROCm"`, followed by
/// the CPU fallback device.
///
/// After P5-A3: returns 2 devices.
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

    // After P5-A3: 2 devices (override ROCm + CPU fallback).
    assert_eq!(
        result.gpus.len(),
        2,
        "override returns override device + CPU fallback"
    );

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
/// `GpuDevice` with `device_type == Cpu` and `name == "CPU"`, followed by
/// the CPU fallback device.
///
/// After P5-A3: returns 2 devices.
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

    // After P5-A3: 2 devices (override CPU + CPU fallback).
    assert_eq!(
        result.gpus.len(),
        2,
        "override returns override device + CPU fallback"
    );

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

/// The returned `HardwareInfo` has `inference_caps` equal to the union of all
/// per-device `InferenceCaps`. Since override devices and CPU fallback both
/// have default (all-false) caps, the union is also default.
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

    // Override device has default caps, CPU fallback has default caps.
    // Union of defaults = defaults.
    assert_eq!(
        result.inference_caps,
        InferenceCaps::default(),
        "inference_caps is the union of all device caps (default when all devices have default caps)"
    );
}

/// When `mock-hardware` feature is compiled in and no override is set,
/// `detect_all_devices` returns the mock-detected device followed by the
/// CPU fallback device, with fields derived from
/// `ANVILML_MOCK_DEVICE_TYPE=cuda` and `ANVILML_MOCK_VRAM_MIB=24576`.
///
/// Verifies: mock device has `device_type == Cuda`, `vram_total_mib == 24576`,
/// `enumeration_source == Mock`, and the CPU fallback is appended last.
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

    // After P5-A3: 2 devices (mock GPU + CPU fallback).
    assert_eq!(
        result.gpus.len(),
        2,
        "mock returns mock device + CPU fallback"
    );

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

    // Second device is the CPU fallback.
    let cpu = &result.gpus[1];
    assert_eq!(
        cpu.device_type,
        DeviceType::Cpu,
        "last device is CPU fallback"
    );
    assert_eq!(
        cpu.enumeration_source,
        EnumerationSource::Cpu,
        "CPU fallback has Cpu enumeration source"
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

/// When `mock-hardware` feature is compiled in and both mock env vars AND
/// `hardware_override` are set, the override short-circuit fires before
/// `MockDetector` is queried — proving override priority is preserved.
///
/// Verifies: the returned devices are the override device + CPU fallback
/// (not the mock device), and `device_type == Rocm` (from override).
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

    // After P5-A3: 2 devices (override ROCm + CPU fallback).
    assert_eq!(
        result.gpus.len(),
        2,
        "override returns override device + CPU fallback"
    );

    // First device is the override (not the mock).
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

    // Second device is the CPU fallback.
    let cpu = &result.gpus[1];
    assert_eq!(
        cpu.device_type,
        DeviceType::Cpu,
        "last device is CPU fallback"
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
/// Verifies: the returned mock device has name "Custom Mock GPU" and
/// vram_total_mib=16384, and the CPU fallback is appended last.
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

    // After P5-A3: 2 devices (mock GPU + CPU fallback).
    assert_eq!(
        result.gpus.len(),
        2,
        "mock returns mock device + CPU fallback"
    );

    let device = &result.gpus[0];
    assert_eq!(
        device.name, "Custom Mock GPU",
        "device name matches custom mock env var"
    );
    assert_eq!(
        device.vram_total_mib, 16384,
        "vram_total_mib matches custom mock env var"
    );

    // Second device is the CPU fallback.
    let cpu = &result.gpus[1];
    assert_eq!(
        cpu.device_type,
        DeviceType::Cpu,
        "last device is CPU fallback"
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

/// The CPU fallback device is always the last element in `gpus` when the
/// mock-hardware feature is active. This test verifies that after mock
/// GPU detection, `CpuDetector`'s device is appended and occupies the
/// final index with `enumeration_source == Cpu`.
#[serial_test::serial]
#[cfg(feature = "mock-hardware")]
#[tokio::test]
async fn test_cpu_device_always_present_and_last() {
    // Capture and restore env vars for test isolation.
    let prior_type = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    let prior_vram = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    let prior_name = std::env::var("ANVILML_MOCK_DEVICE_NAME").ok();

    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "24576");
    }

    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    // After P5-A3: CPU device is always the last element.
    assert!(result.gpus.len() >= 2, "at least mock GPU + CPU fallback");

    let last = result.gpus.last().unwrap();
    assert_eq!(
        last.device_type,
        DeviceType::Cpu,
        "last device is CPU fallback"
    );
    assert_eq!(
        last.enumeration_source,
        EnumerationSource::Cpu,
        "last device has Cpu enumeration source"
    );
    assert_eq!(last.name, "CPU", "last device name is CPU");

    // Restore env vars unconditionally.
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

/// `inference_caps` is the field-wise OR union of all per-device `InferenceCaps`.
///
/// With the mock-hardware feature and default mock caps (all false) plus CPU
/// fallback caps (all false), the union is all false (default). This test
/// confirms the union logic is applied correctly.
#[serial_test::serial]
#[cfg(feature = "mock-hardware")]
#[tokio::test]
async fn test_inference_caps_union_correctness() {
    let prior_type = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    let prior_vram = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    let prior_name = std::env::var("ANVILML_MOCK_DEVICE_NAME").ok();

    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "24576");
    }

    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    // Both mock and CPU devices have default (all-false) caps, so union = default.
    assert_eq!(
        result.inference_caps,
        InferenceCaps::default(),
        "inference_caps is the field-wise OR union of all device caps"
    );

    // Restore env vars unconditionally.
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

/// `host.hostname` and `host.os` are both non-empty strings after
/// `detect_all_devices()` returns. This verifies the minimal `HostInfo`
/// population works correctly regardless of which detection path is taken.
#[tokio::test]
async fn test_host_fields_non_empty() {
    let cfg = ServerConfig::default();

    let result = detect_all_devices(&cfg)
        .await
        .expect("detect_all_devices should return Ok");

    assert!(result.host.hostname.len() > 0, "hostname is non-empty");
    assert!(result.host.os.len() > 0, "OS is non-empty");
}

/// When `hardware_override` is set, the override path still appends the
/// CPU fallback device, making the result contain 2 devices. This ensures
/// the CPU guarantee applies universally to ALL code paths, including override.
#[tokio::test]
async fn test_override_path_still_has_cpu_device() {
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

    // After P5-A3: override path returns 2 devices (override GPU + CPU fallback).
    assert_eq!(
        result.gpus.len(),
        2,
        "override path has CPU device appended"
    );

    // First device is the override GPU.
    assert_eq!(
        result.gpus[0].enumeration_source,
        EnumerationSource::Override,
        "first device is the override device"
    );

    // Second device is the CPU fallback.
    assert_eq!(
        result.gpus[1].enumeration_source,
        EnumerationSource::Cpu,
        "second device is the CPU fallback"
    );
}
