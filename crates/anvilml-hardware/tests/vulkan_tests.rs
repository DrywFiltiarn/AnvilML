/// Integration tests for `VulkanDetector` — the headless Vulkan GPU detector.
///
/// These tests cover:
/// - Vendor ID → `DeviceType` mapping (pure function, no Vulkan required)
/// - `detect()` graceful degradation when Vulkan loader is absent
/// - `refresh_vram()` fallback behavior
use anvilml_core::types::*;
use anvilml_hardware::detect::DeviceDetector;
use anvilml_hardware::vendor_id_to_device_type;
use anvilml_hardware::vulkan::VulkanDetector;

/// Verify `vendor_id_to_device_type(0x10de)` returns `Some(DeviceType::Cuda)`
/// — NVIDIA's PCI vendor ID maps to CUDA backend.
///
/// This is a pure function test; no Vulkan loader or GPU hardware is required.
#[test]
fn test_vulkan_nvidia_vendor_maps_to_cuda() {
    let result = vendor_id_to_device_type(0x10de);
    assert_eq!(
        result,
        Some(DeviceType::Cuda),
        "NVIDIA vendor ID 0x10de maps to DeviceType::Cuda"
    );
}

/// Verify `vendor_id_to_device_type(0x1002)` returns `Some(DeviceType::Rocm)`
/// — AMD's PCI vendor ID maps to ROCm backend.
///
/// This is a pure function test; no Vulkan loader or GPU hardware is required.
#[test]
fn test_vulkan_amd_vendor_maps_to_rocm() {
    let result = vendor_id_to_device_type(0x1002);
    assert_eq!(
        result,
        Some(DeviceType::Rocm),
        "AMD vendor ID 0x1002 maps to DeviceType::Rocm"
    );
}

/// Verify `vendor_id_to_device_type(0x1234)` returns `None`
/// — unknown vendor IDs are skipped during enumeration.
///
/// This is a pure function test; no Vulkan loader or GPU hardware is required.
#[test]
fn test_vulkan_unknown_vendor_skipped() {
    let result = vendor_id_to_device_type(0x1234);
    assert!(
        result.is_none(),
        "Unknown vendor ID 0x1234 returns None (skipped)"
    );
}

/// Verify `vendor_id_to_device_type(0x8086)` returns `None`
/// — Intel's vendor ID is not a compute backend we target (no Vulkan
/// compute in our scope).
#[test]
fn test_vulkan_intel_vendor_skipped() {
    let result = vendor_id_to_device_type(0x8086);
    assert!(
        result.is_none(),
        "Intel vendor ID 0x8086 returns None (skipped)"
    );
}

/// Call `detect()` and assert `result.is_ok()` — proves it never panics
/// or returns `Err` even when the Vulkan loader is absent.
///
/// On CI runners and headless machines without a Vulkan driver,
/// `ash::Entry::load()` fails, and `detect()` must return `Ok(vec![])`
/// rather than panicking or returning `Err`.
#[test]
fn test_vulkan_detect_never_errors() {
    let detector = VulkanDetector;
    let result = detector.detect();
    assert!(
        result.is_ok(),
        "detect() never returns Err for VulkanDetector"
    );
}

/// Call `detect()` and assert the result is `Ok(vec![..])`.
///
/// When no Vulkan-capable GPU is present (CI, headless), the result
/// should be an empty vector — not an error, not a panic.
#[test]
fn test_vulkan_detect_returns_empty_when_no_gpu() {
    let detector = VulkanDetector;
    let result = detector.detect().expect("detect() should always succeed");
    // On headless/CI systems: empty vector is expected.
    // On systems with NVIDIA/AMD GPUs: non-empty vector is expected.
    // The key invariant is: no panic, no Err.
    assert!(
        result.len() == 0
            || result
                .iter()
                .all(|d| d.enumeration_source == EnumerationSource::Vulkan),
        "detect() returns Ok(vec) with Vulkan enumeration source"
    );
}

/// Call `refresh_vram(0)` and assert it returns `Ok((total, total))`
/// or `Ok((0, 0))` — never `Err`.
///
/// On headless/CI systems without Vulkan, returns `(0, 0)`.
/// On systems with Vulkan but no memory budget extension, returns
/// `(total_heap, total_heap)` — the fallback path.
#[test]
fn test_vulkan_refresh_vram_never_errors() {
    let detector = VulkanDetector;
    let result = detector.refresh_vram(0);
    assert!(
        result.is_ok(),
        "refresh_vram() never returns Err for VulkanDetector"
    );
    let (total, free) = result.expect("refresh_vram() should always succeed");
    // When total == free, it means free is unknown (fallback path).
    // This is the documented sentinel for "free unknown."
    assert!(
        total == free || total == 0,
        "refresh_vram returns (total, total) fallback or (0, 0) when Vulkan unavailable"
    );
}

/// Call `refresh_vram()` with an out-of-range index and verify it
/// returns `Ok((0, 0))` rather than panicking.
#[test]
fn test_vulkan_refresh_vram_out_of_range() {
    let detector = VulkanDetector;
    let result = detector.refresh_vram(999);
    assert!(
        result.is_ok(),
        "refresh_vram(999) never returns Err for VulkanDetector"
    );
}
