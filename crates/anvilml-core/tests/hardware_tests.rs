/// Tests for `types::hardware` — `HardwareInfo`, `GpuDevice`, `DeviceType`,
/// `HostInfo`, `InferenceCaps`, `EnumerationSource`, and `CapabilitySource`.
///
/// Verifies:
/// - JSON roundtrip for a fully-populated `HardwareInfo` with two `GpuDevice` entries.
/// - All three `DeviceType` variants roundtrip through JSON.
/// - `InferenceCaps::default()` produces all-false bool fields.
/// - All `EnumerationSource` and `CapabilitySource` variants roundtrip through JSON.
use anvilml_core::{
    CapabilitySource, DeviceType, EnumerationSource, GpuDevice, HardwareInfo, HostInfo,
    InferenceCaps,
};

/// Verifies that a fully-populated `HardwareInfo` serialises to JSON and
/// deserialises back to an identical value, including nested `GpuDevice`,
/// `HostInfo`, and `InferenceCaps` structs.
///
/// This is the primary acceptance test for the correctness of all
/// `Serialize`/`Deserialize` derives on `HardwareInfo` and its nested types.
#[test]
fn test_hardware_info_json_roundtrip() {
    let hardware = HardwareInfo {
        host: HostInfo {
            os: "Linux 6.1.0-ubuntu".to_string(),
            cpu: "Intel(R) Xeon(R) Gold 6348".to_string(),
            ram_total_mib: 65536,
        },
        gpus: vec![
            GpuDevice {
                index: 0,
                name: "NVIDIA A100-SXM4-40GB".to_string(),
                device_type: DeviceType::Cuda,
                vram_total_mib: 40960,
                vram_free_mib: 38000,
                driver_version: "535.129.03".to_string(),
                pci_vendor_id: 0x10de,
                pci_device_id: 0x20b2,
                arch: Some("Ampere".to_string()),
                caps: InferenceCaps {
                    fp32: true,
                    fp16: true,
                    bf16: true,
                    fp8: true,
                    fp4: false,
                    flash_attention: true,
                },
                enumeration_source: EnumerationSource::Nvml,
                capabilities_source: CapabilitySource::PyTorch,
            },
            GpuDevice {
                index: 1,
                name: "NVIDIA A100-SXM4-40GB".to_string(),
                device_type: DeviceType::Cuda,
                vram_total_mib: 40960,
                vram_free_mib: 39000,
                driver_version: "535.129.03".to_string(),
                pci_vendor_id: 0x10de,
                pci_device_id: 0x20b2,
                arch: Some("Ampere".to_string()),
                caps: InferenceCaps {
                    fp32: true,
                    fp16: true,
                    bf16: true,
                    fp8: false,
                    fp4: false,
                    flash_attention: true,
                },
                enumeration_source: EnumerationSource::Nvml,
                capabilities_source: CapabilitySource::PyTorch,
            },
        ],
        inference_caps: InferenceCaps {
            fp32: true,
            fp16: true,
            bf16: true,
            fp8: true,
            fp4: false,
            flash_attention: true,
        },
    };

    // Serialize to JSON
    let json = serde_json::to_string(&hardware).expect("serialize HardwareInfo to JSON");

    // Deserialize back — must not fail
    let restored: HardwareInfo =
        serde_json::from_str(&json).expect("deserialize JSON back to HardwareInfo");

    // All top-level fields must be equal
    assert_eq!(restored.host.os, hardware.host.os);
    assert_eq!(restored.host.cpu, hardware.host.cpu);
    assert_eq!(restored.host.ram_total_mib, hardware.host.ram_total_mib);

    // Both GPUs must roundtrip correctly
    assert_eq!(restored.gpus.len(), hardware.gpus.len());
    for (i, (orig, rest)) in hardware.gpus.iter().zip(restored.gpus.iter()).enumerate() {
        assert_eq!(rest.index, orig.index, "gpu[{}].index", i);
        assert_eq!(rest.name, orig.name, "gpu[{}].name", i);
        assert_eq!(rest.device_type, orig.device_type, "gpu[{}].device_type", i);
        assert_eq!(
            rest.vram_total_mib, orig.vram_total_mib,
            "gpu[{}].vram_total_mib",
            i
        );
        assert_eq!(
            rest.vram_free_mib, orig.vram_free_mib,
            "gpu[{}].vram_free_mib",
            i
        );
        assert_eq!(
            rest.driver_version, orig.driver_version,
            "gpu[{}].driver_version",
            i
        );
        assert_eq!(
            rest.pci_vendor_id, orig.pci_vendor_id,
            "gpu[{}].pci_vendor_id",
            i
        );
        assert_eq!(
            rest.pci_device_id, orig.pci_device_id,
            "gpu[{}].pci_device_id",
            i
        );
        assert_eq!(rest.arch, orig.arch, "gpu[{}].arch", i);
        assert_eq!(rest.caps, orig.caps, "gpu[{}].caps", i);
        assert_eq!(
            rest.enumeration_source, orig.enumeration_source,
            "gpu[{}].enumeration_source",
            i
        );
        assert_eq!(
            rest.capabilities_source, orig.capabilities_source,
            "gpu[{}].capabilities_source",
            i
        );
    }

    // Union inference caps must equal
    assert_eq!(restored.inference_caps, hardware.inference_caps);
}

/// Verifies that all three `DeviceType` enum variants roundtrip through
/// JSON serialisation without data loss.
///
/// Each variant is serialised to a JSON string and deserialised back,
/// then compared for equality. This tests that `#[serde(rename_all = "snake_case")]`
/// produces the correct lowercase variant names.
#[test]
fn test_device_type_variants() {
    let variants = [DeviceType::Cuda, DeviceType::Rocm, DeviceType::Cpu];

    for variant in variants {
        let json = serde_json::to_string(&variant).expect("serialize DeviceType variant to JSON");

        let restored: DeviceType =
            serde_json::from_str(&json).expect("deserialize JSON back to DeviceType");

        assert_eq!(
            restored, variant,
            "DeviceType::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }
}

/// Verifies that `InferenceCaps::default()` produces all-false bool fields,
/// representing the "unknown" initial state before the Python worker
/// reports actual capabilities.
///
/// The `Default` derive is required because pre-spawn values are hints —
/// a zero-value (all false) is a valid initial state.
#[test]
fn test_inference_caps_default() {
    let caps = InferenceCaps::default();

    assert!(!caps.fp32, "fp32 should be false by default");
    assert!(!caps.fp16, "fp16 should be false by default");
    assert!(!caps.bf16, "bf16 should be false by default");
    assert!(!caps.fp8, "fp8 should be false by default");
    assert!(!caps.fp4, "fp4 should be false by default");
    assert!(
        !caps.flash_attention,
        "flash_attention should be false by default"
    );
}

/// Verifies that all variants of `EnumerationSource` and `CapabilitySource`
/// roundtrip through JSON serialisation without data loss.
///
/// Tests all 6 `EnumerationSource` variants and all 3 `CapabilitySource`
/// variants in a single test, verifying that `#[serde(rename_all = "snake_case")]`
/// produces the correct lowercase names for both enums.
#[test]
fn test_enum_variants_roundtrip() {
    let enum_variants = [
        EnumerationSource::Vulkan,
        EnumerationSource::Dxgi,
        EnumerationSource::Sysfs,
        EnumerationSource::Nvml,
        EnumerationSource::Mock,
        EnumerationSource::Override,
    ];

    for variant in enum_variants {
        let json =
            serde_json::to_string(&variant).expect("serialize EnumerationSource variant to JSON");

        let restored: EnumerationSource =
            serde_json::from_str(&json).expect("deserialize JSON back to EnumerationSource");

        assert_eq!(
            restored, variant,
            "EnumerationSource::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }

    let cap_variants = [
        CapabilitySource::PyTorch,
        CapabilitySource::DeviceTable,
        CapabilitySource::Fallback,
    ];

    for variant in cap_variants {
        let json =
            serde_json::to_string(&variant).expect("serialize CapabilitySource variant to JSON");

        let restored: CapabilitySource =
            serde_json::from_str(&json).expect("deserialize JSON back to CapabilitySource");

        assert_eq!(
            restored, variant,
            "CapabilitySource::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }
}
