//! PCI-ID capability table for GPU device resolution.
//!
//! A hardcoded compile-time const-slice mapping (vendor_id, device_id) tuples to
//! model name, architecture string, and ML inference capabilities.
//!
//! Provides [`resolve_caps_from_row`] for populating a [`GpuDevice`] from an
//! [`anvilml_registry::DeviceCapabilityRow`].

use anvilml_core::{CapabilitySource, GpuDevice};

use crate::EnumerationSource;

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

// ── Resolve capabilities from a DeviceCapabilityRow ─────────────────────────

/// Resolve GPU capabilities from an [`anvilml_registry::DeviceCapabilityRow`].
///
/// On a lookup hit, sets `dev.name`, `dev.arch`, `dev.caps`,
/// `dev.enumeration_source`, and `dev.capabilities_source` from the row.
/// On a miss, logs a warning with the vendor + device IDs for table extension tracking,
/// then sets conservative defaults (`caps = InferenceCaps::default()`,
/// `capabilities_source = CapabilitySource::Fallback`).
pub fn resolve_caps_from_row(
    dev: &mut GpuDevice,
    row: Option<&anvilml_registry::DeviceCapabilityRow>,
) {
    match row {
        Some(r) => {
            dev.name = r.model_name.clone();
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
        }
        None => {
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
