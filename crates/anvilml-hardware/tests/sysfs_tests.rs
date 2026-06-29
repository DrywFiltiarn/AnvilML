/// Integration tests for `SysfsPciDetector` — the Linux sysfs PCI GPU detector.
///
/// These tests cover:
/// - Synthetic sysfs tree parsing — a temp-dir-mocked tree with display-class
///   devices parses correctly into `GpuDevice` structs.
/// - Class filtering — non-display devices are excluded.
/// - Vendor ID mapping — NVIDIA (0x10de → Cuda) and AMD (0x1002 → Rocm)
///   map correctly via the shared `vendor_id_to_device_type()` function.
/// - `detect()` graceful degradation — never panics or returns `Err`.
/// - `refresh_vram()` returns `(0, 0)` — sysfs has no VRAM query API.
#[cfg(target_os = "linux")]
use anvilml_core::types::*;
#[cfg(target_os = "linux")]
use anvilml_hardware::detect::DeviceDetector;
#[cfg(target_os = "linux")]
use anvilml_hardware::{SysfsPciDetector, detect_from_path};
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

/// Create a synthetic sysfs device directory within a temp parent directory.
///
/// Writes `vendor`, `device`, and `class` files into `{parent}/{device_name}/`.
/// Returns the path to the device directory on success.
#[cfg(target_os = "linux")]
fn create_synthetic_device(
    parent: &PathBuf,
    device_name: &str,
    vendor: &str,
    device_id: &str,
    class: &str,
) -> PathBuf {
    let device_dir = parent.join(device_name);
    fs::create_dir_all(&device_dir).expect("failed to create device dir");
    fs::write(device_dir.join("vendor"), vendor).expect("failed to write vendor file");
    fs::write(device_dir.join("device"), device_id).expect("failed to write device file");
    fs::write(device_dir.join("class"), class).expect("failed to write class file");
    device_dir
}

/// Call `detect_from_path` with a nonexistent path and assert it returns
/// `Ok(vec![])` — proves the detector handles missing sysfs gracefully
/// without panicking or returning `Err`.
///
/// This is the "missing path" test: on systems without `/sys/bus/pci/devices/`
/// (or in a test with an intentionally bogus path), the detector returns
/// an empty vector. The invariant is: no panic, no `Err`.
#[cfg(target_os = "linux")]
#[test]
fn test_sysfs_detect_missing_path_returns_empty() {
    let result = detect_from_path(PathBuf::from("/nonexistent/sysfs/path").as_path());
    assert!(
        result.is_ok(),
        "detect_from_path returns Ok even for nonexistent path"
    );
    assert!(
        result.expect("should be Ok").is_empty(),
        "detect_from_path returns empty vec for nonexistent path"
    );
}

/// Create a temp-dir-mocked sysfs tree with one synthetic AMD display-class
/// device (vendor=0x1002, device=0x2204, class=0x030000) and assert it
/// parses correctly into a `GpuDevice` with `DeviceType::Rocm`.
///
/// This tests the full detection pipeline: class filtering, vendor parsing,
/// device type mapping, and `GpuDevice` construction — all from a temp dir
/// rather than the real sysfs mount.
#[cfg(target_os = "linux")]
#[test]
fn test_sysfs_detect_synthetic_display_device() {
    let temp_dir = std::env::temp_dir().join("anvilml_sysfs_test");
    // Clean up any previous test run to ensure isolation.
    let _ = fs::remove_dir_all(&temp_dir);

    // Create synthetic device: AMD Radeon RX 6800 (vendor 0x1002, device 0x2204)
    create_synthetic_device(&temp_dir, "0000:01:00.0", "0x1002", "0x2204", "0x030000");

    let result = detect_from_path(&temp_dir);
    assert!(result.is_ok(), "detect_from_path should succeed");
    let devices = result.expect("should be Ok");
    assert_eq!(
        devices.len(),
        1,
        "exactly one device detected from synthetic tree"
    );

    let device = &devices[0];
    assert_eq!(
        device.enumeration_source,
        EnumerationSource::Sysfs,
        "enumeration_source is Sysfs"
    );
    assert_eq!(
        device.device_type,
        DeviceType::Rocm,
        "AMD vendor ID maps to Rocm"
    );
    assert_eq!(device.vram_total_mib, 0, "sysfs has no VRAM info");
    assert_eq!(device.vram_free_mib, 0, "sysfs has no VRAM info");
    assert_eq!(device.driver_version, "n/a", "no driver version from sysfs");

    // Clean up temp dir.
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Create a temp-dir-mocked sysfs tree with a non-display-class device
/// (class=0x020000, network controller) and assert the device is filtered
/// out — the result should be empty.
///
/// This verifies that the class prefix `0x03` filter correctly excludes
/// non-GPU PCI devices that appear in the sysfs directory.
#[cfg(target_os = "linux")]
#[test]
fn test_sysfs_filter_non_display_class() {
    let temp_dir = std::env::temp_dir().join("anvilml_sysfs_test_nic");
    let _ = fs::remove_dir_all(&temp_dir);

    // Create synthetic device: network controller (class 0x020000)
    // with an NVIDIA vendor ID — should be filtered by class, not vendor.
    create_synthetic_device(&temp_dir, "0000:02:00.0", "0x10de", "0x1000", "0x020000");

    let result = detect_from_path(&temp_dir);
    assert!(result.is_ok(), "detect_from_path should succeed");
    let devices = result.expect("should be Ok");
    assert!(
        devices.is_empty(),
        "non-display-class device should be filtered out"
    );

    // Clean up temp dir.
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Create a temp-dir-mocked sysfs tree with one synthetic NVIDIA display-class
/// device (vendor=0x10de, class=0x030000) and assert the device type maps
/// to `DeviceType::Cuda`.
///
/// This verifies the vendor ID → `DeviceType` mapping for NVIDIA, ensuring
/// the shared `vendor_id_to_device_type()` function is used consistently
/// across the sysfs detector.
#[cfg(target_os = "linux")]
#[test]
fn test_sysfs_detect_nvidia_vendor() {
    let temp_dir = std::env::temp_dir().join("anvilml_sysfs_test_nvidia");
    let _ = fs::remove_dir_all(&temp_dir);

    // Create synthetic device: NVIDIA RTX 4090 (vendor 0x10de, device 0x2324)
    create_synthetic_device(&temp_dir, "0000:03:00.0", "0x10de", "0x2324", "0x030000");

    let result = detect_from_path(&temp_dir);
    assert!(result.is_ok(), "detect_from_path should succeed");
    let devices = result.expect("should be Ok");
    assert_eq!(
        devices.len(),
        1,
        "exactly one device detected from synthetic tree"
    );

    let device = &devices[0];
    assert_eq!(
        device.device_type,
        DeviceType::Cuda,
        "NVIDIA vendor ID 0x10de maps to Cuda"
    );
    assert_eq!(
        device.enumeration_source,
        EnumerationSource::Sysfs,
        "enumeration_source is Sysfs"
    );

    // Clean up temp dir.
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Call `detect()` on `SysfsPciDetector` and assert `result.is_ok()` —
/// proves the detector never panics or returns `Err` even when `/sys`
/// is absent or unreadable.
///
/// On systems with `/sys/bus/pci/devices/`, this may return real devices;
/// on headless/CI systems, it returns `Ok(vec![])`. The invariant is:
/// no panic, no `Err`.
#[cfg(target_os = "linux")]
#[test]
fn test_sysfs_detect_never_errors() {
    let detector = SysfsPciDetector;
    let result = detector.detect();
    assert!(
        result.is_ok(),
        "detect() never returns Err for SysfsPciDetector"
    );
}

/// Call `refresh_vram(0)` and assert it returns `Ok((0, 0))` —
/// sysfs has no VRAM query API.
///
/// This is the expected behavior: `(0, 0)` signals "unknown" to the caller,
/// consistent with `DxgiDetector`'s approach.
#[cfg(target_os = "linux")]
#[test]
fn test_sysfs_refresh_vram_returns_zero() {
    let detector = SysfsPciDetector;
    let result = detector.refresh_vram(0);
    assert!(result.is_ok(), "refresh_vram() never returns Err");
    assert_eq!(
        result.expect("refresh_vram() should always succeed"),
        (0, 0),
        "sysfs has no VRAM query API, returns (0, 0)"
    );
}

/// Create a temp-dir-mocked sysfs tree with multiple devices (one display,
/// one network, one audio) and assert only the display device is returned.
///
/// This tests the multi-device filtering scenario: the real sysfs directory
/// contains many non-GPU PCI devices, and the detector must correctly
/// filter to only display controllers.
#[cfg(target_os = "linux")]
#[test]
fn test_sysfs_multi_device_filter() {
    let temp_dir = std::env::temp_dir().join("anvilml_sysfs_test_multi");
    let _ = fs::remove_dir_all(&temp_dir);

    // Device 1: AMD display controller (should be included)
    create_synthetic_device(&temp_dir, "0000:01:00.0", "0x1002", "0x2204", "0x030000");

    // Device 2: NVIDIA network controller (should be filtered by class)
    create_synthetic_device(&temp_dir, "0000:02:00.0", "0x10de", "0x1000", "0x020000");

    // Device 3: Intel audio controller (should be filtered by class)
    // Intel is also unknown to vendor_id_to_device_type, so it would be
    // filtered either way — but the class filter is the primary gate.
    create_synthetic_device(&temp_dir, "0000:03:00.1", "0x8086", "0x22c8", "0x040300");

    let result = detect_from_path(&temp_dir);
    assert!(result.is_ok(), "detect_from_path should succeed");
    let devices = result.expect("should be Ok");

    // Only the AMD display device should be included.
    assert_eq!(
        devices.len(),
        1,
        "only display-class device should be included (expected 1, got {})",
        devices.len()
    );
    assert_eq!(
        devices[0].device_type,
        DeviceType::Rocm,
        "the included device is AMD/Rocm"
    );

    // Clean up temp dir.
    let _ = fs::remove_dir_all(&temp_dir);
}
