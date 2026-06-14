/// Integration tests for `CpuDetector`.
///
/// These tests exercise the `DeviceDetector` trait implementation on
/// `CpuDetector` using the crate's public API. All tests are marked
/// `#[serial]` because they read process-global system state via sysinfo,
/// which is shared across test threads.
use anvilml_hardware::{CpuDetector, DeviceDetector};

/// Verify that `CpuDetector::detect()` returns exactly one device with
/// `DeviceType::Cpu` and index `0`.
#[serial_test::serial]
#[test]
fn test_cpu_detector_detect_returns_one_device() {
    let detector = CpuDetector::new();
    let devices = detector.detect().expect("detect() should not fail");

    assert_eq!(
        devices.len(),
        1,
        "CpuDetector must return exactly one device (the CPU)"
    );

    let device = &devices[0];
    assert_eq!(device.device_type, anvilml_core::DeviceType::Cpu);
    assert_eq!(device.index, 0);
}

/// Verify that `CpuDetector::refresh_vram(0)` returns `(0, 0)` because
/// CPUs have no dedicated video memory.
#[serial_test::serial]
#[test]
fn test_cpu_detector_refresh_vram_returns_zero() {
    let detector = CpuDetector::new();
    let (total, free) = detector
        .refresh_vram(0)
        .expect("refresh_vram() should not fail");

    assert_eq!(total, 0, "CPU has no total VRAM");
    assert_eq!(free, 0, "CPU has no free VRAM");
}

/// Compile-time assertion that `CpuDetector` implements `Send + Sync`.
///
/// This is a zero-cost check: if `CpuDetector` does not implement either
/// trait, this function will not compile. The `DeviceDetector` trait
/// requires `Send + Sync`, so this verifies the impl satisfies the
/// trait bounds.
#[serial_test::serial]
#[test]
fn test_cpu_detector_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CpuDetector>();
}
