/// Integration tests for `device_db.rs` — the PCI-ID capability table
/// and `resolve_caps_from_row` function.
///
/// These tests exercise the public API using the crate's public types.
/// All tests are pure data lookups with no I/O, no subprocess, and no
/// network — they construct `GpuDevice` structs with known PCI IDs and
/// verify that `resolve_caps_from_row` populates the correct fields.
use anvilml_core::{CapabilitySource, DeviceType, GpuDevice, InferenceCaps};
use anvilml_hardware::{resolve_caps_from_row, DEVICE_DB};

/// Verify that a known NVIDIA A100 (Ampere, 0x10de/0x2204) resolves
/// `arch` to `"Ampere"`, `fp8=true`, and `flash_attention=true`.
///
/// This test uses the A100-SXM4-40GB entry from `DEVICE_DB` and
/// confirms that the resolve function correctly populates all
/// capability fields from the table row.
#[serial_test::serial]
#[test]
fn test_resolve_nvidia_ampere() {
    let mut dev = GpuDevice {
        index: 0,
        name: "Unknown GPU".to_string(),
        device_type: DeviceType::Cuda,
        vram_total_mib: 40960,
        vram_free_mib: 38000,
        driver_version: "535.129.03".to_string(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: None,
        caps: InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Vulkan,
        capabilities_source: CapabilitySource::Fallback,
    };

    resolve_caps_from_row(&mut dev, None);

    assert_eq!(
        dev.arch,
        Some("Ampere".to_string()),
        "A100 arch must be Ampere"
    );
    assert!(dev.caps.fp8, "A100 must support FP8");
    assert!(dev.caps.flash_attention, "A100 must support FlashAttention");
    assert_eq!(
        dev.capabilities_source,
        CapabilitySource::DeviceTable,
        "capabilities_source must be DeviceTable after resolve"
    );
}

/// Verify that a known AMD RX 7900 XTX (RDNA3, 0x1002/0x74AF) resolves
/// `arch` to `"RDNA3"`, `fp8=false`, and `flash_attention=true`.
///
/// This test confirms that AMD GPUs are correctly matched and that
/// RDNA3 devices have flash_attention but not fp8 per the vendor spec.
#[serial_test::serial]
#[test]
fn test_resolve_amd_rdna3() {
    let mut dev = GpuDevice {
        index: 0,
        name: "Unknown GPU".to_string(),
        device_type: DeviceType::Rocm,
        vram_total_mib: 24576,
        vram_free_mib: 22000,
        driver_version: "6.0".to_string(),
        pci_vendor_id: 0x1002,
        pci_device_id: 0x74AF,
        arch: None,
        caps: InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Sysfs,
        capabilities_source: CapabilitySource::Fallback,
    };

    resolve_caps_from_row(&mut dev, None);

    assert_eq!(
        dev.arch,
        Some("RDNA3".to_string()),
        "RX 7900 XTX arch must be RDNA3"
    );
    assert!(!dev.caps.fp8, "RDNA3 does not support FP8");
    assert!(dev.caps.flash_attention, "RDNA3 supports FlashAttention");
}

/// Verify that an unknown vendor/device ID pair leaves `arch=None`,
/// `caps` unchanged (all `false`), and `capabilities_source` unchanged.
///
/// This test uses fabricated PCI IDs that do not exist in `DEVICE_DB`
/// and confirms the function is a no-op for unrecognized devices.
#[serial_test::serial]
#[test]
fn test_resolve_unknown_device() {
    let initial_caps = InferenceCaps::default();
    let mut dev = GpuDevice {
        index: 0,
        name: "Unknown GPU".to_string(),
        device_type: DeviceType::Cuda,
        vram_total_mib: 8192,
        vram_free_mib: 7000,
        driver_version: "0.0".to_string(),
        pci_vendor_id: 0x9999,
        pci_device_id: 0x9999,
        arch: None,
        caps: initial_caps.clone(),
        enumeration_source: anvilml_core::EnumerationSource::Vulkan,
        capabilities_source: CapabilitySource::Fallback,
    };

    resolve_caps_from_row(&mut dev, None);

    assert!(dev.arch.is_none(), "Unknown device arch must remain None");
    assert_eq!(
        dev.caps, initial_caps,
        "Unknown device caps must remain unchanged"
    );
    assert_eq!(
        dev.capabilities_source,
        CapabilitySource::Fallback,
        "Unknown device capabilities_source must remain unchanged"
    );
}

/// Verify that a CPU device (vendor_id=0, device_id=0) resolves to
/// no row and leaves `arch=None` and `caps` unchanged.
///
/// CPU devices synthesised by `CpuDetector` have PCI IDs of zero,
/// which do not match any entry in `DEVICE_DB`.
#[serial_test::serial]
#[test]
fn test_resolve_cpu_fallback() {
    let initial_caps = InferenceCaps::default();
    let mut dev = GpuDevice {
        index: 0,
        name: "CPU".to_string(),
        device_type: DeviceType::Cpu,
        vram_total_mib: 0,
        vram_free_mib: 0,
        driver_version: "n/a".to_string(),
        pci_vendor_id: 0,
        pci_device_id: 0,
        arch: None,
        caps: initial_caps.clone(),
        enumeration_source: anvilml_core::EnumerationSource::Mock,
        capabilities_source: CapabilitySource::Fallback,
    };

    resolve_caps_from_row(&mut dev, None);

    assert!(dev.arch.is_none(), "CPU device arch must remain None");
    assert_eq!(
        dev.caps, initial_caps,
        "CPU device caps must remain unchanged"
    );
}

/// Verify that `resolve_caps_from_row` does not modify `vram_total_mib`
/// or `vram_free_mib` — VRAM fields are set by the detector and must
/// be preserved by the capability resolution function.
///
/// This test uses a known GPU (RTX 4090) and confirms that after
/// capability resolution, the VRAM values are unchanged.
#[serial_test::serial]
#[test]
fn test_resolve_vram_untouched() {
    let mut dev = GpuDevice {
        index: 0,
        name: "Unknown GPU".to_string(),
        device_type: DeviceType::Cuda,
        vram_total_mib: 24576,
        vram_free_mib: 20000,
        driver_version: "545.00".to_string(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2488,
        arch: None,
        caps: InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Vulkan,
        capabilities_source: CapabilitySource::Fallback,
    };

    let vram_before_total = dev.vram_total_mib;
    let vram_before_free = dev.vram_free_mib;

    resolve_caps_from_row(&mut dev, None);

    assert_eq!(
        dev.vram_total_mib, vram_before_total,
        "vram_total_mib must not change after resolve"
    );
    assert_eq!(
        dev.vram_free_mib, vram_before_free,
        "vram_free_mib must not change after resolve"
    );
}

/// Verify that resolving a known device overwrites the `name` field
/// with the canonical name from `DEVICE_DB`.
///
/// This test uses an RTX 4090 and confirms the name changes from
/// `"Unknown GPU"` to `"NVIDIA RTX 4090"`.
#[serial_test::serial]
#[test]
fn test_resolve_name_overwrite() {
    let mut dev = GpuDevice {
        index: 0,
        name: "Unknown GPU".to_string(),
        device_type: DeviceType::Cuda,
        vram_total_mib: 16384,
        vram_free_mib: 15000,
        driver_version: "545.00".to_string(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2488,
        arch: None,
        caps: InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Vulkan,
        capabilities_source: CapabilitySource::Fallback,
    };

    resolve_caps_from_row(&mut dev, None);

    assert_eq!(
        dev.name, "NVIDIA RTX 4090",
        "Device name must be overwritten with canonical name"
    );
}

/// Verify that `DEVICE_DB` contains at least 12 curated entries.
///
/// This is a basic sanity check to ensure the table is populated
/// and not accidentally empty. The plan requires ≥ 12 entries.
#[serial_test::serial]
#[test]
fn test_device_db_non_empty() {
    assert!(
        DEVICE_DB.len() >= 12,
        "DEVICE_DB must have ≥ 12 entries, found {}",
        DEVICE_DB.len()
    );
}
