/// Integration tests for `MockDetector` — the env-var driven synthetic
/// hardware detector gated behind the `mock-hardware` feature.
///
/// Every test that mutates an environment variable uses `#[serial]` and
/// captures/restores the prior value unconditionally, per §11.3 of
/// ENVIRONMENT.md and §9.6 of FORGE_AGENT_RULES.md.
use anvilml_core::types::*;
use anvilml_hardware::detect::DeviceDetector;
use anvilml_hardware::mock::MockDetector;
use serial_test::serial;

/// Construct `MockDetector` with no env vars set; verify all default
/// values: device_type=Cpu, vram=8192, name="Mock GPU",
/// enumeration_source=Mock, capabilities_source=Fallback.
#[serial]
#[test]
fn test_mock_detector_defaults() {
    // Ensure all three mock env vars are unset so defaults apply.
    let prior_type = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    let prior_vram = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    let prior_name = std::env::var("ANVILML_MOCK_DEVICE_NAME").ok();

    // Rust 2024: std::env::remove_var is unsafe.
    unsafe {
        std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
        std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
        std::env::remove_var("ANVILML_MOCK_DEVICE_NAME");
    }

    let detector = MockDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(result.len(), 1, "MockDetector returns exactly one device");

    let device = &result[0];
    assert_eq!(
        device.device_type,
        DeviceType::Cpu,
        "default device_type is Cpu"
    );
    assert_eq!(device.vram_total_mib, 8192, "default VRAM is 8192 MiB");
    assert_eq!(
        device.vram_free_mib, 8192,
        "free VRAM equals total by default"
    );
    assert_eq!(device.name, "Mock GPU", "default device name is Mock GPU");
    assert_eq!(
        device.enumeration_source,
        EnumerationSource::Mock,
        "enumeration_source is Mock"
    );
    assert_eq!(
        device.capabilities_source,
        CapabilitySource::Fallback,
        "capabilities_source is Fallback"
    );

    // Restore prior env values unconditionally.
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

/// Set `ANVILML_MOCK_DEVICE_TYPE=cuda`; verify device_type is Cuda.
#[serial]
#[test]
fn test_mock_cuda_device_type() {
    let prior = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
    }

    let detector = MockDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(
        result[0].device_type,
        DeviceType::Cuda,
        "ANVILML_MOCK_DEVICE_TYPE=cuda produces DeviceType::Cuda"
    );

    // Restore prior env value unconditionally.
    unsafe {
        match prior {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE"),
        }
    }
}

/// Set `ANVILML_MOCK_DEVICE_TYPE=rocm`; verify device_type is Rocm.
#[serial]
#[test]
fn test_mock_rocm_device_type() {
    let prior = std::env::var("ANVILML_MOCK_DEVICE_TYPE").ok();
    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "rocm");
    }

    let detector = MockDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(
        result[0].device_type,
        DeviceType::Rocm,
        "ANVILML_MOCK_DEVICE_TYPE=rocm produces DeviceType::Rocm"
    );

    // Restore prior env value unconditionally.
    unsafe {
        match prior {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE"),
        }
    }
}

/// Set `ANVILML_MOCK_VRAM_MIB=16384`; verify vram_total_mib and
/// vram_free_mib are both 16384.
#[serial]
#[test]
fn test_mock_vram_override() {
    let prior = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    unsafe {
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "16384");
    }

    let detector = MockDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(
        result[0].vram_total_mib, 16384,
        "ANVILML_MOCK_VRAM_MIB=16384 sets vram_total_mib to 16384"
    );
    assert_eq!(
        result[0].vram_free_mib, 16384,
        "ANVILML_MOCK_VRAM_MIB=16384 sets vram_free_mib to 16384"
    );

    // Restore prior env value unconditionally.
    unsafe {
        match prior {
            Some(v) => std::env::set_var("ANVILML_MOCK_VRAM_MIB", v),
            None => std::env::remove_var("ANVILML_MOCK_VRAM_MIB"),
        }
    }
}

/// Set `ANVILML_MOCK_DEVICE_NAME=Test GPU`; verify name is "Test GPU".
#[serial]
#[test]
fn test_mock_device_name_override() {
    let prior = std::env::var("ANVILML_MOCK_DEVICE_NAME").ok();
    unsafe {
        std::env::set_var("ANVILML_MOCK_DEVICE_NAME", "Test GPU");
    }

    let detector = MockDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(
        result[0].name, "Test GPU",
        "ANVILML_MOCK_DEVICE_NAME=Test GPU sets device name to Test GPU"
    );

    // Restore prior env value unconditionally.
    unsafe {
        match prior {
            Some(v) => std::env::set_var("ANVILML_MOCK_DEVICE_NAME", v),
            None => std::env::remove_var("ANVILML_MOCK_DEVICE_NAME"),
        }
    }
}

/// Call `refresh_vram(0)` with default VRAM (no env vars set); verify
/// `Ok((8192, 8192))`.
#[serial]
#[test]
fn test_mock_refresh_vram() {
    let prior = std::env::var("ANVILML_MOCK_VRAM_MIB").ok();
    unsafe {
        std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    }

    let detector = MockDetector;
    let result = detector
        .refresh_vram(0)
        .expect("refresh_vram() should always succeed");
    assert_eq!(
        result,
        (8192, 8192),
        "refresh_vram(0) returns (8192, 8192) with default VRAM"
    );

    // Restore prior env value unconditionally.
    unsafe {
        match prior {
            Some(v) => std::env::set_var("ANVILML_MOCK_VRAM_MIB", v),
            None => std::env::remove_var("ANVILML_MOCK_VRAM_MIB"),
        }
    }
}
