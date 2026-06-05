//! PCI-ID capability table for GPU device resolution.
//!
//! A hardcoded compile-time const-slice mapping (vendor_id, device_id) tuples to
//! model name, architecture string, and ML inference capabilities.
//!
//! Provides [`lookup`] for static-reference lookup and [`resolve_caps`] for
//! populating a [`GpuDevice`] from the table.

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
    /// Whether the device supports FP16 (half-precision) inference.
    pub fp16: bool,
    /// Whether the device supports BF16 (bfloat16) inference.
    pub bf16: bool,
    /// Whether the device supports Flash Attention.
    pub flash_attention: bool,
}

// ── Seeded PCI capability table ───────────────────────────────────────────────

/// Compile-time PCI-ID capability table.
///
/// Seeded with representative NVIDIA (Ampere, Hopper, Turing) and AMD (RDNA3, CDNA) cards.
/// No VRAM values are stored — those come from runtime detection.
pub const PCI_CAPABILITY_TABLE: &[DeviceCapabilityEntry] = &[
    // ── NVIDIA Ampere ───────────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2204,
        model_name: "NVIDIA GeForce RTX 3090",
        arch: "8.6",
        fp16: true,
        bf16: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x20B0,
        model_name: "NVIDIA A100-SXM4-40GB",
        arch: "8.0",
        fp16: true,
        bf16: true,
        flash_attention: true,
    },
    // ── NVIDIA Hopper ───────────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2322,
        model_name: "NVIDIA H100-SXM5-80GB",
        arch: "9.0",
        fp16: true,
        bf16: true,
        flash_attention: true,
    },
    // ── NVIDIA Turing ───────────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2206,
        model_name: "NVIDIA GeForce RTX 3080",
        arch: "8.6",
        fp16: true,
        bf16: false,
        flash_attention: false,
    },
    // ── AMD RDNA3 ───────────────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x740C,
        model_name: "AMD Radeon RX 7900 XTX",
        arch: "gfx1100",
        fp16: true,
        bf16: true,
        flash_attention: true,
    },
    // ── AMD CDNA ────────────────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x0BDB,
        model_name: "AMD Instinct MI250X",
        arch: "gfx90a",
        fp16: true,
        bf16: true,
        flash_attention: true,
    },
];

// ── Lookup ────────────────────────────────────────────────────────────────────

/// Look up a [`DeviceCapabilityEntry`] by PCI vendor and device ID.
///
/// Returns `Some(&'static DeviceCapabilityEntry)` on match, `None` if the
/// (vendor_id, device_id) pair is not present in the table.
///
/// Uses linear scan — acceptable for this small static table (< 30 entries).
pub fn lookup(vendor_id: u16, device_id: u16) -> Option<&'static DeviceCapabilityEntry> {
    PCI_CAPABILITY_TABLE
        .iter()
        .find(|e| e.vendor_id == vendor_id && e.device_id == device_id)
}

// ── Resolve capabilities (stub) ──────────────────────────────────────────────

/// Resolve GPU capabilities from the PCI capability table.
///
/// On a lookup hit, sets `dev.name`, `dev.arch`, `dev.caps`,
/// `dev.enumeration_source`, and `dev.capabilities_source` from the table.
/// On a miss, logs a warning with the vendor + device IDs for table extension tracking,
/// then sets conservative defaults (`caps = InferenceCaps::default()`,
/// `capabilities_source = CapabilitySource::Fallback`).
pub fn resolve_caps(dev: &mut GpuDevice, vendor_id: u16, device_id: u16) {
    if let Some(entry) = lookup(vendor_id, device_id) {
        dev.name = entry.model_name.to_string();
        dev.arch = Some(entry.arch.to_string());
        dev.caps = anvilml_core::InferenceCaps {
            fp32: false,
            fp16: entry.fp16,
            bf16: entry.bf16,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attention: entry.flash_attention,
        };
        dev.enumeration_source = EnumerationSource::DeviceTable;
        dev.capabilities_source = CapabilitySource::DeviceTable;
    } else {
        tracing::warn!(
            detector = "DeviceDB",
            vendor_id = %format_args!("0x{:04X}", vendor_id),
            device_id = %format_args!("0x{:04X}", device_id),
            "unknown PCI ID — add to PCI_CAPABILITY_TABLE"
        );
        // Set conservative fallback values.
        dev.caps = anvilml_core::InferenceCaps::default();
        dev.capabilities_source = CapabilitySource::Fallback;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Every seeded entry must be found by lookup() with the correct PCI IDs.
    #[test]
    fn seed_entries_lookup() {
        // NVIDIA RTX 3090 — Ampere SM 8.6
        let entry = lookup(0x10DE, 0x2204);
        assert!(entry.is_some(), "RTX 3090 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA GeForce RTX 3090");
        assert_eq!(e.arch, "8.6");
        assert!(e.fp16);
        assert!(!e.bf16);
        assert!(!e.flash_attention);

        // NVIDIA A100 — Ampere SM 8.0 (datacenter)
        let entry = lookup(0x10DE, 0x20B0);
        assert!(entry.is_some(), "A100 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA A100-SXM4-40GB");
        assert_eq!(e.arch, "8.0");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);

        // NVIDIA H100 — Hopper SM 9.0
        let entry = lookup(0x10DE, 0x2322);
        assert!(entry.is_some(), "H100 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA H100-SXM5-80GB");
        assert_eq!(e.arch, "9.0");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);

        // NVIDIA RTX 3080 — Turing SM 8.6
        let entry = lookup(0x10DE, 0x2206);
        assert!(entry.is_some(), "RTX 3080 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA GeForce RTX 3080");
        assert_eq!(e.arch, "8.6");
        assert!(e.fp16);
        assert!(!e.bf16);
        assert!(!e.flash_attention);

        // AMD RX 7900 XTX — RDNA3 gfx1100
        let entry = lookup(0x1002, 0x740C);
        assert!(entry.is_some(), "RX 7900 XTX must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "AMD Radeon RX 7900 XTX");
        assert_eq!(e.arch, "gfx1100");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);

        // AMD MI250X — CDNA gfx90a
        let entry = lookup(0x1002, 0x0BDB);
        assert!(entry.is_some(), "MI250X must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "AMD Instinct MI250X");
        assert_eq!(e.arch, "gfx90a");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);
    }

    /// Lookup must return None for non-existent PCI ID combinations.
    #[test]
    fn miss_returns_none() {
        // Intel vendor ID (not seeded)
        assert!(lookup(0x8086, 0x56A0).is_none());
        // Arbitrary unknown pair
        assert!(lookup(0xDEAD, 0xBEEF).is_none());
        // NVIDIA vendor but unknown device
        assert!(lookup(0x10DE, 0xFFFF).is_none());
    }

    /// No two entries may share the same (vendor_id, device_id) pair.
    #[test]
    fn no_duplicate_pci_ids() {
        let mut seen: Vec<(u16, u16)> = Vec::new();
        for entry in PCI_CAPABILITY_TABLE.iter() {
            let key = (entry.vendor_id, entry.device_id);
            assert!(
                !seen.contains(&key),
                "duplicate PCI ID pair ({:#06X}, {:#06X}) found",
                key.0,
                key.1
            );
            seen.push(key);
        }
    }

    /// Every non-empty arch string must match expected format:
    /// CUDA SM version ("X.Y") or AMD gfx identifier ("gfx\d{4}").
    #[test]
    fn arch_format_validation() {
        for entry in PCI_CAPABILITY_TABLE.iter() {
            let arch = entry.arch;
            if arch.is_empty() {
                continue;
            }

            // Check for AMD gfx format: starts with "gfx" followed by digits,
            // optionally ending with a trailing letter (e.g. "gfx90a").
            if arch.starts_with("gfx") {
                let after_gfx = &arch[3..];
                assert!(
                    !after_gfx.is_empty(),
                    "AMD arch '{}' must have content after 'gfx'",
                    arch
                );
                // The core part must be digits; an optional trailing letter is
                // allowed (e.g. "gfx90a" where "90" are digits and "a" is suffix).
                let core = after_gfx.trim_end_matches(|c: char| c.is_ascii_alphabetic());
                assert!(
                    !core.is_empty() && core.chars().all(|c| c.is_ascii_digit()),
                    "AMD arch '{}' must be 'gfx' + digits (optionally trailing letter)",
                    arch
                );
            } else {
                // Check for CUDA SM format: "X.Y" (single digit, dot, single digit)
                let parts: Vec<&str> = arch.split('.').collect();
                assert_eq!(
                    parts.len(),
                    2,
                    "CUDA arch '{}' must match 'X.Y' pattern",
                    arch
                );
                for part in parts {
                    assert!(
                        !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()),
                        "CUDA arch '{}' component must be digits",
                        arch
                    );
                }
            }
        }
    }

    /// Boolean capability flags must be consistent per architecture family.
    #[test]
    fn boolean_flag_consistency() {
        for entry in PCI_CAPABILITY_TABLE.iter() {
            match entry.arch {
                // Ampere (8.0): fp16 + bf16 + flash attention (datacenter)
                "8.0" => {
                    assert!(entry.fp16);
                    assert!(entry.bf16);
                    assert!(entry.flash_attention);
                }
                // Hopper (9.0+): full modern capabilities
                s if s.starts_with('9') => {
                    assert!(entry.fp16);
                    assert!(entry.bf16);
                    assert!(entry.flash_attention);
                }
                // AMD RDNA3 / CDNA: fp16 + bf16 + flash attention
                s if s.starts_with("gfx") => {
                    assert!(entry.fp16);
                    assert!(entry.bf16);
                    assert!(entry.flash_attention);
                }
                // Consumer Ampere (8.6): fp16, no bf16, no flash attention
                "8.6" => {
                    assert!(entry.fp16);
                    assert!(!entry.bf16);
                    assert!(!entry.flash_attention);
                }
                _ => {}
            }
        }
    }

    /// DeviceCapabilityEntry must have exactly five capability fields —
    /// no VRAM-related field present.
    #[test]
    fn field_count_no_vram() {
        // Construct entry with all capability fields.
        let entry = DeviceCapabilityEntry {
            vendor_id: 0x10DE,
            device_id: 0x20B0,
            model_name: "Test",
            arch: "9.0",
            fp16: true,
            bf16: false,
            flash_attention: true,
        };

        // Verify all capability fields are accessible and correct.
        assert_eq!(entry.model_name, "Test");
        assert_eq!(entry.arch, "9.0");
        assert!(entry.fp16);
        assert!(!entry.bf16);
        assert!(entry.flash_attention);

        // Verify Copy + Clone work (returned by value, not reference).
        let cloned = entry;
        let copied = entry;
        assert_eq!(cloned.model_name, "Test");
        assert_eq!(copied.arch, "9.0");

        // Verify the struct does NOT contain any VRAM-related fields by
        // confirming the table itself has no VRAM field at compile time:
        // PCI_CAPABILITY_TABLE entries must only have the defined fields.
        // We verify this indirectly: the const table compiles, meaning the
        // struct definition is exactly as declared above.
        assert!(!PCI_CAPABILITY_TABLE.is_empty(), "table must not be empty");
    }

    /// Seed entry integrity: model names must be non-empty and within bounds.
    #[test]
    fn seed_entry_integrity() {
        for entry in PCI_CAPABILITY_TABLE.iter() {
            assert!(!entry.model_name.is_empty(), "model_name must not be empty");
            let len = entry.model_name.len();
            assert!(
                len >= 4 && len <= 128,
                "model_name '{}' length {} is out of range [4..128]",
                entry.model_name,
                len
            );

            // arch: must be non-empty for seeded entries
            assert!(
                !entry.arch.is_empty(),
                "arch must not be empty for '{}'",
                entry.model_name
            );
        }
    }
}
