/// PCI-ID capability table for known GPU devices.
///
/// A hardcoded constant table mapping `(vendor_id, device_id)` tuples to
/// `DeviceRow` structs containing the canonical product name, GPU
/// microarchitecture, and inference capability flags. This table is used
/// as a fallback source of truth for device capabilities before the
/// Python worker reports actual capabilities at startup.
///
/// The table is intentionally small (≤ 20 entries) so that a linear scan
/// is O(1) with no heap allocation — no `HashMap` or external database
/// dependency is needed. If the table grows beyond ~50 entries in a
/// future task, the implementation should switch to a binary search or
/// a compile-time HashMap.
use anvilml_core::{CapabilitySource, GpuDevice};

/// A single row in the PCI-ID device capability database.
///
/// Each row maps a GPU to its canonical product name, microarchitecture,
/// and a subset of inference capabilities that are known from the
/// vendor's public specification. These are pre-spawn hints — the
/// Python worker overwrites them with actual runtime capabilities.
#[derive(Debug, Clone)]
pub struct DeviceRow {
    /// Canonical product name (e.g. `"NVIDIA A100-SXM4-40GB"`).
    /// Empty string means no canonical name is known.
    pub name: &'static str,
    /// GPU microarchitecture (e.g. `"Ampere"`, `"RDNA3"`).
    pub arch: &'static str,
    /// PCI vendor ID (e.g. `0x10de` for NVIDIA, `0x1002` for AMD).
    pub pci_vendor_id: u16,
    /// PCI device ID — unique within the vendor.
    pub pci_device_id: u16,
    /// Whether the device supports FP8 inference per the vendor spec.
    pub fp8: bool,
    /// Whether the device supports FlashAttention per the vendor spec.
    pub flash_attention: bool,
}

/// PCI-ID capability table mapping `(vendor_id, device_id)` tuples to
/// `DeviceRow` capability information.
///
/// Entries cover representative NVIDIA (Ampere, Ada, Hopper), AMD
/// (RDNA2, RDNA3), and Intel Arc GPUs. Each entry is curated from
/// publicly available vendor specification data.
///
/// The table is intentionally kept small (≤ 20 entries) so that the
/// linear scan in `resolve_caps_from_row` is O(1) with no heap
/// allocation or external dependency.
pub const DEVICE_DB: &[DeviceRow] = &[
    // ── NVIDIA Ampere ──────────────────────────────────────────────
    DeviceRow {
        name: "NVIDIA A100-SXM4-40GB",
        arch: "Ampere",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        fp8: true,
        flash_attention: true,
    },
    DeviceRow {
        name: "NVIDIA A100-SXM4-80GB",
        arch: "Ampere",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2230,
        fp8: true,
        flash_attention: true,
    },
    DeviceRow {
        name: "NVIDIA A6000",
        arch: "Ampere",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x20B2,
        fp8: true,
        flash_attention: true,
    },
    // ── NVIDIA Ada ─────────────────────────────────────────────────
    DeviceRow {
        name: "NVIDIA RTX 4090",
        arch: "Ada",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2488,
        fp8: true,
        flash_attention: true,
    },
    DeviceRow {
        name: "NVIDIA H100 PCIe",
        arch: "Hopper",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2704,
        fp8: true,
        flash_attention: true,
    },
    // ── NVIDIA Hopper ──────────────────────────────────────────────
    DeviceRow {
        name: "NVIDIA H100 SXM5",
        arch: "Hopper",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2322,
        fp8: true,
        flash_attention: true,
    },
    // ── AMD RDNA2 ──────────────────────────────────────────────────
    DeviceRow {
        name: "AMD Radeon RX 6900 XT",
        arch: "RDNA2",
        pci_vendor_id: 0x1002,
        pci_device_id: 0x73BF,
        fp8: false,
        flash_attention: true,
    },
    // ── AMD RDNA3 ──────────────────────────────────────────────────
    DeviceRow {
        name: "AMD Radeon RX 7900 XTX",
        arch: "RDNA3",
        pci_vendor_id: 0x1002,
        pci_device_id: 0x74AF,
        fp8: false,
        flash_attention: true,
    },
    DeviceRow {
        name: "AMD Radeon RX 7900 XT",
        arch: "RDNA3",
        pci_vendor_id: 0x1002,
        pci_device_id: 0x74A1,
        fp8: false,
        flash_attention: true,
    },
    // ── Intel Arc ──────────────────────────────────────────────────
    DeviceRow {
        name: "Intel Arc A770",
        arch: "Xe-HPG",
        pci_vendor_id: 0x8086,
        pci_device_id: 0x56A0,
        fp8: false,
        flash_attention: false,
    },
    // ── Additional NVIDIA Ampere (consumer) ────────────────────────
    DeviceRow {
        name: "NVIDIA RTX 3090",
        arch: "Ampere",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        fp8: false,
        flash_attention: true,
    },
    DeviceRow {
        name: "NVIDIA RTX 3080",
        arch: "Ampere",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2206,
        fp8: false,
        flash_attention: true,
    },
    // ── Additional NVIDIA Ada ──────────────────────────────────────
    DeviceRow {
        name: "NVIDIA RTX 4080",
        arch: "Ada",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2484,
        fp8: true,
        flash_attention: true,
    },
    DeviceRow {
        name: "NVIDIA RTX 4070 Ti",
        arch: "Ada",
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2481,
        fp8: true,
        flash_attention: true,
    },
    // ── Additional AMD RDNA3 ───────────────────────────────────────
    DeviceRow {
        name: "AMD Radeon RX 7900 GRE",
        arch: "RDNA3",
        pci_vendor_id: 0x1002,
        pci_device_id: 0x744F,
        fp8: false,
        flash_attention: true,
    },
];

/// Resolve device capabilities from a `DeviceRow` in the PCI-ID
/// capability table.
///
/// If `row` is `Some`, populates `dev.arch`, `dev.caps.fp8`,
/// `dev.caps.flash_attention`, and `dev.name` from the row. Sets
/// `dev.capabilities_source` to `CapabilitySource::DeviceTable`.
///
/// If `row` is `None`, the function performs a linear scan of
/// `DEVICE_DB` matching on `dev.pci_vendor_id` and `dev.pci_device_id`.
/// If a match is found, the same fields are populated. If no match is
/// found, `dev` is left unchanged (arch remains `None`, caps unchanged).
///
/// VRAM fields (`vram_total_mib`, `vram_free_mib`) are never modified
/// by this function — they are set by the detector and left as-is.
///
/// # Arguments
///
/// * `dev` — A mutable reference to the `GpuDevice` to populate.
/// * `row` — An optional pre-matched row. If `Some`, used directly
///   without looking up `DEVICE_DB`. If `None`, the function performs
///   a PCI-ID lookup.
pub fn resolve_caps_from_row(dev: &mut GpuDevice, row: Option<&DeviceRow>) {
    // Resolve the row: use the provided row if given, otherwise look
    // up by PCI IDs from DEVICE_DB. With ≤ 20 entries, linear scan is
    // O(1) and avoids any heap allocation or HashMap dependency.
    let resolved = match row {
        Some(r) => Some(r),
        None => DEVICE_DB.iter().find(|entry| {
            // Match against the device's PCI vendor and device IDs.
            // This lookup is const-compatible and allocation-free.
            entry.pci_vendor_id == dev.pci_vendor_id && entry.pci_device_id == dev.pci_device_id
        }),
    };

    if let Some(row) = resolved {
        // Populate architecture, inference capabilities, and canonical
        // name from the matched device table row.
        dev.arch = Some(row.arch.to_string());
        dev.caps.fp8 = row.fp8;
        dev.caps.flash_attention = row.flash_attention;
        // Overwrite the device name with the canonical name from the
        // table if the row has a non-empty name.
        if !row.name.is_empty() {
            dev.name = row.name.to_string();
        }
        // Mark capabilities as coming from the device table.
        // A future task (P4-A5 or later) will overwrite this to
        // `CapabilitySource::PyTorch` when the Python worker reports
        // actual capabilities at startup.
        dev.capabilities_source = CapabilitySource::DeviceTable;
    }

    // Log the resolution result for both hit and miss paths.
    // This is a DEBUG-level call so it costs nothing at the default
    // INFO log level but is invaluable during diagnosis.
    tracing::debug!(
        vendor_id = dev.pci_vendor_id,
        device_id = dev.pci_device_id,
        source = "device_db",
        found = resolved.is_some(),
        "resolve_caps_from_row"
    );
}
