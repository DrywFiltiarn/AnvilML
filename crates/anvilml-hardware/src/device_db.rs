//! PCI-ID capability table for GPU device resolution.
//!
//! A hardcoded compile-time const-slice mapping (vendor_id, device_id) tuples to
//! model name, architecture string, and ML inference capabilities.
//!
//! Provides [`resolve_caps_from_row`] for populating a [`GpuDevice`] from an
//! [`anvilml_registry::DeviceCapabilityRow`].

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
    /// Whether the device supports FP32 (single-precision) inference via TF32 tensor cores.
    pub fp32: bool,
    /// Whether the device supports FP16 (half-precision) inference.
    pub fp16: bool,
    /// Whether the device supports BF16 (bfloat16) inference.
    pub bf16: bool,
    /// Whether the device supports FP8 inference.
    pub fp8: bool,
    /// Whether the device supports FP4 inference.
    pub fp4: bool,
    /// Whether the device supports NVIDIA FP4 (NVFP4) inference.
    pub nvfp4: bool,
    /// Whether the device supports Flash Attention.
    pub flash_attention: bool,
}

// ── Seeded PCI capability table ───────────────────────────────────────────────

/// Compile-time PCI-ID capability table seeded from [`SUPPORTED_DEVICES_DB.md`](../../docs/SUPPORTED_DEVICES_DB.md).
///
/// Contains all 126 entries from the NVIDIA and AMD device tables.
pub const SEED_ENTRIES: &[DeviceCapabilityEntry] = &[
    // ── NVIDIA Pascal (SM 6.1) ───────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1B00,
        model_name: "NVIDIA TITAN X (Pascal)",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1B02,
        model_name: "NVIDIA TITAN Xp",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1B80,
        model_name: "NVIDIA GeForce GTX 1080 Ti",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1B81,
        model_name: "NVIDIA GeForce GTX 1080",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1B82,
        model_name: "NVIDIA GeForce GTX 1070 Ti",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1B84,
        model_name: "NVIDIA GeForce GTX 1070",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1B83,
        model_name: "NVIDIA GeForce GTX 1060 6GB",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1C02,
        model_name: "NVIDIA GeForce GTX 1060 3GB",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1C82,
        model_name: "NVIDIA GeForce GTX 1050 Ti",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1C81,
        model_name: "NVIDIA GeForce GTX 1050",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1CB3,
        model_name: "NVIDIA Quadro P4000",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1CB1,
        model_name: "NVIDIA Quadro P2000",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1CA8,
        model_name: "NVIDIA Quadro P5000",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1CB2,
        model_name: "NVIDIA Quadro P3000",
        arch: "6.1",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    // ── NVIDIA Turing GTX 16xx (SM 7.5) ─────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2182,
        model_name: "NVIDIA GeForce GTX 1660 Ti",
        arch: "7.5",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x21C4,
        model_name: "NVIDIA GeForce GTX 1660 Super",
        arch: "7.5",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2184,
        model_name: "NVIDIA GeForce GTX 1660",
        arch: "7.5",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2187,
        model_name: "NVIDIA GeForce GTX 1650 Super",
        arch: "7.5",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1F82,
        model_name: "NVIDIA GeForce GTX 1650",
        arch: "7.5",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    // ── NVIDIA Turing RTX 20xx (SM 7.5) ─────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1E02,
        model_name: "NVIDIA GeForce RTX 2080 Ti",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1E84,
        model_name: "NVIDIA GeForce RTX 2080 Super",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1E04,
        model_name: "NVIDIA GeForce RTX 2080",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1F06,
        model_name: "NVIDIA GeForce RTX 2070 Super",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1F02,
        model_name: "NVIDIA GeForce RTX 2070",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1F47,
        model_name: "NVIDIA GeForce RTX 2060 Super",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1F08,
        model_name: "NVIDIA GeForce RTX 2060",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1E30,
        model_name: "NVIDIA Quadro RTX 6000",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x1E78,
        model_name: "NVIDIA Quadro RTX 8000",
        arch: "7.5",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    // ── NVIDIA Ampere datacenter GA100 (SM 8.0) ─────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x20B2,
        model_name: "NVIDIA A100-SXM4-80GB",
        arch: "8.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x20B0,
        model_name: "NVIDIA A100-SXM4-40GB",
        arch: "8.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x20B5,
        model_name: "NVIDIA A100-PCIe-80GB",
        arch: "8.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x20F1,
        model_name: "NVIDIA A100-PCIe-40GB",
        arch: "8.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x20B8,
        model_name: "NVIDIA A30",
        arch: "8.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── NVIDIA Ampere consumer RTX 30xx (SM 8.6) ────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2208,
        model_name: "NVIDIA GeForce RTX 3090 Ti",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2204,
        model_name: "NVIDIA GeForce RTX 3090",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2216,
        model_name: "NVIDIA GeForce RTX 3080 Ti",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2206,
        model_name: "NVIDIA GeForce RTX 3080",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2482,
        model_name: "NVIDIA GeForce RTX 3070 Ti",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2484,
        model_name: "NVIDIA GeForce RTX 3070",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2489,
        model_name: "NVIDIA GeForce RTX 3060 Ti",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2503,
        model_name: "NVIDIA GeForce RTX 3060",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2571,
        model_name: "NVIDIA GeForce RTX 3050",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2230,
        model_name: "NVIDIA RTX A6000",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2233,
        model_name: "NVIDIA RTX A5000",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2235,
        model_name: "NVIDIA RTX A4000",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2236,
        model_name: "NVIDIA RTX A4500",
        arch: "8.6",
        fp32: true,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── NVIDIA Hopper GH100 (SM 9.0) ────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2322,
        model_name: "NVIDIA H100-SXM5-80GB",
        arch: "9.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2330,
        model_name: "NVIDIA H100-PCIe-80GB",
        arch: "9.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2324,
        model_name: "NVIDIA H800-SXM5-80GB",
        arch: "9.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2336,
        model_name: "NVIDIA H20",
        arch: "9.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── NVIDIA Ada consumer RTX 40xx (SM 8.9) ───────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2684,
        model_name: "NVIDIA GeForce RTX 4090",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2702,
        model_name: "NVIDIA GeForce RTX 4080",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2704,
        model_name: "NVIDIA GeForce RTX 4080 Super",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2782,
        model_name: "NVIDIA GeForce RTX 4070 Ti",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2783,
        model_name: "NVIDIA GeForce RTX 4070 Ti Super",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2788,
        model_name: "NVIDIA GeForce RTX 4070 Super",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2786,
        model_name: "NVIDIA GeForce RTX 4070",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2803,
        model_name: "NVIDIA GeForce RTX 4060 Ti 16GB",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2805,
        model_name: "NVIDIA GeForce RTX 4060 Ti 8GB",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2882,
        model_name: "NVIDIA GeForce RTX 4060",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x28A0,
        model_name: "NVIDIA GeForce RTX 4050",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── NVIDIA Ada datacenter L40/L40S (SM 8.9) ─────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x26B5,
        model_name: "NVIDIA L40S",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x26B9,
        model_name: "NVIDIA L40",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x26F5,
        model_name: "NVIDIA L4",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x27B0,
        model_name: "NVIDIA RTX 6000 Ada",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x27B8,
        model_name: "NVIDIA RTX 5000 Ada",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x27B6,
        model_name: "NVIDIA RTX 4500 Ada",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x27B2,
        model_name: "NVIDIA RTX 4000 Ada",
        arch: "8.9",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── NVIDIA Blackwell (SM 10.0) ──────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2B85,
        model_name: "NVIDIA GeForce RTX 5090",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2B87,
        model_name: "NVIDIA GeForce RTX 5080",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2C02,
        model_name: "NVIDIA GeForce RTX 5070 Ti",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2C05,
        model_name: "NVIDIA GeForce RTX 5070",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2C82,
        model_name: "NVIDIA GeForce RTX 5060 Ti",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2C87,
        model_name: "NVIDIA GeForce RTX 5060",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2B02,
        model_name: "NVIDIA B200",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2B03,
        model_name: "NVIDIA B100",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x10DE,
        device_id: 0x2B06,
        model_name: "NVIDIA B40",
        arch: "10.0",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: true,
        flash_attention: true,
    },
    // ── AMD RDNA 1 (gfx101x) ────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x731F,
        model_name: "AMD Radeon RX 5700 XT",
        arch: "gfx1010",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7312,
        model_name: "AMD Radeon RX 5700",
        arch: "gfx1010",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7310,
        model_name: "AMD Radeon RX 5700 XT 50th",
        arch: "gfx1010",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7318,
        model_name: "AMD Radeon RX 5600 XT",
        arch: "gfx1010",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7360,
        model_name: "AMD Radeon Pro V520",
        arch: "gfx1011",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7340,
        model_name: "AMD Radeon RX 5500 XT",
        arch: "gfx1012",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7341,
        model_name: "AMD Radeon RX 5500",
        arch: "gfx1012",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7347,
        model_name: "AMD Radeon RX 5300",
        arch: "gfx1012",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7362,
        model_name: "AMD Radeon Pro W5700",
        arch: "gfx1012",
        fp32: false,
        fp16: false,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    // ── AMD RDNA 2 (gfx103x) ────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73BF,
        model_name: "AMD Radeon RX 6900 XT",
        arch: "gfx1030",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73AF,
        model_name: "AMD Radeon RX 6950 XT",
        arch: "gfx1030",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73A5,
        model_name: "AMD Radeon RX 6800 XT",
        arch: "gfx1030",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73AB,
        model_name: "AMD Radeon RX 6800",
        arch: "gfx1030",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73A1,
        model_name: "AMD Radeon Pro V620",
        arch: "gfx1030",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73A3,
        model_name: "AMD Radeon Pro W6800",
        arch: "gfx1030",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73DF,
        model_name: "AMD Radeon RX 6750 XT",
        arch: "gfx1031",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73DA,
        model_name: "AMD Radeon RX 6700 XT",
        arch: "gfx1031",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73DC,
        model_name: "AMD Radeon RX 6700",
        arch: "gfx1031",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73E1,
        model_name: "AMD Radeon Pro W6600",
        arch: "gfx1032",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73FF,
        model_name: "AMD Radeon RX 6650 XT",
        arch: "gfx1032",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73EF,
        model_name: "AMD Radeon RX 6600 XT",
        arch: "gfx1032",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x73F0,
        model_name: "AMD Radeon RX 6600",
        arch: "gfx1032",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7422,
        model_name: "AMD Radeon RX 6500 XT",
        arch: "gfx1034",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7424,
        model_name: "AMD Radeon RX 6400",
        arch: "gfx1034",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    // ── AMD RDNA 3 (gfx11xx) ────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x744C,
        model_name: "AMD Radeon RX 7900 XTX",
        arch: "gfx1100",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7448,
        model_name: "AMD Radeon RX 7900 XT",
        arch: "gfx1100",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x745E,
        model_name: "AMD Radeon RX 7900 GRE",
        arch: "gfx1100",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7461,
        model_name: "AMD Radeon Pro W7900",
        arch: "gfx1100",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7470,
        model_name: "AMD Radeon RX 7800 XT",
        arch: "gfx1101",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x747E,
        model_name: "AMD Radeon RX 7700 XT",
        arch: "gfx1101",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7490,
        model_name: "AMD Radeon Pro W7700",
        arch: "gfx1101",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7483,
        model_name: "AMD Radeon RX 7600 XT",
        arch: "gfx1102",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7480,
        model_name: "AMD Radeon RX 7600",
        arch: "gfx1102",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7489,
        model_name: "AMD Radeon RX 7700",
        arch: "gfx1102",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7499,
        model_name: "AMD Radeon RX 7400",
        arch: "gfx1102",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: false,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7452,
        model_name: "AMD Radeon Pro W7800",
        arch: "gfx1100",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── AMD RDNA 4 (gfx12xx) ────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7550,
        model_name: "AMD Radeon RX 9070 XT",
        arch: "gfx1201",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7551,
        model_name: "AMD Radeon AI PRO R9700",
        arch: "gfx1201",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7590,
        model_name: "AMD Radeon RX 9060 XT",
        arch: "gfx1200",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── AMD CDNA 1 (gfx908) ─────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x738C,
        model_name: "AMD Instinct MI100",
        arch: "gfx908",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7388,
        model_name: "AMD Instinct MI100 (alt SKU)",
        arch: "gfx908",
        fp32: false,
        fp16: true,
        bf16: false,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── AMD CDNA 2 (gfx90a) ─────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7408,
        model_name: "AMD Instinct MI250X",
        arch: "gfx90a",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x740C,
        model_name: "AMD Instinct MI250",
        arch: "gfx90a",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x7410,
        model_name: "AMD Instinct MI210",
        arch: "gfx90a",
        fp32: false,
        fp16: true,
        bf16: true,
        fp8: false,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── AMD CDNA 3 (gfx942) ─────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x74A0,
        model_name: "AMD Instinct MI300A",
        arch: "gfx942",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x74A1,
        model_name: "AMD Instinct MI300X",
        arch: "gfx942",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x74B5,
        model_name: "AMD Instinct MI325X",
        arch: "gfx942",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: false,
        nvfp4: false,
        flash_attention: true,
    },
    // ── AMD CDNA 4 (gfx950) ─────────────────────────────────────────────────
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x74C0,
        model_name: "AMD Instinct MI350X",
        arch: "gfx950",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: true,
        nvfp4: false,
        flash_attention: true,
    },
    DeviceCapabilityEntry {
        vendor_id: 0x1002,
        device_id: 0x74C1,
        model_name: "AMD Instinct MI355X",
        arch: "gfx950",
        fp32: true,
        fp16: true,
        bf16: true,
        fp8: true,
        fp4: true,
        nvfp4: false,
        flash_attention: true,
    },
];

// ── Resolve capabilities from a DeviceCapabilityRow ─────────────────────────

/// Resolve GPU capabilities from an [`anvilml_registry::DeviceCapabilityRow`].
///
/// On a lookup hit, sets `dev.name`, `dev.arch`, `dev.caps`,
/// `dev.enumeration_source`, and `dev.capabilities_source` from the row.
/// On a miss, logs a warning with the vendor + device IDs for table extension tracking,
/// then sets conservative defaults (`caps = InferenceCaps::default()`,
/// `capabilities_source = CapabilitySource::Fallback`).
pub fn resolve_caps_from_row(
    dev: &mut GpuDevice,
    row: Option<&anvilml_registry::DeviceCapabilityRow>,
) {
    match row {
        Some(r) => {
            dev.name = r.model_name.clone();
            dev.arch = Some(r.arch.clone());
            dev.caps = anvilml_core::InferenceCaps {
                fp32: r.fp32,
                fp16: r.fp16,
                bf16: r.bf16,
                fp8: r.fp8,
                fp4: r.fp4,
                nvfp4: r.nvfp4,
                flash_attention: r.flash_attn,
            };
            dev.capabilities_source = CapabilitySource::DeviceTable;
            dev.enumeration_source = EnumerationSource::DeviceTable;
        }
        None => {
            tracing::warn!(
                detector = "DeviceDB",
                vendor_id = %format_args!("0x{:04X}", dev.pci_vendor_id),
                device_id = %format_args!("0x{:04X}", dev.pci_device_id),
                "unknown PCI ID — add to SUPPORTED_DEVICES_DB.md"
            );
            dev.caps = anvilml_core::InferenceCaps::default();
            dev.capabilities_source = CapabilitySource::Fallback;
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Every seeded entry must be found by iterating SEED_ENTRIES with the correct PCI IDs.
    #[test]
    fn seed_entries_lookup() {
        // NVIDIA RTX 3090 — Ampere SM 8.6
        let entry = SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x10DE && e.device_id == 0x2204);
        assert!(entry.is_some(), "RTX 3090 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA GeForce RTX 3090");
        assert_eq!(e.arch, "8.6");
        assert!(e.fp16);
        assert!(!e.bf16);
        assert!(e.flash_attention);

        // NVIDIA A100 — Ampere SM 8.0 (datacenter)
        let entry = SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x10DE && e.device_id == 0x20B0);
        assert!(entry.is_some(), "A100 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA A100-SXM4-40GB");
        assert_eq!(e.arch, "8.0");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);

        // NVIDIA H100 — Hopper SM 9.0
        let entry = SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x10DE && e.device_id == 0x2322);
        assert!(entry.is_some(), "H100 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA H100-SXM5-80GB");
        assert_eq!(e.arch, "9.0");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);

        // NVIDIA RTX 3080 — Turing SM 8.6
        let entry = SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x10DE && e.device_id == 0x2206);
        assert!(entry.is_some(), "RTX 3080 must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "NVIDIA GeForce RTX 3080");
        assert_eq!(e.arch, "8.6");
        assert!(e.fp16);
        assert!(!e.bf16);
        assert!(e.flash_attention);

        // AMD RX 7900 XTX — RDNA3 gfx1100
        let entry = SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x1002 && e.device_id == 0x744C);
        assert!(entry.is_some(), "RX 7900 XTX must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "AMD Radeon RX 7900 XTX");
        assert_eq!(e.arch, "gfx1100");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);

        // AMD MI250X — CDNA gfx90a
        let entry = SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x1002 && e.device_id == 0x740C);
        assert!(entry.is_some(), "MI250X must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "AMD Instinct MI250");
        assert_eq!(e.arch, "gfx90a");
        assert!(e.fp16);
        assert!(e.bf16);
        assert!(e.flash_attention);
    }

    /// Lookup must return None for non-existent PCI ID combinations.
    #[test]
    fn miss_returns_none() {
        // Intel vendor ID (not seeded)
        assert!(SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x8086 && e.device_id == 0x56A0)
            .is_none());
        // Arbitrary unknown pair
        assert!(SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0xDEAD && e.device_id == 0xBEEF)
            .is_none());
        // NVIDIA vendor but unknown device
        assert!(SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x10DE && e.device_id == 0xFFFF)
            .is_none());
    }

    /// No two entries may share the same (vendor_id, device_id) pair.
    #[test]
    fn no_duplicate_pci_ids() {
        let mut seen: Vec<(u16, u16)> = Vec::new();
        for entry in SEED_ENTRIES.iter() {
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
        for entry in SEED_ENTRIES.iter() {
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
        for entry in SEED_ENTRIES.iter() {
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
                // Consumer Ampere (8.6): fp16, no bf16, flash attention
                "8.6" => {
                    assert!(entry.fp16);
                    assert!(!entry.bf16);
                    assert!(entry.flash_attention);
                }
                // AMD RDNA1 (gfx101x): no ML capabilities
                s if s.starts_with("gfx101") => {
                    assert!(!entry.fp16);
                    assert!(!entry.bf16);
                    assert!(!entry.flash_attention);
                }
                // AMD cut-down RDNA2/3 (gfx1034, gfx1102 RX7400): no bf16, no flash attn
                s if s == "gfx1034" || (s == "gfx1102" && entry.model_name.contains("7400")) => {
                    assert!(entry.fp16);
                    assert!(!entry.bf16);
                    assert!(!entry.flash_attention);
                }
                // AMD CDNA1 (gfx908): fp16, no bf16, flash attention
                "gfx908" => {
                    assert!(entry.fp16);
                    assert!(!entry.bf16);
                    assert!(entry.flash_attention);
                }
                // AMD RDNA2/3 (non-cut-down) / CDNA2+ / RDNA4: fp16 + bf16 + flash attention
                s if s.starts_with("gfx") => {
                    assert!(entry.fp16);
                    assert!(entry.bf16);
                    assert!(entry.flash_attention);
                }
                _ => {}
            }
        }
    }

    /// DeviceCapabilityEntry must have exactly seven capability fields —
    /// no VRAM-related field present.
    #[test]
    fn field_count_no_vram() {
        // Construct entry with all capability fields.
        let entry = DeviceCapabilityEntry {
            vendor_id: 0x10DE,
            device_id: 0x20B0,
            model_name: "Test",
            arch: "9.0",
            fp32: true,
            fp16: true,
            bf16: false,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attention: true,
        };

        // Verify all capability fields are accessible and correct.
        assert_eq!(entry.model_name, "Test");
        assert_eq!(entry.arch, "9.0");
        assert!(entry.fp16);
        assert!(!entry.bf16);
        assert!(entry.flash_attention);
        assert!(entry.fp32);
        assert!(!entry.fp8);
        assert!(!entry.fp4);
        assert!(!entry.nvfp4);

        // Verify Copy + Clone work (returned by value, not reference).
        let cloned = entry;
        let copied = entry;
        assert_eq!(cloned.model_name, "Test");
        assert_eq!(copied.arch, "9.0");

        // Verify the struct does NOT contain any VRAM-related fields by
        // confirming the table itself has no VRAM field at compile time:
        // SEED_ENTRIES entries must only have the defined fields.
        // We verify this indirectly: the const table compiles, meaning the
        // struct definition is exactly as declared above.
        assert!(!SEED_ENTRIES.is_empty(), "table must not be empty");
    }

    /// Seed entry integrity: model names must be non-empty and within bounds.
    #[test]
    fn seed_entry_integrity() {
        for entry in SEED_ENTRIES.iter() {
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

    /// AMD Radeon RX 9070 XT entry must have correct PCI IDs, name, arch, and flags.
    #[test]
    fn rx9070xt_entry_correct() {
        let entry = SEED_ENTRIES
            .iter()
            .find(|e| e.vendor_id == 0x1002 && e.device_id == 0x7550);
        assert!(entry.is_some(), "RX 9070 XT must be in table");
        let e = entry.unwrap();
        assert_eq!(e.model_name, "AMD Radeon RX 9070 XT");
        assert_eq!(e.arch, "gfx1201");
        assert!(e.fp8);
        assert!(!e.fp32);
    }

    /// SEED_ENTRIES must contain exactly 126 entries.
    #[test]
    fn seed_entries_count() {
        assert_eq!(SEED_ENTRIES.len(), 126, "must have exactly 126 entries");
    }

    /// resolve_caps_from_row hit-path must set all fields correctly.
    #[test]
    fn resolve_caps_from_row_hit() {
        let mut dev = GpuDevice {
            index: 0,
            name: "Driver Name".to_string(),
            device_type: anvilml_core::DeviceType::Cuda,
            vram_total_mib: 8192,
            vram_free_mib: 7000,
            driver_version: "535.0".to_string(),
            pci_vendor_id: 0x10DE,
            pci_device_id: 0x2204,
            arch: None,
            caps: anvilml_core::InferenceCaps::default(),
            enumeration_source: EnumerationSource::Fallback,
            capabilities_source: CapabilitySource::Fallback,
        };

        let row = anvilml_registry::DeviceCapabilityRow {
            vendor_id: 0x10DE,
            device_id: 0x2204,
            model_name: "NVIDIA GeForce RTX 3090".to_string(),
            arch: "8.6".to_string(),
            fp32: true,
            fp16: true,
            bf16: false,
            fp8: false,
            fp4: false,
            nvfp4: false,
            flash_attn: true,
        };

        resolve_caps_from_row(&mut dev, Some(&row));

        assert_eq!(dev.name, "NVIDIA GeForce RTX 3090");
        assert_eq!(dev.arch, Some("8.6".to_string()));
        assert!(dev.caps.fp16);
        assert!(!dev.caps.bf16);
        assert!(dev.caps.flash_attention);
        assert!(matches!(
            dev.capabilities_source,
            CapabilitySource::DeviceTable
        ));
        assert!(matches!(
            dev.enumeration_source,
            EnumerationSource::DeviceTable
        ));
    }

    /// resolve_caps_from_row miss-path must preserve dev.name and set fallback defaults.
    #[test]
    fn resolve_caps_from_row_miss() {
        let mut dev = GpuDevice {
            index: 0,
            name: "Driver Reported Name".to_string(),
            device_type: anvilml_core::DeviceType::Cuda,
            vram_total_mib: 8192,
            vram_free_mib: 7000,
            driver_version: "535.0".to_string(),
            pci_vendor_id: 0xDEAD,
            pci_device_id: 0xBEEF,
            arch: None,
            caps: anvilml_core::InferenceCaps::default(),
            enumeration_source: EnumerationSource::Fallback,
            capabilities_source: CapabilitySource::Fallback,
        };

        resolve_caps_from_row(&mut dev, None);

        // Name should be preserved (not overwritten).
        assert_eq!(dev.name, "Driver Reported Name");
        // Caps should be default.
        assert!(!dev.caps.fp16);
        assert!(!dev.caps.bf16);
        assert!(!dev.caps.flash_attention);
        assert!(matches!(
            dev.capabilities_source,
            CapabilitySource::Fallback
        ));
    }
}
