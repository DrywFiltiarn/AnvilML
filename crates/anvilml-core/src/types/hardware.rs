//! Hardware domain types for the AnvilML system.
//!
//! Defines `HardwareInfo` (a snapshot of the host machine's hardware),
//! `GpuDevice` (information about a single GPU), and supporting types
//! (`DeviceType`, `HostInfo`, `InferenceCaps`, `EnumerationSource`,
//! `CapabilitySource`) used during hardware detection and reporting.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The compute backend a device uses for inference.
///
/// Maps to one of the three supported acceleration backends: NVIDIA CUDA,
/// AMD ROCm, or generic CPU execution. This enum is used to select which
/// device detection path to take and which Python worker configuration to
/// produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// NVIDIA CUDA backend — requires an NVIDIA GPU with CUDA drivers.
    Cuda,
    /// AMD ROCm backend — requires an AMD GPU with ROCm drivers.
    Rocm,
    /// Generic CPU execution — no GPU acceleration.
    Cpu,
}

/// The backend used to enumerate and detect a device on the host.
///
/// Each variant corresponds to a different detection mechanism. The
/// enumeration source is recorded alongside device metadata so the
/// system can assess confidence in the reported values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnumerationSource {
    /// Vulkan ICD enumeration — detects GPUs via Vulkan driver interface.
    Vulkan,
    /// Windows DXGI enumeration — detects GPUs via the Windows Display
    /// Graphics Interface.
    Dxgi,
    /// Linux sysfs enumeration — reads GPU info from `/sys/class/drm`.
    Sysfs,
    /// NVIDIA NVML enumeration — uses the NVIDIA Management Library for
    /// detailed GPU telemetry.
    Nvml,
    /// Mock enumeration — device info was synthesised by the mock-hardware
    /// feature for CI testing.
    Mock,
    /// Override enumeration — device info was provided via a configuration
    /// override (e.g. `[hardware_override]` in `anvilml.toml`).
    Override,
}

/// The backend used to report a device's inference capabilities.
///
/// Capabilities can come from the Python worker (PyTorch runtime),
/// a local device table (pre-known GPU specs), or a fallback heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    /// Capabilities reported by the Python worker at startup via PyTorch.
    PyTorch,
    /// Capabilities looked up from a local device table of known GPUs.
    DeviceTable,
    /// Capabilities inferred by a fallback heuristic when no precise
    /// source is available.
    Fallback,
}

/// Inference compute capabilities supported by a device.
///
/// Each field indicates whether the device supports a specific precision
/// format. All fields default to `false` because pre-spawn values are
/// hints — the Python worker reports actual capabilities at startup,
/// and these values are overwritten with the real data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct InferenceCaps {
    /// Whether the device supports 32-bit floating point inference.
    pub fp32: bool,
    /// Whether the device supports 16-bit floating point inference.
    pub fp16: bool,
    /// Whether the device supports brain floating point (16-bit with
    /// extended range).
    pub bf16: bool,
    /// Whether the device supports 8-bit floating point inference.
    pub fp8: bool,
    /// Whether the device supports 4-bit floating point inference.
    pub fp4: bool,
    /// Whether the device supports FlashAttention for faster attention
    /// computation.
    pub flash_attention: bool,
}

/// Host-level hardware information for a hardware snapshot.
///
/// Captures the minimal host-level information needed for a hardware
/// inventory: operating system identity, CPU model, and total system
/// RAM. This is reported alongside GPU information to provide a
/// complete picture of the machine's capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HostInfo {
    /// Operating system identity string (e.g. `"Linux 6.1.0"`, `"Windows 11"`).
    pub os: String,
    /// CPU model string (e.g. `"Intel(R) Xeon(R) Gold 6348"`).
    pub cpu: String,
    /// Total system RAM in mebibytes (MiB).
    pub ram_total_mib: u32,
}

/// Information about a single GPU device.
///
/// Produced by the hardware detection subsystem. Contains the device
/// index, human-readable name, compute backend, VRAM capacity, driver
/// version, PCI identifiers, optional architecture string, and both
/// the enumeration source and the inference capabilities reported by
/// the device.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GpuDevice {
    /// Zero-based index of the device as reported by the OS/driver.
    pub index: u32,
    /// Human-readable device name (e.g. `"NVIDIA A100-SXM4-40GB"`).
    pub name: String,
    /// Compute backend this device uses for inference.
    pub device_type: DeviceType,
    /// Total VRAM capacity in mebibytes (MiB).
    pub vram_total_mib: u32,
    /// Free VRAM at the time of detection, in mebibytes (MiB).
    pub vram_free_mib: u32,
    /// Driver version string (e.g. `"535.129.03"` for NVIDIA, `"6.0"` for ROCm).
    pub driver_version: String,
    /// PCI vendor ID (e.g. `0x10de` for NVIDIA, `0x1002` for AMD).
    pub pci_vendor_id: u16,
    /// PCI device ID — unique within the vendor.
    pub pci_device_id: u16,
    /// GPU architecture string (e.g. `"Ampere"`, `"RDNA3"`). `None` when
    /// the detection backend does not provide this information.
    pub arch: Option<String>,
    /// Inference capabilities supported by this device.
    pub caps: InferenceCaps,
    /// Backend used to enumerate and detect this device.
    pub enumeration_source: EnumerationSource,
    /// Backend that reported the inference capabilities in `caps`.
    pub capabilities_source: CapabilitySource,
}

/// A complete hardware snapshot of the host machine.
///
/// Combines host-level information, a list of detected GPUs, and the
/// union of all per-device inference capabilities. The `inference_caps`
/// field is the union of all `GpuDevice.caps` values, as stated in the
/// design doc — it represents the best capabilities available across
/// all devices on the system.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HardwareInfo {
    /// Host-level hardware information (OS, CPU, RAM).
    pub host: HostInfo,
    /// List of detected GPU devices, indexed by `GpuDevice.index`.
    pub gpus: Vec<GpuDevice>,
    /// Union of all per-device inference capabilities across all GPUs.
    pub inference_caps: InferenceCaps,
}
