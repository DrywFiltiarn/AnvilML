/// Integration tests for `DxgiDetector` — the Windows DXGI GPU detector.
///
/// These tests cover:
/// - Vendor ID → `DeviceType` mapping (pure function, no Windows API required)
/// - `detect()` graceful degradation — never panics or returns `Err`
/// - `refresh_vram()` returns `(0, 0)` — DXGI has no VRAM query API
#[cfg(target_os = "windows")]
use anvilml_core::types::*;
#[cfg(target_os = "windows")]
use anvilml_hardware::DxgiDetector;
#[cfg(target_os = "windows")]
use anvilml_hardware::detect::DeviceDetector;
#[cfg(target_os = "windows")]
use anvilml_hardware::vendor_id_to_device_type;

/// Verify `vendor_id_to_device_type(0x10de)` returns `Some(DeviceType::Cuda)`
/// — NVIDIA's PCI vendor ID maps to CUDA backend.
///
/// This is a pure function test; no Windows API calls or GPU hardware is required.
/// Same pattern as `test_vulkan_nvidia_vendor_maps_to_cuda`.
#[cfg(target_os = "windows")]
#[test]
fn test_dxgi_nvidia_vendor_maps_to_cuda() {
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
/// This is a pure function test; no Windows API calls or GPU hardware is required.
/// Same pattern as `test_vulkan_amd_vendor_maps_to_rocm`.
#[cfg(target_os = "windows")]
#[test]
fn test_dxgi_amd_vendor_maps_to_rocm() {
    let result = vendor_id_to_device_type(0x1002);
    assert_eq!(
        result,
        Some(DeviceType::Rocm),
        "AMD vendor ID 0x1002 maps to DeviceType::Rocm"
    );
}

/// Call `detect()` and assert `result.is_ok()` — proves it never panics
/// or returns `Err` even when DXGI is unavailable.
///
/// On Windows with GPUs, this returns detected devices; on headless/CI
/// Windows, it returns `Ok(vec![])`. The invariant is: no panic, no `Err`.
#[cfg(target_os = "windows")]
#[test]
fn test_dxgi_detect_never_errors() {
    let detector = DxgiDetector;
    let result = detector.detect();
    assert!(
        result.is_ok(),
        "detect() never returns Err for DxgiDetector"
    );
}

/// Call `refresh_vram(0)` and assert it returns `Ok((0, 0))` —
/// DXGI has no VRAM query API.
///
/// This is the expected behavior: `(0, 0)` signals "unknown" to the caller,
/// consistent with Vulkan's fallback when memory budget is unavailable.
#[cfg(target_os = "windows")]
#[test]
fn test_dxgi_refresh_vram_never_errors() {
    let detector = DxgiDetector;
    let result = detector.refresh_vram(0);
    assert!(
        result.is_ok(),
        "refresh_vram() never returns Err for DxgiDetector"
    );
    assert_eq!(
        result.expect("refresh_vram() should always succeed"),
        (0, 0),
        "DXGI has no VRAM query API, returns (0, 0)"
    );
}
