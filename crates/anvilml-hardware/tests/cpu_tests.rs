/// Integration tests for `CpuDetector` — the unconditional final-fallback detector.
///
/// These tests verify that `CpuDetector` always returns exactly one synthesized
/// CPU device with the correct field values, per §6.2 of the design.
use anvilml_core::types::*;
use anvilml_hardware::cpu::CpuDetector;
use anvilml_hardware::detect::DeviceDetector;

/// Construct `CpuDetector`, call `detect()`, assert the result is `Ok(vec![..])`
/// with exactly one element, and verify the device's `name == "CPU"`.
#[test]
fn test_cpu_detector_returns_one_device() {
    let detector = CpuDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(result.len(), 1, "CpuDetector returns exactly one device");
    assert_eq!(result[0].name, "CPU", "device name is CPU");
}

/// Assert the returned device has `device_type == DeviceType::Cpu`.
#[test]
fn test_cpu_detector_device_type_is_cpu() {
    let detector = CpuDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(
        result[0].device_type,
        DeviceType::Cpu,
        "device_type is CPU, not a GPU backend"
    );
}

/// Assert the returned device has `enumeration_source == EnumerationSource::Cpu`
/// (distinct from `EnumerationSource::Mock` which is env-var-driven, P4-A3).
#[test]
fn test_cpu_detector_enumeration_source_is_cpu() {
    let detector = CpuDetector;
    let result = detector.detect().expect("detect() should always succeed");
    assert_eq!(
        result[0].enumeration_source,
        EnumerationSource::Cpu,
        "enumeration_source is Cpu (synthesised fallback), not Mock"
    );
}

/// Construct `CpuDetector`, call `refresh_vram(0)`, assert `Ok((0, 0))` —
/// CPU has no VRAM.
#[test]
fn test_cpu_detector_refresh_vram_returns_zero() {
    let detector = CpuDetector;
    let result = detector
        .refresh_vram(0)
        .expect("refresh_vram() should always succeed");
    assert_eq!(
        result,
        (0, 0),
        "CPU has no VRAM — both total and free are 0"
    );
}

/// Assert every other field on the returned `GpuDevice` matches expected values:
/// vram_total_mib=0, vram_free_mib=0, driver_version="n/a", pci_ids=0,
/// arch=None, caps=default (all-false), capabilities_source=Fallback.
#[test]
fn test_cpu_detector_all_device_fields() {
    let detector = CpuDetector;
    let result = detector.detect().expect("detect() should always succeed");
    let device = &result[0];

    assert_eq!(device.index, 0, "zero-based device index");
    assert_eq!(device.vram_total_mib, 0, "CPU has no VRAM");
    assert_eq!(device.vram_free_mib, 0, "CPU has no VRAM");
    assert_eq!(device.driver_version, "n/a", "no driver string for CPU");
    assert_eq!(device.pci_vendor_id, 0, "CPU has no PCI device");
    assert_eq!(device.pci_device_id, 0, "CPU has no PCI device");
    assert!(device.arch.is_none(), "CPU has no GPU architecture string");
    assert_eq!(
        device.caps,
        InferenceCaps::default(),
        "all inference capability flags are false for CPU"
    );
    assert_eq!(
        device.capabilities_source,
        CapabilitySource::Fallback,
        "pre-spawn hint, not authoritative"
    );
}

/// Call `detect()` and assert `result.is_ok()` — proves it never panics
/// or returns `Err`. `CpuDetector` is pure value construction with no I/O.
#[test]
fn test_cpu_detect_never_errors() {
    let detector = CpuDetector;
    let result = detector.detect();
    assert!(result.is_ok(), "detect() never returns Err for CpuDetector");
}
