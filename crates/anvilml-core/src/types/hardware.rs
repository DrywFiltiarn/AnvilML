//! Hardware detection output types — host info, GPU devices, and inference capabilities.
//!
//! All types are pure serializable data: zero I/O, zero async. They derive
//! `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// DeviceType — hardware backend kind
// ---------------------------------------------------------------------------

/// The hardware backend / device type for inference.
///
/// MVP set: CUDA (NVIDIA), ROCm (AMD), and CPU. Deferred backends
/// (Intel IPEX, Apple MPS, AMD DirectML) are tracked in §25.
#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    ToSchema,
)]
pub enum DeviceType {
    /// NVIDIA CUDA backend.
    Cuda,
    /// AMD ROCm backend.
    Rocm,
    /// CPU (generic) backend.
    Cpu,
}

// ---------------------------------------------------------------------------
// GpuDevice — information about a single GPU
// ---------------------------------------------------------------------------

/// Metadata about a single GPU device detected on the host.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct GpuDevice {
    /// Zero-based index of the device.
    pub index: u32,

    /// Human-readable device name (e.g. "NVIDIA GeForce RTX 4090").
    pub name: String,

    /// The hardware backend type.
    pub device_type: DeviceType,

    /// Total VRAM in mebibytes.
    pub vram_total_mib: u32,

    /// Free VRAM in mebibytes (refreshed periodically).
    pub vram_free_mib: u32,

    /// Driver version string (e.g. "535.129.03").
    pub driver_version: String,
}

// ---------------------------------------------------------------------------
// HardwareInfo — complete hardware report
// ---------------------------------------------------------------------------

/// Full hardware report returned by the hardware detection subsystem.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct HardwareInfo {
    /// Host-level information (OS, CPU, RAM).
    pub host: HostInfo,

    /// Detected GPU devices.
    pub gpus: Vec<GpuDevice>,

    /// Inference capabilities derived from device type and driver.
    pub inference_caps: InferenceCaps,
}

// ---------------------------------------------------------------------------
// HostInfo — host-level metadata
// ---------------------------------------------------------------------------

/// Metadata about the host machine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct HostInfo {
    /// Operating system name (e.g. "Linux").
    pub os: String,

    /// CPU model string (e.g. "AMD Ryzen 9 7950X").
    pub cpu_model: String,

    /// Total physical RAM in mebibytes.
    pub ram_total_mib: u64,

    /// Available physical RAM in mebibytes.
    pub ram_free_mib: u64,
}

// ---------------------------------------------------------------------------
// InferenceCaps — ML inference capability flags
// ---------------------------------------------------------------------------

/// Flags describing the ML inference capabilities of the detected hardware.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct InferenceCaps {
    /// Whether the hardware supports FP16 inference.
    pub fp16: bool,

    /// Whether the hardware supports BF16 inference.
    pub bf16: bool,

    /// Whether Flash Attention is available.
    pub flash_attention: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // DeviceType — serialization round-trip
    // ------------------------------------------------------------------

    #[test]
    fn device_type_serialization_round_trip() {
        for dtype in [DeviceType::Cuda, DeviceType::Rocm, DeviceType::Cpu] {
            let json = serde_json::to_string(&dtype).unwrap();
            let back: DeviceType = serde_json::from_str(&json).unwrap();
            assert_eq!(dtype, back, "failed for {:?}", dtype);
        }
    }

    // ------------------------------------------------------------------
    // HardwareInfo — construction and round-trip
    // ------------------------------------------------------------------

    #[test]
    fn hardware_info_round_trip() {
        let info = HardwareInfo {
            host: HostInfo {
                os: "Linux".into(),
                cpu_model: "Test CPU".into(),
                ram_total_mib: 32768,
                ram_free_mib: 16384,
            },
            gpus: vec![GpuDevice {
                index: 0,
                name: "NVIDIA GeForce RTX 4090".into(),
                device_type: DeviceType::Cuda,
                vram_total_mib: 24576,
                vram_free_mib: 24000,
                driver_version: "535.129.03".into(),
            }],
            inference_caps: InferenceCaps {
                fp16: true,
                bf16: true,
                flash_attention: true,
            },
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: HardwareInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, back);
    }
}
