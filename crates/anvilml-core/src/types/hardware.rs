//! Hardware domain types per ANVILML_DESIGN §4.3.
//!
//! Defines `HardwareInfo`, `GpuDevice`, `HostInfo`, and `InferenceCaps` — all
//! serializable, clonable, debuggable, and schema-annotated for OpenAPI generation.
//!
//! `DeviceType` is re-exported from the config module to avoid duplication,
//! since it already exists there with identical variants (Cuda, Rocm, Cpu).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Re-use the existing DeviceType from config to avoid duplication.
pub use crate::config::DeviceType;

/// Source used to enumerate a GPU device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub enum EnumerationSource {
    /// Detected via Vulkan physical device enumeration.
    Vulkan,
    /// Detected via Windows DXGI adapter enumeration.
    Dxgi,
    /// Detected via Linux/Unix PCI sysfs walk.
    Sysfs,
    /// Detected via NVIDIA Management Library (NVML).
    Nvml,
    /// Provided by a user-specified hardware override in config.
    Override,
    /// Synthetic device from the mock detector (CI/testing).
    Mock,
    /// Device was discovered through PCI-ID table resolution.
    DeviceTable,
    /// Synthetic device from the CPU fallback detector.
    #[default]
    Fallback,
}

/// Source used to resolve inference capabilities for a GPU device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub enum CapabilitySource {
    /// Capabilities resolved from the PCI-ID device capability table.
    #[default]
    Fallback,
    /// Capabilities resolved from the PCI-ID device capability table.
    DeviceTable,
    /// Capabilities refined at runtime by the worker (e.g. via MemoryReport).
    Worker,
}

/// Host-level hardware information.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostInfo {
    /// Operating system identifier (e.g. "Linux 6.1.0").
    pub os: String,
    /// CPU model string (e.g. "Intel(R) Xeon(R) CPU E5-2686 v4 @ 2.30GHz").
    pub cpu_model: String,
    /// Total host RAM in MiB.
    pub ram_total_mib: u64,
    /// Free host RAM in MiB.
    pub ram_free_mib: u64,
}

/// Inference capability flags for detected hardware.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, ToSchema)]
pub struct InferenceCaps {
    /// Whether the device supports FP32 (single-precision) inference.
    #[serde(default)]
    pub fp32: bool,
    /// Whether the device supports FP16 (half-precision) inference.
    #[serde(default)]
    pub fp16: bool,
    /// Whether the device supports BF16 (bfloat16) inference.
    #[serde(default)]
    pub bf16: bool,
    /// Whether the device supports FP8 inference.
    #[serde(default)]
    pub fp8: bool,
    /// Whether the device supports FP4 inference.
    #[serde(default)]
    pub fp4: bool,
    /// Whether the device supports NVIDIA FP4 (NVFP4) inference.
    #[serde(default)]
    pub nvfp4: bool,
    /// Whether the device supports Flash Attention.
    #[serde(default)]
    pub flash_attention: bool,
}

/// A single GPU device detected on the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GpuDevice {
    /// Zero-based index of this GPU device.
    pub index: u32,
    /// Human-readable device name (e.g. "NVIDIA A100-SXM4-80GB").
    pub name: String,
    /// Hardware backend type.
    pub device_type: DeviceType,
    /// Total VRAM in MiB.
    pub vram_total_mib: u32,
    /// Free VRAM in MiB (refreshed periodically from worker MemoryReport events).
    pub vram_free_mib: u32,
    /// GPU driver version string.
    pub driver_version: String,
    /// PCI vendor ID (e.g. 0x10DE for NVIDIA, 0x1002 for AMD).
    #[serde(default)]
    pub pci_vendor_id: u16,
    /// PCI device ID within the vendor.
    #[serde(default)]
    pub pci_device_id: u16,
    /// Architecture identifier — CUDA SM version ("8.6") or AMD gfx ("gfx1100").
    #[serde(default)]
    pub arch: Option<String>,
    /// Inference capability flags resolved from the device database or defaults.
    #[serde(default)]
    pub caps: InferenceCaps,
    /// Source used to enumerate this device.
    #[serde(default)]
    pub enumeration_source: EnumerationSource,
    /// Source used to resolve inference capabilities.
    #[serde(default)]
    pub capabilities_source: CapabilitySource,
}

/// Complete hardware report for the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HardwareInfo {
    /// Host-level information (OS, CPU, RAM).
    pub host: HostInfo,
    /// Detected GPU devices.
    #[serde(default)]
    pub gpus: Vec<GpuDevice>,
    /// Inference capability flags derived from device type and driver.
    #[serde(default)]
    pub inference_caps: InferenceCaps,
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `DeviceType` must have exactly 3 variants and all pairs must compare
    /// equal/unequal correctly.
    #[test]
    fn device_type_variants() {
        let types: Vec<DeviceType> = vec![DeviceType::Cuda, DeviceType::Rocm, DeviceType::Cpu];

        assert_eq!(types.len(), 3, "must have exactly 3 variants");

        // All variants must be distinct.
        for i in 0..types.len() {
            for j in (i + 1)..types.len() {
                assert_ne!(types[i], types[j], "variants {i} and {j} must differ");
            }
        }

        // Self-equality.
        assert_eq!(DeviceType::Cuda, DeviceType::Cuda);
        assert_eq!(DeviceType::Rocm, DeviceType::Rocm);
        assert_eq!(DeviceType::Cpu, DeviceType::Cpu);
    }

    /// `InferenceCaps` defaults must produce all-false.
    #[test]
    fn inference_caps_defaults() {
        let caps = InferenceCaps::default();
        assert!(!caps.fp32);
        assert!(!caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.fp8);
        assert!(!caps.fp4);
        assert!(!caps.nvfp4);
        assert!(!caps.flash_attention);
    }

    /// `InferenceCaps` fields must round-trip through JSON serialization.
    #[test]
    fn inference_caps_roundtrip() {
        let caps = InferenceCaps {
            fp32: false,
            fp16: true,
            bf16: false,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attention: true,
        };

        let json = serde_json::to_string(&caps).expect("serialize InferenceCaps");
        let restored: InferenceCaps =
            serde_json::from_str(&json).expect("deserialize InferenceCaps");

        assert_eq!(restored.fp32, false);
        assert_eq!(restored.fp16, true);
        assert_eq!(restored.bf16, false);
        assert_eq!(restored.fp8, false);
        assert_eq!(restored.fp4, false);
        assert_eq!(restored.nvfp4, false);
        assert_eq!(restored.flash_attention, true);
    }

    /// `HostInfo` fields must round-trip through JSON serialization.
    #[test]
    fn host_info_roundtrip() {
        let info = HostInfo {
            os: "Linux 6.1.0".to_string(),
            cpu_model: "Intel(R) Xeon(R) CPU E5-2686 v4 @ 2.30GHz".to_string(),
            ram_total_mib: 65536,
            ram_free_mib: 32768,
        };

        let json = serde_json::to_string(&info).expect("serialize HostInfo");
        let restored: HostInfo = serde_json::from_str(&json).expect("deserialize HostInfo");

        assert_eq!(restored.os, info.os);
        assert_eq!(restored.cpu_model, info.cpu_model);
        assert_eq!(restored.ram_total_mib, info.ram_total_mib);
        assert_eq!(restored.ram_free_mib, info.ram_free_mib);
    }

    /// `GpuDevice` fields must round-trip through JSON serialization.
    #[test]
    fn gpu_device_roundtrip() {
        let device = GpuDevice {
            index: 0,
            name: "NVIDIA A100-SXM4-80GB".to_string(),
            device_type: DeviceType::Cuda,
            vram_total_mib: 81920,
            vram_free_mib: 78000,
            driver_version: "535.129.03".to_string(),
            pci_vendor_id: 0x10DE,
            pci_device_id: 0x20B0,
            arch: Some("8.0".to_string()),
            caps: InferenceCaps {
                fp32: false,
                fp16: true,
                bf16: true,
                fp8: false,
                fp4: false,
                nvfp4: false,
                flash_attention: true,
            },
            enumeration_source: EnumerationSource::Vulkan,
            capabilities_source: CapabilitySource::DeviceTable,
        };

        let json = serde_json::to_string(&device).expect("serialize GpuDevice");
        let restored: GpuDevice = serde_json::from_str(&json).expect("deserialize GpuDevice");

        assert_eq!(restored.index, device.index);
        assert_eq!(restored.name, device.name);
        assert_eq!(restored.device_type, device.device_type);
        assert_eq!(restored.vram_total_mib, device.vram_total_mib);
        assert_eq!(restored.vram_free_mib, device.vram_free_mib);
        assert_eq!(restored.driver_version, device.driver_version);
        assert_eq!(restored.pci_vendor_id, device.pci_vendor_id);
        assert_eq!(restored.pci_device_id, device.pci_device_id);
        assert_eq!(restored.arch, device.arch);
        assert_eq!(restored.caps.fp32, device.caps.fp32);
        assert_eq!(restored.caps.fp16, device.caps.fp16);
        assert_eq!(restored.caps.bf16, device.caps.bf16);
        assert_eq!(restored.caps.fp8, device.caps.fp8);
        assert_eq!(restored.caps.fp4, device.caps.fp4);
        assert_eq!(restored.caps.nvfp4, device.caps.nvfp4);
        assert_eq!(restored.caps.flash_attention, device.caps.flash_attention);
        assert_eq!(restored.enumeration_source, device.enumeration_source);
        assert_eq!(restored.capabilities_source, device.capabilities_source);
    }

    /// `GpuDevice` with old fields only must deserialize with sensible defaults.
    #[test]
    fn gpu_device_backward_compat() {
        // JSON with only the original 6 fields.
        let json = r#"{
            "index": 0,
            "name": "Test GPU",
            "device_type": "cuda",
            "vram_total_mib": 16384,
            "vram_free_mib": 15000,
            "driver_version": "535.0"
        }"#;

        let restored: GpuDevice = serde_json::from_str(json).expect("deserialize GpuDevice");

        assert_eq!(restored.index, 0);
        assert_eq!(restored.name, "Test GPU");
        assert_eq!(restored.device_type, DeviceType::Cuda);
        assert_eq!(restored.vram_total_mib, 16384);
        assert_eq!(restored.vram_free_mib, 15000);
        assert_eq!(restored.driver_version, "535.0");
        // New fields should use defaults.
        assert_eq!(restored.pci_vendor_id, 0);
        assert_eq!(restored.pci_device_id, 0);
        assert!(restored.arch.is_none());
        assert!(!restored.caps.fp32);
        assert!(!restored.caps.fp16);
        assert!(!restored.caps.bf16);
        assert!(!restored.caps.fp8);
        assert!(!restored.caps.fp4);
        assert!(!restored.caps.nvfp4);
        assert!(!restored.caps.flash_attention);
        assert!(matches!(
            restored.enumeration_source,
            EnumerationSource::Fallback
        ));
        assert!(matches!(
            restored.capabilities_source,
            CapabilitySource::Fallback
        ));
    }

    /// `HardwareInfo` with multiple GPUs must round-trip through JSON serialization.
    #[test]
    fn hardware_info_roundtrip() {
        let info = HardwareInfo {
            host: HostInfo {
                os: "Linux 6.1.0".to_string(),
                cpu_model: "AMD Ryzen Threadripper PRO 7995WX".to_string(),
                ram_total_mib: 262144,
                ram_free_mib: 200000,
            },
            gpus: vec![
                GpuDevice {
                    index: 0,
                    name: "NVIDIA A100-SXM4-80GB".to_string(),
                    device_type: DeviceType::Cuda,
                    vram_total_mib: 81920,
                    vram_free_mib: 80000,
                    driver_version: "535.129.03".to_string(),
                    pci_vendor_id: 0x10DE,
                    pci_device_id: 0x20B0,
                    arch: Some("8.0".to_string()),
                    caps: InferenceCaps {
                        fp32: false,
                        fp16: true,
                        bf16: true,
                        fp8: false,
                        fp4: false,
                        nvfp4: false,
                        flash_attention: true,
                    },
                    enumeration_source: EnumerationSource::Vulkan,
                    capabilities_source: CapabilitySource::DeviceTable,
                },
                GpuDevice {
                    index: 1,
                    name: "NVIDIA A100-SXM4-80GB".to_string(),
                    device_type: DeviceType::Cuda,
                    vram_total_mib: 81920,
                    vram_free_mib: 75000,
                    driver_version: "535.129.03".to_string(),
                    pci_vendor_id: 0x10DE,
                    pci_device_id: 0x20B0,
                    arch: Some("8.0".to_string()),
                    caps: InferenceCaps {
                        fp32: false,
                        fp16: true,
                        bf16: true,
                        fp8: false,
                        fp4: false,
                        nvfp4: false,
                        flash_attention: true,
                    },
                    enumeration_source: EnumerationSource::Vulkan,
                    capabilities_source: CapabilitySource::DeviceTable,
                },
            ],
            inference_caps: InferenceCaps {
                fp32: false,
                fp16: true,
                bf16: true,
                fp8: false,
                fp4: false,
                nvfp4: false,
                flash_attention: true,
            },
        };

        let json = serde_json::to_string(&info).expect("serialize HardwareInfo");
        let restored: HardwareInfo = serde_json::from_str(&json).expect("deserialize HardwareInfo");

        assert_eq!(restored.host.os, info.host.os);
        assert_eq!(restored.host.cpu_model, info.host.cpu_model);
        assert_eq!(restored.host.ram_total_mib, info.host.ram_total_mib);
        assert_eq!(restored.host.ram_free_mib, info.host.ram_free_mib);
        assert_eq!(restored.gpus.len(), info.gpus.len());
        for (a, b) in restored.gpus.iter().zip(info.gpus.iter()) {
            assert_eq!(a.index, b.index);
            assert_eq!(a.name, b.name);
            assert_eq!(a.device_type, b.device_type);
            assert_eq!(a.vram_total_mib, b.vram_total_mib);
            assert_eq!(a.vram_free_mib, b.vram_free_mib);
            assert_eq!(a.driver_version, b.driver_version);
            assert_eq!(a.pci_vendor_id, b.pci_vendor_id);
            assert_eq!(a.pci_device_id, b.pci_device_id);
            assert_eq!(a.arch, b.arch);
            assert_eq!(a.caps.fp32, b.caps.fp32);
            assert_eq!(a.caps.fp16, b.caps.fp16);
            assert_eq!(a.caps.bf16, b.caps.bf16);
            assert_eq!(a.caps.fp8, b.caps.fp8);
            assert_eq!(a.caps.fp4, b.caps.fp4);
            assert_eq!(a.caps.nvfp4, b.caps.nvfp4);
            assert_eq!(a.caps.flash_attention, b.caps.flash_attention);
            assert_eq!(a.enumeration_source, b.enumeration_source);
            assert_eq!(a.capabilities_source, b.capabilities_source);
        }
        assert_eq!(restored.inference_caps.fp32, info.inference_caps.fp32);
        assert_eq!(restored.inference_caps.fp16, info.inference_caps.fp16);
        assert_eq!(restored.inference_caps.bf16, info.inference_caps.bf16);
        assert_eq!(restored.inference_caps.fp8, info.inference_caps.fp8);
        assert_eq!(restored.inference_caps.fp4, info.inference_caps.fp4);
        assert_eq!(restored.inference_caps.nvfp4, info.inference_caps.nvfp4);
        assert_eq!(
            restored.inference_caps.flash_attention,
            info.inference_caps.flash_attention
        );
    }

    /// `HardwareInfo` with empty GPU list must round-trip correctly.
    #[test]
    fn hardware_info_empty_gpus() {
        let info = HardwareInfo {
            host: HostInfo {
                os: "Linux 6.1.0".to_string(),
                cpu_model: "Intel(R) Core(TM) i9-13900K".to_string(),
                ram_total_mib: 32768,
                ram_free_mib: 28000,
            },
            gpus: vec![],
            inference_caps: InferenceCaps::default(),
        };

        let json = serde_json::to_string(&info).expect("serialize HardwareInfo with no GPUs");
        let restored: HardwareInfo =
            serde_json::from_str(&json).expect("deserialize HardwareInfo with no GPUs");

        assert!(restored.gpus.is_empty());
        assert!(!restored.inference_caps.fp32);
        assert!(!restored.inference_caps.fp16);
        assert!(!restored.inference_caps.bf16);
        assert!(!restored.inference_caps.fp8);
        assert!(!restored.inference_caps.fp4);
        assert!(!restored.inference_caps.nvfp4);
        assert!(!restored.inference_caps.flash_attention);
    }

    /// Verify that DeviceType serializes to expected JSON strings.
    #[test]
    fn device_type_json_strings() {
        let cuda_json = serde_json::to_string(&DeviceType::Cuda).unwrap();
        assert_eq!(cuda_json, "\"cuda\"");

        let rocm_json = serde_json::to_string(&DeviceType::Rocm).unwrap();
        assert_eq!(rocm_json, "\"rocm\"");

        let cpu_json = serde_json::to_string(&DeviceType::Cpu).unwrap();
        assert_eq!(cpu_json, "\"cpu\"");
    }

    /// `EnumerationSource` must have exactly 8 variants and all pairs distinct.
    #[test]
    fn enumeration_source_variants() {
        let sources = vec![
            EnumerationSource::Vulkan,
            EnumerationSource::Dxgi,
            EnumerationSource::Sysfs,
            EnumerationSource::Nvml,
            EnumerationSource::Override,
            EnumerationSource::Mock,
            EnumerationSource::DeviceTable,
            EnumerationSource::Fallback,
        ];

        assert_eq!(sources.len(), 8, "must have exactly 8 variants");

        for i in 0..sources.len() {
            for j in (i + 1)..sources.len() {
                assert_ne!(sources[i], sources[j], "variants {i} and {j} must differ");
            }
        }
    }

    /// `CapabilitySource` must have exactly 3 variants and all pairs distinct.
    #[test]
    fn capability_source_variants() {
        let sources = vec![
            CapabilitySource::Fallback,
            CapabilitySource::DeviceTable,
            CapabilitySource::Worker,
        ];

        assert_eq!(sources.len(), 3, "must have exactly 3 variants");

        for i in 0..sources.len() {
            for j in (i + 1)..sources.len() {
                assert_ne!(sources[i], sources[j], "variants {i} and {j} must differ");
            }
        }
    }

    /// `EnumerationSource` defaults must be Fallback.
    #[test]
    fn enumeration_source_default_is_fallback() {
        let source: EnumerationSource = Default::default();
        assert!(matches!(source, EnumerationSource::Fallback));
    }

    /// `CapabilitySource` defaults must be Fallback.
    #[test]
    fn capability_source_default_is_fallback() {
        let source: CapabilitySource = Default::default();
        assert!(matches!(source, CapabilitySource::Fallback));
    }

    /// `EnumerationSource` and `CapabilitySource` must round-trip through JSON.
    #[test]
    fn enumeration_capability_sources_roundtrip() {
        let json = r#""Vulkan""#;
        let restored: EnumerationSource = serde_json::from_str(json).unwrap();
        assert_eq!(restored, EnumerationSource::Vulkan);

        let json = r#""DeviceTable""#;
        let restored: CapabilitySource = serde_json::from_str(json).unwrap();
        assert_eq!(restored, CapabilitySource::DeviceTable);

        let json = r#""Override""#;
        let restored: EnumerationSource = serde_json::from_str(json).unwrap();
        assert_eq!(restored, EnumerationSource::Override);

        let json = r#""Worker""#;
        let restored: CapabilitySource = serde_json::from_str(json).unwrap();
        assert_eq!(restored, CapabilitySource::Worker);

        let json = r#""Fallback""#;
        let restored: EnumerationSource = serde_json::from_str(json).unwrap();
        assert_eq!(restored, EnumerationSource::Fallback);
    }
}
