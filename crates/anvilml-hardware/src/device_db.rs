//! PCI-ID capability table for GPU device resolution.
//!
//! A hardcoded compile-time const-slice mapping (vendor_id, device_id) tuples to
//! model name, architecture string, and ML inference capabilities.
//!
//! Provides [`resolve_caps_from_row`] for populating a [`GpuDevice`] from an
//! [`anvilml_registry::DeviceCapabilityRow`].

use anvilml_core::{CapabilitySource, GpuDevice};

use crate::EnumerationSource;

/// Return `true` if the given name looks like a generic or placeholder driver string
/// rather than a specific SKU identifier.
///
/// This list is intentionally non-exhaustive — new generic names can be added here
/// as they are discovered.
pub fn is_generic_driver_name(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    // Common AMD Radeon driver fallback names.
    if name == "AMD Radeon Graphics" || name == "AMD proprietary driver" {
        return true;
    }
    // PCI enumeration placeholder pattern (e.g. "Device 1002:687F").
    const PREFIX: &str = "Device ";
    if name.len() > PREFIX.len() && name.starts_with(PREFIX) {
        let hex_part = &name[PREFIX.len()..];
        // Check that the remaining string looks like hex (contains at least one non-hex char
        // separator like ':' and valid hex digits around it).
        if let Some(colon_pos) = hex_part.find(':') {
            let before = &hex_part[..colon_pos];
            let after = &hex_part[colon_pos + 1..];
            if !before.is_empty()
                && !after.is_empty()
                && before.chars().all(|c| c.is_ascii_hexdigit())
                && after.chars().all(|c| c.is_ascii_hexdigit())
            {
                return true;
            }
        }
    }
    false
}

/// Resolve GPU capabilities from an [`anvilml_registry::DeviceCapabilityRow`].
///
/// On a lookup hit, sets `dev.name`, `dev.arch`, `dev.caps`,
/// `dev.enumeration_source`, and `dev.capabilities_source` from the row.
/// If the device already has a specific (non-generic) name, that name is preserved
/// and the DB model name is stored in `db_group_name` for display purposes.
/// On a miss, logs a warning with the vendor + device IDs for table extension tracking,
/// then sets conservative defaults (`caps = InferenceCaps::default()`,
/// `capabilities_source = CapabilitySource::Fallback`).
pub fn resolve_caps_from_row(
    dev: &mut GpuDevice,
    row: Option<&anvilml_registry::DeviceCapabilityRow>,
) {
    match row {
        Some(r) => {
            if dev.name.is_empty() || is_generic_driver_name(&dev.name) {
                // Generic or empty driver name — replace with the DB model name.
                dev.name = r.model_name.clone();
                dev.db_group_name = None;
            } else {
                // Specific Vulkan SKU name — preserve it, store DB label for display.
                dev.db_group_name = Some(r.model_name.clone());
            }
            dev.arch = Some(r.arch.clone());
            dev.caps = anvilml_core::InferenceCaps {
                fp32: r.fp32,
                fp16: r.fp16,
                bf16: r.bf16,
                fp8: r.fp8,
                fp4: r.fp4,
                nvfp4: r.nvfp4,
                flash_attention: r.flash_attn,
            };
            dev.capabilities_source = CapabilitySource::DeviceTable;
            dev.enumeration_source = EnumerationSource::DeviceTable;
            tracing::debug!(
                vendor_id = %format_args!("0x{:04X}", dev.pci_vendor_id),
                device_id = %format_args!("0x{:04X}", dev.pci_device_id),
                name = %dev.name,
                source = "DeviceTable",
                "caps resolved"
            );
        }
        None => {
            if dev.name.is_empty() || is_generic_driver_name(&dev.name) {
                dev.name = format!(
                    "Unknown GPU (0x{:04X}:0x{:04X})",
                    dev.pci_vendor_id, dev.pci_device_id
                );
                dev.db_group_name = None;
            }
            tracing::debug!(
                vendor_id = %format_args!("0x{:04X}", dev.pci_vendor_id),
                device_id = %format_args!("0x{:04X}", dev.pci_device_id),
                name = %dev.name,
                source = "Fallback",
                "caps resolved"
            );
            // If the device already has a specific name, keep it unchanged.
            tracing::warn!(
                detector = "DeviceDB",
                vendor_id = %format_args!("0x{:04X}", dev.pci_vendor_id),
                device_id = %format_args!("0x{:04X}", dev.pci_device_id),
                "unknown PCI ID — add to SUPPORTED_DEVICES_DB.md"
            );
            dev.caps = anvilml_core::InferenceCaps::default();
            dev.capabilities_source = CapabilitySource::Fallback;
        }
    }
}

/// A single GPU capability entry keyed by PCI vendor + device ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCapabilityEntry {
    /// PCI vendor ID (e.g. 0x10DE for NVIDIA, 0x1002 for AMD).
    pub vendor_id: u16,
    /// PCI device ID within the vendor.
    pub device_id: u16,
    /// Human-readable model name (e.g. "NVIDIA GeForce RTX 3090").
    pub model_name: &'static str,
    /// Architecture identifier — CUDA SM version ("8.6") or AMD gfx ("gfx1100").
    pub arch: &'static str,
    /// Whether the device supports FP32 (single-precision) inference via TF32 tensor cores.
    pub fp32: bool,
    /// Whether the device supports FP16 (half-precision) inference.
    pub fp16: bool,
    /// Whether the device supports BF16 (bfloat16) inference.
    pub bf16: bool,
    /// Whether the device supports FP8 inference.
    pub fp8: bool,
    /// Whether the device supports FP4 inference.
    pub fp4: bool,
    /// Whether the device supports NVIDIA FP4 (NVFP4) inference.
    pub nvfp4: bool,
    /// Whether the device supports Flash Attention.
    pub flash_attention: bool,
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use anvilml_core::{DeviceType, InferenceCaps};

    fn make_device(name: &str, vid: u16, did: u16) -> GpuDevice {
        GpuDevice {
            index: 0,
            name: name.to_string(),
            device_type: DeviceType::Cpu,
            vram_total_mib: 0,
            vram_free_mib: 0,
            driver_version: String::new(),
            pci_vendor_id: vid,
            pci_device_id: did,
            arch: None,
            caps: InferenceCaps::default(),
            enumeration_source: EnumerationSource::Fallback,
            capabilities_source: CapabilitySource::Fallback,
            db_group_name: None,
        }
    }

    fn make_row(model: &str, arch: &str) -> anvilml_registry::DeviceCapabilityRow {
        anvilml_registry::DeviceCapabilityRow {
            vendor_id: 0x10DE,
            device_id: 0x20B0,
            model_name: model.to_string(),
            arch: arch.to_string(),
            fp32: false,
            fp16: true,
            bf16: true,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attn: true,
        }
    }

    /// A device with an empty or generic driver name should have its name
    /// replaced by the database model name on a hit; `db_group_name` stays None.
    #[test]
    fn generic_name_replaced_by_group_label() {
        let mut dev = make_device("", 0x10DE, 0x20B0);
        let row = make_row("NVIDIA GeForce RTX 4090", "8.9");
        resolve_caps_from_row(&mut dev, Some(&row));

        assert_eq!(dev.name, "NVIDIA GeForce RTX 4090");
        assert_eq!(dev.db_group_name, None);
        assert!(matches!(
            dev.enumeration_source,
            EnumerationSource::DeviceTable
        ));
        assert!(matches!(
            dev.capabilities_source,
            CapabilitySource::DeviceTable
        ));
    }

    /// A device with a specific Vulkan SKU name should preserve that name on a hit;
    /// the database model name is stored in `db_group_name`.
    #[test]
    fn specific_vulkan_name_preserved() {
        let mut dev = make_device("AMD Radeon RX 7900 XTX", 0x10DE, 0x20B0);
        let row = make_row("Navi 31 XL", "gfx1100");
        resolve_caps_from_row(&mut dev, Some(&row));

        assert_eq!(dev.name, "AMD Radeon RX 7900 XTX");
        assert_eq!(dev.db_group_name, Some("Navi 31 XL".to_string()));
    }

    /// A device with an empty name that misses the DB should get a fallback
    /// "Unknown GPU (0x...)" name.
    #[test]
    fn miss_with_empty_name_shows_unknown() {
        let mut dev = make_device("", 0x1234, 0x5678);
        resolve_caps_from_row(&mut dev, None);

        assert_eq!(dev.name, "Unknown GPU (0x1234:0x5678)");
        assert_eq!(dev.db_group_name, None);
        assert!(matches!(
            dev.capabilities_source,
            CapabilitySource::Fallback
        ));
    }

    /// A device with a specific name that misses the DB should keep its existing name.
    #[test]
    fn miss_with_specific_name_preserved() {
        let mut dev = make_device("NVIDIA GeForce RTX 3080", 0x1234, 0x5678);
        resolve_caps_from_row(&mut dev, None);

        assert_eq!(dev.name, "NVIDIA GeForce RTX 3080");
        assert!(matches!(
            dev.capabilities_source,
            CapabilitySource::Fallback
        ));
    }
}
