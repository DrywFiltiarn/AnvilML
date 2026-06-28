//! Tests for `DeviceType`, `HostInfo`, `GpuDevice`, `HardwareInfo`,
//! `InferenceCaps`, `EnumerationSource`, and `CapabilitySource` serde roundtrips.
//!
//! All tests construct types via the public API, serialise to JSON,
//! deserialise back, and assert equality. No I/O or env vars are used.

use anvilml_core::types::*;

/// Each of the three `DeviceType` variants serialises to the correct `snake_case`
/// JSON string and roundtrips back to an equal value.
#[test]
fn test_device_type_serde_snake_case() {
    let variants: [(DeviceType, &str); 3] = [
        (DeviceType::Cuda, "cuda"),
        (DeviceType::Rocm, "rocm"),
        (DeviceType::Cpu, "cpu"),
    ];

    for (device_type, expected_json) in variants {
        let json = serde_json::to_string(&device_type).expect("failed to serialise DeviceType");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "DeviceType::{:?} JSON mismatch",
            device_type
        );

        let roundtripped: DeviceType =
            serde_json::from_str(&json).expect("failed to deserialise DeviceType");
        assert_eq!(
            device_type, roundtripped,
            "DeviceType::{:?} roundtrip mismatch",
            device_type
        );
    }
}

/// A `HostInfo` with populated fields serialises to JSON and roundtrips
/// back to an equal value.
#[test]
fn test_host_info_serde_roundtrip() {
    let host = HostInfo {
        hostname: "testhost".to_string(),
        os: "Linux".to_string(),
    };

    let json = serde_json::to_string(&host).expect("failed to serialise HostInfo");
    let roundtripped: HostInfo =
        serde_json::from_str(&json).expect("failed to deserialise HostInfo");

    assert_eq!(
        host, roundtripped,
        "roundtripped HostInfo does not equal original"
    );

    // Verify the JSON contains the expected field names.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["hostname"], "testhost");
    assert_eq!(parsed["os"], "Linux");
}

/// A `GpuDevice` with all 12 fields populated serialises to JSON and
/// roundtrips back to an equal value.
#[test]
fn test_gpu_device_construction_and_serde() {
    let device = GpuDevice {
        index: 0,
        name: "NVIDIA GeForce RTX 4090".to_string(),
        device_type: DeviceType::Cuda,
        vram_total_mib: 24576,
        vram_free_mib: 23000,
        driver_version: "550.54.15".to_string(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2324,
        arch: Some("Ada Lovelace".to_string()),
        caps: InferenceCaps {
            fp32: true,
            fp16: true,
            bf16: true,
            fp8: true,
            fp4: false,
            flash_attention: true,
        },
        enumeration_source: EnumerationSource::Vulkan,
        capabilities_source: CapabilitySource::DeviceTable,
    };

    let json = serde_json::to_string(&device).expect("failed to serialise GpuDevice");
    let roundtripped: GpuDevice =
        serde_json::from_str(&json).expect("failed to deserialise GpuDevice");

    assert_eq!(
        device, roundtripped,
        "roundtripped GpuDevice does not equal original"
    );

    // Verify the JSON contains the expected field names.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["index"], 0);
    assert_eq!(parsed["name"], "NVIDIA GeForce RTX 4090");
    assert_eq!(parsed["device_type"], "cuda");
    assert_eq!(parsed["vram_total_mib"], 24576);
    assert_eq!(parsed["vram_free_mib"], 23000);
    assert_eq!(parsed["driver_version"], "550.54.15");
    assert_eq!(parsed["arch"], "Ada Lovelace");
    assert_eq!(parsed["caps"]["bf16"], true);
    assert_eq!(parsed["enumeration_source"], "vulkan");
    assert_eq!(parsed["capabilities_source"], "device_table");
}

/// A `HardwareInfo` with a `HostInfo`, a vector of two `GpuDevice` entries,
/// and an `InferenceCaps` serialises to JSON and roundtrips correctly.
#[test]
fn test_hardware_info_serde_roundtrip() {
    let info = HardwareInfo {
        host: HostInfo {
            hostname: "workstation".to_string(),
            os: "Linux".to_string(),
        },
        gpus: vec![
            GpuDevice {
                index: 0,
                name: "NVIDIA GeForce RTX 4090".to_string(),
                device_type: DeviceType::Cuda,
                vram_total_mib: 24576,
                vram_free_mib: 23000,
                driver_version: "550.54.15".to_string(),
                pci_vendor_id: 0x10de,
                pci_device_id: 0x2324,
                arch: Some("Ada Lovelace".to_string()),
                caps: InferenceCaps {
                    fp32: true,
                    fp16: true,
                    bf16: true,
                    fp8: true,
                    fp4: false,
                    flash_attention: true,
                },
                enumeration_source: EnumerationSource::Vulkan,
                capabilities_source: CapabilitySource::DeviceTable,
            },
            GpuDevice {
                index: 1,
                name: "NVIDIA GeForce RTX 3080".to_string(),
                device_type: DeviceType::Cuda,
                vram_total_mib: 10240,
                vram_free_mib: 9800,
                driver_version: "550.54.15".to_string(),
                pci_vendor_id: 0x10de,
                pci_device_id: 0x2206,
                arch: Some("Ampere".to_string()),
                caps: InferenceCaps {
                    fp32: true,
                    fp16: true,
                    bf16: false,
                    fp8: false,
                    fp4: false,
                    flash_attention: true,
                },
                enumeration_source: EnumerationSource::Sysfs,
                capabilities_source: CapabilitySource::Fallback,
            },
        ],
        inference_caps: InferenceCaps {
            fp32: true,
            fp16: true,
            bf16: false,
            fp8: false,
            fp4: false,
            flash_attention: true,
        },
    };

    let json = serde_json::to_string(&info).expect("failed to serialise HardwareInfo");
    let roundtripped: HardwareInfo =
        serde_json::from_str(&json).expect("failed to deserialise HardwareInfo");

    assert_eq!(
        info, roundtripped,
        "roundtripped HardwareInfo does not equal original"
    );

    // Verify the JSON contains the expected structure.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["host"]["hostname"], "workstation");
    assert_eq!(parsed["gpus"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["gpus"][0]["index"], 0);
    assert_eq!(parsed["gpus"][1]["index"], 1);
}

/// `InferenceCaps` with all defaults serialises correctly and roundtrips.
#[test]
fn test_inference_caps_default_roundtrip() {
    let caps = InferenceCaps::default();

    let json = serde_json::to_string(&caps).expect("failed to serialise InferenceCaps");
    let roundtripped: InferenceCaps =
        serde_json::from_str(&json).expect("failed to deserialise InferenceCaps");

    assert_eq!(
        caps, roundtripped,
        "roundtripped InferenceCaps does not equal original"
    );

    // Verify all fields are false (default).
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["fp32"], false);
    assert_eq!(parsed["fp16"], false);
    assert_eq!(parsed["bf16"], false);
    assert_eq!(parsed["fp8"], false);
    assert_eq!(parsed["fp4"], false);
    assert_eq!(parsed["flash_attention"], false);
}

/// All `EnumerationSource` variants serialise to correct `snake_case` JSON.
#[test]
fn test_enumeration_source_serde_snake_case() {
    let variants: [(EnumerationSource, &str); 7] = [
        (EnumerationSource::Vulkan, "vulkan"),
        (EnumerationSource::Dxgi, "dxgi"),
        (EnumerationSource::Sysfs, "sysfs"),
        (EnumerationSource::Nvml, "nvml"),
        (EnumerationSource::Cpu, "cpu"),
        (EnumerationSource::Mock, "mock"),
        (EnumerationSource::Override, "override"),
    ];

    for (source, expected_json) in variants {
        let json = serde_json::to_string(&source).expect("failed to serialise EnumerationSource");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "EnumerationSource::{:?} JSON mismatch",
            source
        );

        let roundtripped: EnumerationSource =
            serde_json::from_str(&json).expect("failed to deserialise");
        assert_eq!(
            source, roundtripped,
            "EnumerationSource::{:?} roundtrip mismatch",
            source
        );
    }
}

/// All `CapabilitySource` variants serialise to correct `snake_case` JSON.
#[test]
fn test_capability_source_serde_snake_case() {
    let variants: [(CapabilitySource, &str); 3] = [
        (CapabilitySource::PyTorch, "pytorch"),
        (CapabilitySource::DeviceTable, "device_table"),
        (CapabilitySource::Fallback, "fallback"),
    ];

    for (source, expected_json) in variants {
        let json = serde_json::to_string(&source).expect("failed to serialise CapabilitySource");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "CapabilitySource::{:?} JSON mismatch",
            source
        );

        let roundtripped: CapabilitySource =
            serde_json::from_str(&json).expect("failed to deserialise");
        assert_eq!(
            source, roundtripped,
            "CapabilitySource::{:?} roundtrip mismatch",
            source
        );
    }
}

/// An `InferenceCaps` with mixed true/false fields serialises to correct JSON,
/// roundtrips back to an equal value, and all six field names appear in the
/// JSON payload with the expected types.
#[test]
fn test_inference_caps_non_default_roundtrip() {
    let caps = InferenceCaps {
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        flash_attention: true,
    };

    let json = serde_json::to_string(&caps).expect("failed to serialise InferenceCaps");
    let roundtripped: InferenceCaps =
        serde_json::from_str(&json).expect("failed to deserialise InferenceCaps");

    assert_eq!(
        caps, roundtripped,
        "roundtripped InferenceCaps does not equal original"
    );

    // Verify JSON field names are correct snake_case identifiers.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["fp32"], true);
    assert_eq!(parsed["fp16"], true);
    assert_eq!(parsed["bf16"], true);
    assert_eq!(parsed["fp8"], false);
    assert_eq!(parsed["fp4"], false);
    assert_eq!(parsed["flash_attention"], true);
}

/// Both `EnumerationSource` and `CapabilitySource` implement `Copy` — assigning
/// a variant to a new variable does not move it, so both the original and the
/// copy remain usable.
#[test]
fn test_enumeration_source_copy_trait() {
    // EnumerationSource: copy and use both values independently.
    let source = EnumerationSource::Cpu;
    let source_copy = source; // If Copy is not implemented, this would move `source`.
    assert_eq!(
        source, source_copy,
        "EnumerationSource Copy assignment changed value"
    );
    // Use both values in further assertions to prove neither was moved.
    let json_a = serde_json::to_string(&source).expect("serialise original");
    let json_b = serde_json::to_string(&source_copy).expect("serialise copy");
    assert_eq!(
        json_a, json_b,
        "EnumerationSource original and copy serialise identically"
    );

    // CapabilitySource: same Copy verification.
    let cap = CapabilitySource::PyTorch;
    let cap_copy = cap; // If Copy is not implemented, this would move `cap`.
    assert_eq!(
        cap, cap_copy,
        "CapabilitySource Copy assignment changed value"
    );
    let json_c = serde_json::to_string(&cap).expect("serialise original");
    let json_d = serde_json::to_string(&cap_copy).expect("serialise copy");
    assert_eq!(
        json_c, json_d,
        "CapabilitySource original and copy serialise identically"
    );
}
