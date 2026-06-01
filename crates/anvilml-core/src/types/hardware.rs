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
    /// Whether the device supports FP16 (half-precision) inference.
    #[serde(default)]
    pub fp16: bool,
    /// Whether the device supports BF16 (bfloat16) inference.
    #[serde(default)]
    pub bf16: bool,
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
        assert!(!caps.fp16);
        assert!(!caps.bf16);
        assert!(!caps.flash_attention);
    }

    /// `InferenceCaps` fields must round-trip through JSON serialization.
    #[test]
    fn inference_caps_roundtrip() {
        let caps = InferenceCaps {
            fp16: true,
            bf16: false,
            flash_attention: true,
        };

        let json = serde_json::to_string(&caps).expect("serialize InferenceCaps");
        let restored: InferenceCaps =
            serde_json::from_str(&json).expect("deserialize InferenceCaps");

        assert_eq!(restored.fp16, true);
        assert_eq!(restored.bf16, false);
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
        };

        let json = serde_json::to_string(&device).expect("serialize GpuDevice");
        let restored: GpuDevice = serde_json::from_str(&json).expect("deserialize GpuDevice");

        assert_eq!(restored.index, device.index);
        assert_eq!(restored.name, device.name);
        assert_eq!(restored.device_type, device.device_type);
        assert_eq!(restored.vram_total_mib, device.vram_total_mib);
        assert_eq!(restored.vram_free_mib, device.vram_free_mib);
        assert_eq!(restored.driver_version, device.driver_version);
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
                },
                GpuDevice {
                    index: 1,
                    name: "NVIDIA A100-SXM4-80GB".to_string(),
                    device_type: DeviceType::Cuda,
                    vram_total_mib: 81920,
                    vram_free_mib: 75000,
                    driver_version: "535.129.03".to_string(),
                },
            ],
            inference_caps: InferenceCaps {
                fp16: true,
                bf16: true,
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
        for (_i, (a, b)) in restored.gpus.iter().zip(info.gpus.iter()).enumerate() {
            assert_eq!(a.index, b.index);
            assert_eq!(a.name, b.name);
            assert_eq!(a.device_type, b.device_type);
            assert_eq!(a.vram_total_mib, b.vram_total_mib);
            assert_eq!(a.vram_free_mib, b.vram_free_mib);
            assert_eq!(a.driver_version, b.driver_version);
        }
        assert_eq!(restored.inference_caps.fp16, info.inference_caps.fp16);
        assert_eq!(restored.inference_caps.bf16, info.inference_caps.bf16);
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
        assert!(!restored.inference_caps.fp16);
        assert!(!restored.inference_caps.bf16);
        assert!(!restored.inference_caps.flash_attention);
    }

    /// Verify that DeviceType serializes to expected JSON strings.
    #[test]
    fn device_type_json_strings() {
        let cuda_json = serde_json::to_string(&DeviceType::Cuda).unwrap();
        assert_eq!(cuda_json, "\"Cuda\"");

        let rocm_json = serde_json::to_string(&DeviceType::Rocm).unwrap();
        assert_eq!(rocm_json, "\"Rocm\"");

        let cpu_json = serde_json::to_string(&DeviceType::Cpu).unwrap();
        assert_eq!(cpu_json, "\"Cpu\"");
    }
}
