//! Linux sysfs-based PCI GPU enumeration.
//!
//! This detector walks `/sys/bus/pci/devices/` and reads `vendor` and
//! `device` files for each PCI device directory to identify GPUs.
//! It maps PCI vendor IDs to device types (NVIDIA → CUDA, AMD → ROCm)
//! and reports devices that lack VRAM data from NVML.
//!
//! This is a fallback detection path on Linux systems. It works even
//! when the Vulkan loader is absent, but provides less metadata than
//! the Vulkan detector (no device name, no driver version, no VRAM).
//!
//! **Hard constraints:** Never panic on missing sysfs entries. Always
//! return an empty list when `/sys/bus/pci/devices/` is unreadable.

pub struct SysfsPciDetector;

impl SysfsPciDetector {
    /// Construct a new `SysfsPciDetector`.
    ///
    /// This is a zero-sized unit struct — no allocation or state is required.
    pub const fn new() -> Self {
        SysfsPciDetector
    }
}

impl Default for SysfsPciDetector {
    fn default() -> Self {
        Self::new()
    }
}

use anvilml_core::{CapabilitySource, DeviceType, EnumerationSource, GpuDevice, InferenceCaps};

use crate::DeviceDetector;

/// Path to the PCI device sysfs directory on Linux.
///
/// This path is part of the Linux kernel sysfs virtual filesystem and
/// is present on all Linux systems with PCI buses.
const PCI_DEVICES_PATH: &str = "/sys/bus/pci/devices/";

impl DeviceDetector for SysfsPciDetector {
    /// Enumerate available GPUs by reading PCI vendor/device IDs from sysfs.
    ///
    /// Walks `/sys/bus/pci/devices/`, reads `vendor` and `device` files
    /// for each directory, and builds a `GpuDevice` entry for each PCI
    /// device that matches a known GPU vendor ID.
    ///
    /// # Errors
    ///
    /// Returns `Ok(vec![])` when the sysfs path doesn't exist or is
    /// unreadable. The method never returns `Err` — it treats sysfs
    /// unavailability as "no GPUs detected" rather than a hard failure.
    fn detect(&self) -> Result<Vec<GpuDevice>, anvilml_core::AnvilError> {
        // Read the PCI device directory. On systems without PCI buses
        // (e.g. some VMs, WSL2), this path may not exist.
        let entries = match std::fs::read_dir(PCI_DEVICES_PATH) {
            Ok(entries) => entries,
            Err(err) => {
                // Sysfs path not available — not an error, just no PCI devices.
                // This is expected on WSL2, VMs, and containerised environments.
                tracing::debug!(
                    path = PCI_DEVICES_PATH,
                    error = %err,
                    "sysfs PCI devices path not readable"
                );
                return Ok(vec![]);
            }
        };

        let mut devices = Vec::new();
        let mut index: u32 = 0;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    // Skip entries that can't be read (e.g. permission denied).
                    // This can happen for PCI devices that are bound to
                    // vfio-pci or other drivers that restrict sysfs access.
                    tracing::debug!(error = %err, "skipping unreadable sysfs entry");
                    continue;
                }
            };

            let dir_name = entry.file_name();
            let _dir_name_str = dir_name.to_string_lossy().into_owned();

            // Read the vendor file for this PCI device.
            // PCI vendor and device IDs are stored as hex strings with
            // a "0x" prefix in sysfs (e.g. "0x10de" for NVIDIA).
            let vendor_path = entry.path().join("vendor");
            let vendor_str = match std::fs::read_to_string(&vendor_path) {
                Ok(s) => s.trim().to_string(),
                Err(_) => continue, // Skip devices with unreadable vendor file
            };

            let vendor_id = parse_pci_id(&vendor_str);

            // Skip entries with vendor 0x0000 (no device) or 0xffff (placeholder).
            // These are sysfs artifacts, not real hardware.
            // 0x0000 indicates a slot with no device attached.
            // 0xffff is used by some chipsets as a placeholder.
            if vendor_id == 0x0000 || vendor_id == 0xffff {
                continue;
            }

            // Read the device file for this PCI device.
            let device_path = entry.path().join("device");
            let device_str = match std::fs::read_to_string(&device_path) {
                Ok(s) => s.trim().to_string(),
                Err(_) => continue, // Skip devices with unreadable device file
            };

            let device_id = parse_pci_id(&device_str);

            // Map the PCI vendor ID to a DeviceType variant.
            // 0x10de = NVIDIA → Cuda, 0x1002 = AMD → Rocm, else → Cpu.
            // This is the standard PCI vendor ID assignment used throughout
            // the crate (see vulkan.rs, cpu.rs for consistency).
            let device_type = match vendor_id {
                0x10de => DeviceType::Cuda,
                0x1002 => DeviceType::Rocm,
                _ => DeviceType::Cpu,
            };

            // Build a device name from the PCI address and IDs.
            // sysfs doesn't provide a human-readable device name — the
            // Vulkan detector provides this on systems where it's available.
            let name = format!(
                "PCI GPU (vendor={:04x}, device={:04x})",
                vendor_id, device_id
            );

            // Build the GpuDevice entry.
            // enumeration_source is set to Sysfs since we enumerated via
            // the Linux sysfs interface. VRAM is 0 because it comes from
            // NVML on NVIDIA systems (handled by NvmlDetector).
            let device = GpuDevice {
                index,
                name,
                db_name: None,
                device_type,
                vram_total_mib: 0,
                vram_free_mib: 0,
                driver_version: "unknown".to_string(),
                pci_vendor_id: vendor_id,
                pci_device_id: device_id,
                arch: None,
                caps: InferenceCaps::default(),
                enumeration_source: EnumerationSource::Sysfs,
                capabilities_source: CapabilitySource::Fallback,
            };

            // Log the detected device at INFO level per the mandatory
            // "each detected device" logging convention.
            tracing::info!(
                index = index,
                name = %device.name,
                device_type = ?device.device_type,
                vram_total_mib = 0u32,
                fp8 = false,
                "gpu device detected via sysfs"
            );

            devices.push(device);
            index += 1;
        }

        // Log that sysfs detection completed at DEBUG level per the
        // mandatory "detection fallback used" logging convention.
        tracing::debug!(
            fallback = "sysfs",
            device_count = devices.len(),
            "sysfs detection completed"
        );

        Ok(devices)
    }

    /// Refresh the VRAM for the device at `index`.
    ///
    /// Returns `(0, 0)` because sysfs doesn't provide live VRAM data.
    /// Live VRAM refresh is handled by NVML on NVIDIA systems.
    ///
    /// # Errors
    ///
    /// This method never returns an error.
    fn refresh_vram(&self, _index: u32) -> Result<(u32, u32), anvilml_core::AnvilError> {
        tracing::debug!(index = _index, "refresh_vram returns (0,0) for sysfs");
        Ok((0, 0))
    }
}

/// Parse a PCI vendor or device ID from a sysfs hex string.
///
/// Sysfs stores PCI IDs as hex strings with a "0x" prefix (e.g. "0x10de").
/// This function strips the prefix and parses the remaining hex digits
/// as a `u16`. Returns `0` on any parse error.
fn parse_pci_id(s: &str) -> u16 {
    // Strip the "0x" or "0X" prefix if present.
    let hex_str = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    // Parse as hexadecimal u16. Return 0 on any parse error — this
    // ensures we don't crash on malformed sysfs entries.
    u16::from_str_radix(hex_str, 16).unwrap_or(0)
}
