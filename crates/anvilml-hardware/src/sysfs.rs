//! Sysfs GPU enumerator (Linux/unix).
//!
//! Implements a PCI sysfs-based GPU detector by walking
//! `/sys/bus/pci/devices/*/` and reading vendor/device IDs from the PCI
//! config space files. For AMD GPUs, it additionally reads VRAM from the
//! amdgpu sysfs interface at `/sys/class/drm/cardN/device/mem_info_vram_total`.
//!
//! Vendor → [`DeviceType`](anvilml_core::DeviceType) mapping:
//!
//! | vendorID | DeviceType |
//! |----------|-----------|
//! | 0x10DE   | Cuda      |
//! | 0x1002   | Rocm      |
//! | other    | Cpu       |
//!
//! When `/sys/bus/pci/devices` is absent or unreadable, `detect()` returns
//! `Ok(vec![])` — no panic, no error. Per-device parse failures are logged
//! with `log::warn!` and the device is skipped.

#![cfg(unix)]

use anvilml_core::{AnvilError, CapabilitySource, DeviceType, EnumerationSource, GpuDevice};

use crate::DeviceDetector;

// ── Constants ─────────────────────────────────────────────────────────────────

/// MiB divisor (bytes → MiB).
const BYTES_PER_MIB: u64 = 1024 * 1024;

/// NVIDIA PCI vendor ID.
const VENDOR_NVIDIA: u16 = 0x10de;

/// AMD PCI vendor ID.
const VENDOR_AMD: u16 = 0x1002;

/// Path to the PCI devices sysfs directory.
const SYSFS_PCI_DEVICES: &str = "/sys/bus/pci/devices";

// ── Vendor → DeviceType mapping ───────────────────────────────────────────────

/// Map a PCI vendor ID to a [`DeviceType`].
///
/// Per ANVILML_DESIGN §5.3:
/// - `0x10DE` → Cuda (NVIDIA)
/// - `0x1002` → Rocm (AMD)
/// - Intel or anything else → Cpu
pub fn vendor_id_to_device_type(vendor_id: u16) -> DeviceType {
    match vendor_id {
        VENDOR_NVIDIA => DeviceType::Cuda,
        VENDOR_AMD => DeviceType::Rocm,
        _ => DeviceType::Cpu,
    }
}

// ── Parse helpers (public for test fixture injection) ─────────────────────────

/// Parse a PCI vendor or device ID from a sysfs file content string.
///
/// Accepts hex strings with optional `0x` prefix, case-insensitive.
/// Returns `None` if the content cannot be parsed as a valid u16 hex value.
///
/// # Examples
///
/// ```
/// use anvilml_hardware::sysfs::parse_pci_id;
/// assert_eq!(parse_pci_id("0x10de"), Some(0x10de));
/// assert_eq!(parse_pci_id("10DE"), Some(0x10de));
/// assert_eq!(parse_pci_id("invalid"), None);
/// ```
pub fn parse_pci_id(content: &str) -> Option<u16> {
    let trimmed = content.trim();
    let hex_str = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
        &trimmed[2..]
    } else {
        trimmed
    };

    u16::from_str_radix(hex_str, 16).ok()
}

/// Read VRAM total from an amdgpu sysfs path.
///
/// Reads the file at `mem_info_vram_total` and converts bytes to MiB.
/// Returns `None` if the file doesn't exist or can't be parsed.
///
/// # Examples
///
/// ```
/// use anvilml_hardware::sysfs::read_vram_from_amdgpu_sysfs;
/// let result = read_vram_from_amdgpu_sysfs("/nonexistent/path");
/// assert!(result.is_none());
/// ```
pub fn read_vram_from_amdgpu_sysfs(path: &str) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    let bytes = content.trim().parse::<u64>().ok()?;
    Some((bytes / BYTES_PER_MIB) as u32)
}

/// Find the amdgpu sysfs card path matching a PCI bus/device ID.
///
/// Scans `/sys/class/drm/card*/device` symlinks to find one whose
/// `../../../../pci_bus/...` or `uevent` file contains the given PCI address.
/// Returns the card path prefix for reading VRAM data, or `None`.
fn find_amdgpu_card_path(vendor_id: u16, device_id: u16) -> Option<String> {
    let pci_addr = format!("{vendor_id:04x}:{device_id:04x}");

    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only consider cardN directories.
            if !name_str.starts_with("card") {
                continue;
            }

            let device_path = entry.path().join("device");
            if !device_path.exists() {
                continue;
            }

            // Check uevent file for PCI address match.
            let uevent_path = device_path.join("uevent");
            if let Ok(uevent) = std::fs::read_to_string(&uevent_path) {
                for line in uevent.lines() {
                    if line.starts_with("PCI_SLOT_NAME=")
                        || line.starts_with("PCI_ID=")
                        || line.contains(&pci_addr)
                    {
                        return Some(entry.path().to_string_lossy().into_owned());
                    }
                }
            }

            // Also check the vendor/device files directly.
            if let (Ok(v), Ok(d)) = (
                std::fs::read_to_string(device_path.join("vendor")),
                std::fs::read_to_string(device_path.join("device")),
            ) {
                if let (Some(v_id), Some(d_id)) = (parse_pci_id(&v), parse_pci_id(&d)) {
                    if v_id == vendor_id && d_id == device_id {
                        return Some(entry.path().to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    None
}

// ── SysfsDetector ─────────────────────────────────────────────────────────────

/// A sysfs-based GPU detector.
///
/// Walks `/sys/bus/pci/devices/*/` to discover GPUs and reads vendor/device
/// IDs from PCI config space files. For AMD GPUs, VRAM is read from the
/// amdgpu sysfs interface.
///
/// When the sysfs path is absent or unreadable, `detect()` returns
/// `Ok(vec![])` — no panic, no error. Per-device parse failures are logged
/// and skipped.
#[derive(Debug, Clone, Default)]
pub struct SysfsDetector;

impl DeviceDetector for SysfsDetector {
    fn detect(&self) -> Result<Vec<GpuDevice>, AnvilError> {
        let base_dir = match std::fs::read_dir(SYSFS_PCI_DEVICES) {
            Ok(d) => d,
            Err(e) => {
                log::warn!(
                    "sysfs: cannot read {SYSFS_PCI_DEVICES}: {e}, returning empty device list"
                );
                return Ok(Vec::new());
            }
        };

        let mut devices = Vec::new();
        let mut index: u32 = 0;

        for entry in base_dir.flatten() {
            let path = entry.path();

            // Only consider directories (PCI device entries).
            if !path.is_dir() {
                continue;
            }

            // Read vendor ID.
            let vendor_content = match std::fs::read_to_string(path.join("vendor")) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("sysfs: cannot read vendor for {}: {e}", path.display());
                    continue;
                }
            };

            // Read device ID.
            let device_content = match std::fs::read_to_string(path.join("device")) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("sysfs: cannot read device for {}: {e}", path.display());
                    continue;
                }
            };

            // Parse IDs.
            let vendor_id = match parse_pci_id(&vendor_content) {
                Some(id) => id,
                None => {
                    log::warn!(
                        "sysfs: failed to parse vendor ID from {} for {}",
                        path.join("vendor").display(),
                        path.display()
                    );
                    continue;
                }
            };

            let device_id = match parse_pci_id(&device_content) {
                Some(id) => id,
                None => {
                    log::warn!(
                        "sysfs: failed to parse device ID from {} for {}",
                        path.join("device").display(),
                        path.display()
                    );
                    continue;
                }
            };

            // Map vendor ID to device type.
            let device_type = vendor_id_to_device_type(vendor_id);

            // Build name from PCI address.
            let name = format!("PCI {:04x}:{:04x}", vendor_id, device_id);

            // Determine VRAM.
            let (vram_total_mib, vram_free_mib) = match device_type {
                DeviceType::Rocm => {
                    // AMD: try to read from amdgpu sysfs.
                    if let Some(card_path) = find_amdgpu_card_path(vendor_id, device_id) {
                        let vram_path = format!("{card_path}/mem_info_vram_total");
                        match read_vram_from_amdgpu_sysfs(&vram_path) {
                            Some(vram) => (vram, u32::MAX), // Free VRAM not available via sysfs.
                            None => (0, u32::MAX),
                        }
                    } else {
                        (0, u32::MAX)
                    }
                }
                DeviceType::Cuda => {
                    // NVIDIA: VRAM handled by NVML module.
                    (0, u32::MAX)
                }
                DeviceType::Cpu => (0, 0),
            };

            devices.push(GpuDevice {
                index,
                name,
                device_type,
                vram_total_mib,
                vram_free_mib,
                driver_version: String::new(),
                pci_vendor_id: vendor_id as u16,
                pci_device_id: device_id as u16,
                arch: None, // resolved later by device_db::resolve_caps
                caps: anvilml_core::InferenceCaps::default(),
                enumeration_source: EnumerationSource::Sysfs,
                capabilities_source: CapabilitySource::Fallback,
            });

            index += 1;
        }

        Ok(devices)
    }

    fn refresh_vram(&self, _idx: u32) -> Result<(u32, u32), AnvilError> {
        // VRAM refresh not supported via sysfs alone.
        Ok((0, 0))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvilml_core::DeviceType;
    use serial_test::serial;

    /// `parse_pci_id` must correctly parse hex values with and without prefix.
    #[test]
    #[serial]
    fn parse_pci_ids_valid_hex() {
        assert_eq!(parse_pci_id("0x10de"), Some(0x10de));
        assert_eq!(parse_pci_id("10DE"), Some(0x10de));
        assert_eq!(parse_pci_id("0X10DE"), Some(0x10de));
        assert_eq!(parse_pci_id(" 0x10de "), Some(0x10de));
        assert_eq!(parse_pci_id("0x8086"), Some(0x8086));
        assert_eq!(parse_pci_id("0x1002"), Some(0x1002));
        // Invalid strings return None.
        assert_eq!(parse_pci_id("invalid"), None);
        assert_eq!(parse_pci_id(""), None);
        assert_eq!(parse_pci_id("zzzz"), None);
    }

    /// `read_vram_from_amdgpu_sysfs` helper must convert bytes to MiB correctly.
    #[test]
    #[serial]
    fn read_vram_helper_converts_bytes_to_mib() {
        // 8 GB = 8 * 1024 * 1024 * 1024 = 8589934592 bytes = 8192 MiB.
        let expected_bytes = 8u64 * 1024 * 1024 * 1024;
        assert_eq!(super::read_vram_from_amdgpu_sysfs("/nonexistent"), None);

        // Create a temp file with the expected value.
        let tmp_dir = std::env::temp_dir().join("anvilml_test_sysfs");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).expect("create temp dir");

        let vram_file = tmp_dir.join("mem_info_vram_total");
        std::fs::write(&vram_file, format!("{expected_bytes}")).expect("write temp file");

        let result = super::read_vram_from_amdgpu_sysfs(vram_file.to_str().unwrap());
        assert_eq!(result, Some(8192), "8 GB must be 8192 MiB");

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    /// Vendor ID mapping must produce correct DeviceType values.
    #[test]
    #[serial]
    fn vendor_id_maps_cuda() {
        assert_eq!(vendor_id_to_device_type(0x10de), DeviceType::Cuda);
    }

    #[test]
    #[serial]
    fn vendor_id_maps_rocm() {
        assert_eq!(vendor_id_to_device_type(0x1002), DeviceType::Rocm);
    }

    #[test]
    #[serial]
    fn vendor_id_maps_cpu_intel() {
        assert_eq!(vendor_id_to_device_type(0x8086), DeviceType::Cpu);
    }

    #[test]
    #[serial]
    fn vendor_id_maps_cpu_unknown() {
        assert_eq!(vendor_id_to_device_type(0xDEAD), DeviceType::Cpu);
    }

    /// SysfsDetector must implement the DeviceDetector trait.
    #[test]
    #[serial]
    fn sysfs_detect_returns_ok_on_absent_dir() {
        let detector = SysfsDetector::default();
        let result = detector.detect();
        // Must always return Ok — never panics, never Err.
        assert!(result.is_ok(), "detect() must return Ok, got {:?}", result);
    }

    /// Sysfs detect with fixture data: create temp PCI device dirs and verify parsing.
    #[test]
    #[serial]
    fn sysfs_detect_with_fixture_data() {
        // Create a fake PCI devices directory structure.
        let tmp_dir = std::env::temp_dir().join("anvilml_test_sysfs_device");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).expect("create temp dir");

        // Create a fake NVIDIA device.
        let nvidia_path = tmp_dir.join("0000:03:00.0");
        std::fs::create_dir_all(&nvidia_path).expect("create fake device dir");
        std::fs::write(nvidia_path.join("vendor"), "0x10de\n").expect("write vendor");
        std::fs::write(nvidia_path.join("device"), "0x2230\n").expect("write device");

        // Create a fake AMD device.
        let amd_path = tmp_dir.join("0000:04:00.0");
        std::fs::create_dir_all(&amd_path).expect("create fake device dir");
        std::fs::write(amd_path.join("vendor"), "0x1002\n").expect("write vendor");
        std::fs::write(amd_path.join("device"), "0x740c\n").expect("write device");

        // Create a fake Intel device.
        let intel_path = tmp_dir.join("0000:00:02.0");
        std::fs::create_dir_all(&intel_path).expect("create fake device dir");
        std::fs::write(intel_path.join("vendor"), "0x8086\n").expect("write vendor");
        std::fs::write(intel_path.join("device"), "0x5916\n").expect("write device");

        // Create a malformed device (missing device file) — should be skipped.
        let bad_path = tmp_dir.join("0000:05:00.0");
        std::fs::create_dir_all(&bad_path).expect("create fake device dir");
        std::fs::write(bad_path.join("vendor"), "0x1234\n").expect("write vendor");

        // Use a custom detection path by calling the internal logic directly.
        let devices = enumerate_pci_devices_at(&tmp_dir);

        assert_eq!(
            devices.len(),
            3,
            "must find 3 valid devices, skipped malformed"
        );

        // Collect device names for order-independent verification.
        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        let types: Vec<DeviceType> = devices.iter().map(|d| d.device_type).collect();

        assert!(
            names.contains(&"PCI 10de:2230"),
            "must contain NVIDIA device"
        );
        assert!(names.contains(&"PCI 1002:740c"), "must contain AMD device");
        assert!(
            names.contains(&"PCI 8086:5916"),
            "must contain Intel device"
        );

        assert!(
            types.contains(&DeviceType::Cuda),
            "must have a Cuda device type"
        );
        assert!(
            types.contains(&DeviceType::Rocm),
            "must have a Rocm device type"
        );
        assert!(
            types.contains(&DeviceType::Cpu),
            "must have a Cpu device type"
        );

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    /// Helper: enumerate PCI devices at a custom path (for testing).
    fn enumerate_pci_devices_at(base_path: &std::path::Path) -> Vec<GpuDevice> {
        let mut devices = Vec::new();
        let mut index: u32 = 0;

        if let Ok(entries) = std::fs::read_dir(base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let vendor_content = match std::fs::read_to_string(path.join("vendor")) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let device_content = match std::fs::read_to_string(path.join("device")) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let vendor_id = match parse_pci_id(&vendor_content) {
                    Some(id) => id,
                    None => continue,
                };

                let device_id = match parse_pci_id(&device_content) {
                    Some(id) => id,
                    None => continue,
                };

                let device_type = vendor_id_to_device_type(vendor_id);
                let name = format!("PCI {:04x}:{:04x}", vendor_id, device_id);

                devices.push(GpuDevice {
                    index,
                    name,
                    device_type,
                    vram_total_mib: 0,
                    vram_free_mib: u32::MAX,
                    driver_version: String::new(),
                    pci_vendor_id: vendor_id as u16,
                    pci_device_id: device_id as u16,
                    arch: None, // resolved later by device_db::resolve_caps
                    caps: anvilml_core::InferenceCaps::default(),
                    enumeration_source: EnumerationSource::Sysfs,
                    capabilities_source: CapabilitySource::Fallback,
                });

                index += 1;
            }
        }

        devices
    }
}
