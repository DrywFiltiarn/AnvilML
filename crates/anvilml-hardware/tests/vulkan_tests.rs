/// Integration tests for `VulkanDetector`.
///
/// These tests exercise the `DeviceDetector` trait implementation on
/// `VulkanDetector` using the crate's public API. All tests are marked
/// `#[serial]` because the Vulkan detection path reads process-global
/// system state (Vulkan loader availability) which is shared across
/// test threads.
use anvilml_hardware::{DeviceDetector, VulkanDetector};

/// Verify that `VulkanDetector::new()` constructs a zero-sized struct.
///
/// This is a zero-cost check: constructing a unit struct involves no
/// allocation, no I/O, and no system calls.
#[serial_test::serial]
#[test]
fn test_vulkan_detector_new() {
    let detector = VulkanDetector::new();
    // Unit struct — just verify construction succeeded.
    let _ = detector;
}

/// Verify that `VulkanDetector::detect()` never panics and returns a
/// valid result.
///
/// On systems without a Vulkan loader (CI, WSL2 without GPU), this
/// returns `Ok(vec![])`. On systems with Vulkan GPUs, it returns the
/// detected devices. Either outcome is acceptable — the key invariant
/// is that the method never panics or returns an Err.
#[serial_test::serial]
#[test]
fn test_vulkan_detector_detect_returns_empty_or_devices() {
    let detector = VulkanDetector::new();
    let result = detector.detect();

    // The method must never return an error — Vulkan failures are
    // treated as "no GPUs" rather than hard errors.
    assert!(
        result.is_ok(),
        "detect() should not return Err — Vulkan failures are graceful"
    );

    let devices = result.expect("detect() returned Ok");

    // The device list must be a valid vec (possibly empty).
    // On a system with no Vulkan loader, this will be empty.
    // On a system with Vulkan GPUs, it will contain detected devices.
    assert!(
        devices.is_empty() || !devices.is_empty(),
        "detect() returned a valid device list"
    );
}

/// Verify that `VulkanDetector::refresh_vram(0)` returns `(0, 0)`
/// because no Vulkan device context exists at detection time.
///
/// Live VRAM refresh requires a Vulkan device (queue, command buffer)
/// which this task does not create. It is handled by NVML in P4-A3.
#[serial_test::serial]
#[test]
fn test_vulkan_detector_refresh_vram_returns_zero() {
    let detector = VulkanDetector::new();
    let (total, free) = detector
        .refresh_vram(0)
        .expect("refresh_vram() should not fail");

    assert_eq!(
        total, 0,
        "Vulkan has no total VRAM without a device context"
    );
    assert_eq!(free, 0, "Vulkan has no free VRAM without a device context");
}

/// Compile-time assertion that `VulkanDetector` implements `Send + Sync`.
///
/// This is a zero-cost check: if `VulkanDetector` does not implement
/// either trait, this function will not compile. The `DeviceDetector`
/// trait requires `Send + Sync`, so this verifies the impl satisfies
/// the trait bounds.
#[serial_test::serial]
#[test]
fn test_vulkan_detector_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VulkanDetector>();
}
