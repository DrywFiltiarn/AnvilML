//! Hardware snapshot types for the AnvilML device table.
//!
//! These structs capture a point-in-time snapshot of the host machine's compute
//! devices (GPUs, CPUs) and their capabilities. They are populated by
//! `anvilml-hardware` detectors (Phase 4) and consumed by the scheduler and
//! worker dispatcher.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The compute backend or execution target of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// NVIDIA CUDA device.
    Cuda,
    /// AMD ROCm device.
    Rocm,
    /// CPU (no GPU).
    Cpu,
}

/// Minimal host information for the hardware snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HostInfo {
    /// The hostname of the machine as reported by the OS.
    pub hostname: String,
    /// The operating system name (e.g. "Linux", "Windows").
    pub os: String,
}

/// Inference precision capabilities.
///
/// Pre-spawn values (`capabilities_source = DeviceTable` or `Fallback`) are HINTS
/// only — they exist so the scheduler can make a provisional VRAM/dtype guess before
/// any worker has started. They are never trusted as ground truth for an actual
/// inference decision. The authoritative values come from the Python worker's own
/// runtime probe at `Ready` (`capabilities_source = PyTorch`) — see §6.6.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct InferenceCaps {
    /// Device supports FP32 compute.
    pub fp32: bool,
    /// Device supports FP16 compute.
    pub fp16: bool,
    /// Device supports BF16 compute.
    pub bf16: bool,
    /// Device supports FP8 compute.
    pub fp8: bool,
    /// Device supports FP4 compute.
    pub fp4: bool,
    /// Device supports Flash Attention.
    pub flash_attention: bool,
}

/// A single detected compute device (GPU or CPU).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GpuDevice {
    /// Zero-based device index as reported by the OS/driver.
    pub index: u32,
    /// Human-readable device name (e.g. "NVIDIA GeForce RTX 4090").
    pub name: String,
    /// The compute backend type.
    pub device_type: DeviceType,
    /// Total VRAM in mebibytes.
    pub vram_total_mib: u32,
    /// Free VRAM in mebibytes at time of detection.
    pub vram_free_mib: u32,
    /// Driver version string (e.g. "550.54.15").
    pub driver_version: String,
    /// PCI vendor ID (e.g. 0x10de for NVIDIA).
    pub pci_vendor_id: u16,
    /// PCI device ID (vendor-specific).
    pub pci_device_id: u16,
    /// Architecture string (e.g. "Ada Lovelace", "RDNA 3"). None for CPU.
    pub arch: Option<String>,
    /// Per-device inference capabilities (bf16, fp16, fp8, etc.).
    pub caps: InferenceCaps,
    /// How this device was enumerated.
    pub enumeration_source: EnumerationSource,
    /// Where the capability values came from.
    pub capabilities_source: CapabilitySource,
}

/// Where a device was enumerated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnumerationSource {
    /// Detected via Vulkan (headless GPU enumeration).
    Vulkan,
    /// Detected via Windows DXGI.
    Dxgi,
    /// Detected via Linux sysfs PCI enumeration.
    Sysfs,
    /// Detected via NVIDIA NVML.
    Nvml,
    /// Synthesised CPU device (no GPU detected, fallback).
    Cpu,
    /// Mock device driven by environment variables.
    Mock,
    /// Override from config `[hardware_override]` section.
    Override,
}

/// Where an `InferenceCaps` value came from.
///
/// `PyTorch` is the only source an arch module's loader is permitted to make a
/// compute-dtype decision from at runtime. `DeviceTable` and `Fallback` are
/// pre-spawn hints for scheduling estimates only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    /// Authoritative values from the Python worker's runtime torch probe.
    #[serde(rename = "pytorch")]
    PyTorch,
    /// Pre-spawn hint from PCI-ID device capability table.
    DeviceTable,
    /// Pre-spawn fallback when no PCI-ID match was found.
    Fallback,
}

/// Complete hardware snapshot: host info, all detected compute devices, and
/// the aggregate inference capabilities across all devices.
///
/// This is the primary output of hardware enumeration — produced once at server
/// startup and used by the scheduler for device selection and the worker
/// dispatcher for routing jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HardwareInfo {
    /// The host machine's basic information.
    pub host: HostInfo,
    /// All detected compute devices on the host.
    pub gpus: Vec<GpuDevice>,
    /// Aggregate inference capabilities across all detected devices.
    /// Represents the intersection of per-device capabilities.
    pub inference_caps: InferenceCaps,
}
