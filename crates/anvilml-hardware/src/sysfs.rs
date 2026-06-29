/// Linux sysfs PCI enumeration detector — fallback GPU discovery on Linux.
///
/// Reads PCI config space files from `/sys/bus/pci/devices/` to detect
/// display controllers (class prefix `0x03`). Maps vendor IDs to
/// `DeviceType` via the shared `vendor_id_to_device_type()` function,
/// ensuring consistent mapping across all three real/fallback detectors
/// (Vulkan, DXGI, sysfs).
///
/// This detector is the Linux fallback when Vulkan enumeration returns
/// empty — per §6.4 of the design doc, the detection priority chain is:
/// Vulkan → sysfs (Linux) / DXGI (Windows) → CPU.
///
/// # Error resilience
///
/// `detect()` **never** panics and **never** returns `Err`. If the sysfs
/// path does not exist or is not a directory, the function returns
/// `Ok(vec![])`. Each device's config files are individually guarded —
/// a missing or unreadable file for one device skips that device only.
pub struct SysfsPciDetector;

use crate::detect::DeviceDetector;
use crate::vendor_id_to_device_type;
use anvilml_core::{AnvilError, CapabilitySource, EnumerationSource, GpuDevice, InferenceCaps};
use std::fs;
use std::path::Path;

/// Enumerate Linux PCI display controllers from a sysfs base directory.
///
/// Reads `{vendor,device,class}` files within each device subdirectory,
/// filters by class prefix `0x03` (display controller per PCI class code
/// spec), and maps vendor IDs to `DeviceType`.
///
/// # Parameters
///
/// * `base_path` — The sysfs PCI devices directory (e.g.
///   `/sys/bus/pci/devices`). Passing an alternate path enables
///   test coverage without requiring a real sysfs mount.
///
/// # Returns
///
/// A vector of `GpuDevice` structs for each detected display controller.
/// Returns `Ok(vec![])` if the base path does not exist, is not a
/// directory, or contains no display-class devices — never returns
/// `Err` and never panics.
///
/// This function is `pub` to allow integration tests to construct
/// synthetic sysfs trees in temp directories and exercise the full
/// detection pipeline without requiring a real `/sys` mount.
pub fn detect_from_path(base_path: &Path) -> Result<Vec<GpuDevice>, AnvilError> {
    // Open the sysfs PCI devices directory. If the path does not exist
    // or is not a directory (IO error), return Ok(vec![]) — per the
    // "detection never panics" rule, sysfs absence is not an error
    // condition; it just means this detector has nothing to report.
    let entries = match fs::read_dir(base_path) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!(
                path = %base_path.display(),
                error = %e,
                "sysfs PCI devices directory not accessible"
            );
            return Ok(vec![]);
        }
    };

    let mut devices = Vec::new();
    let mut index: u32 = 0;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                // Skip entries that could not be read (e.g. permission
                // denied on a device directory). Log at DEBUG and continue.
                tracing::debug!(error = %e, "skipping unreadable sysfs entry");
                continue;
            }
        };

        let device_path = entry.path();

        // Only process directories — `/sys/bus/pci/devices/` may
        // contain non-directory entries (symlinks, files) that we
        // should skip.
        if !device_path.is_dir() {
            continue;
        }

        // Read the class file first — this is the filter. We check
        // if the class prefix is 0x03 (display controller) before
        // reading the more expensive vendor/device files.
        let class_path = device_path.join("class");
        let class_str = match fs::read_to_string(&class_path) {
            Ok(s) => s,
            Err(e) => {
                // Class file missing or unreadable — skip this device.
                // This is common for non-display PCI devices that still
                // appear in the directory listing.
                tracing::debug!(
                    device = %device_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                    error = %e,
                    "skipping device without readable class file"
                );
                continue;
            }
        };

        // Strip the "0x" prefix if present, then check if the high
        // byte (first two hex digits) equals "03" for display
        // controller. The PCI class code spec defines class code
        // 0x03 as "Display controller" — the low bytes are subclass
        // and programming interface which vary by vendor.
        let class_trimmed = class_str.trim().trim_start_matches("0x");
        let high_byte = &class_trimmed[..2.min(class_trimmed.len())];
        if high_byte != "03" {
            // Not a display controller — skip. This is the normal
            // case; most PCI devices in the directory are not GPUs.
            continue;
        }

        // Read the vendor file and parse as hex to u16. If missing
        // or malformed, skip this device.
        let vendor_path = device_path.join("vendor");
        let vendor_str = match fs::read_to_string(&vendor_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(
                    device = %device_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                    error = %e,
                    "skipping device without readable vendor file"
                );
                continue;
            }
        };
        let vendor_id = match u32::from_str_radix(vendor_str.trim().trim_start_matches("0x"), 16) {
            Ok(id) => id,
            Err(e) => {
                tracing::debug!(
                    device = %device_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                    error = %e,
                    "skipping device with unparseable vendor ID"
                );
                continue;
            }
        };

        // Read the device file and parse as hex to u16. If missing
        // or malformed, skip this device.
        let device_file_path = device_path.join("device");
        let device_str = match fs::read_to_string(&device_file_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(
                    device = %device_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                    error = %e,
                    "skipping device without readable device file"
                );
                continue;
            }
        };
        let pci_device_id = match u32::from_str_radix(
            device_str.trim().trim_start_matches("0x"),
            16,
        ) {
            Ok(id) => id,
            Err(e) => {
                tracing::debug!(
                    device = %device_path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                    error = %e,
                    "skipping device with unparseable device ID"
                );
                continue;
            }
        };

        // Map vendor ID to device type using the shared function.
        // This ensures identical mapping across all three real/fallback
        // detectors (Vulkan, DXGI, sysfs), preventing inconsistent
        // results in Phase 5's priority chain. Unknown vendors are
        // skipped — we only care about NVIDIA (CUDA) and AMD (ROCm).
        let device_type = match vendor_id_to_device_type(vendor_id) {
            Some(dt) => dt,
            None => {
                tracing::debug!(vendor_id = vendor_id, "skipping unknown PCI vendor");
                continue;
            }
        };

        // Construct the device name from the directory entry name
        // (e.g. "0000:01:00.0"). If the file name is not available,
        // fall back to "PCI Device".
        let name = device_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "PCI Device".into());

        let device = GpuDevice {
            index,
            name,
            device_type,
            vram_total_mib: 0, // sysfs PCI config space has no VRAM query API
            vram_free_mib: 0,
            driver_version: "n/a".into(), // sysfs doesn't expose driver version
            pci_vendor_id: (vendor_id & 0xFFFF) as u16,
            pci_device_id: (pci_device_id & 0xFFFF) as u16,
            arch: None, // sysfs doesn't expose architecture string directly
            caps: InferenceCaps::default(), // pre-spawn hint
            enumeration_source: EnumerationSource::Sysfs,
            capabilities_source: CapabilitySource::Fallback,
        };

        tracing::debug!(
            vendor_id = vendor_id,
            pci_device_id = pci_device_id,
            device_name = %device.name,
            index = index,
            "detected GPU via sysfs"
        );

        devices.push(device);
        index += 1;
    }

    Ok(devices)
}

impl DeviceDetector for SysfsPciDetector {
    /// Enumerate Linux PCI display controllers from `/sys/bus/pci/devices/`.
    ///
    /// Reads vendor/device/class PCI config space files, filters for
    /// display controllers (class prefix 0x03), maps vendor IDs to
    /// `DeviceType`, and constructs `GpuDevice` structs with
    /// `enumeration_source: EnumerationSource::Sysfs`.
    ///
    /// Returns `Ok(vec![])` if `/sys/bus/pci/devices/` does not exist
    /// or is not readable — never returns `Err` and never panics.
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        // Hard-coded sysfs PCI devices path — this is the standard
        // location on Linux kernels that expose PCI config space.
        // The path is immutable in the kernel ABI.
        detect_from_path(Path::new("/sys/bus/pci/devices"))
    }

    /// Refresh VRAM totals for a device by its index.
    ///
    /// Returns `Ok((0, 0))` — sysfs PCI config space does not expose
    /// VRAM information. This is consistent with `DxgiDetector`'s
    /// approach and signals "unknown" to the caller.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), AnvilError> {
        // sysfs PCI config space has no VRAM query API — return
        // (0, 0) as the "unknown" sentinel. This matches the DXGI
        // fallback and the Vulkan fallback when memory budget is
        // unavailable.
        tracing::debug!("sysfs has no VRAM query API, returning (0, 0)");
        Ok((0, 0))
    }
}
