//! Model metadata types for the AnvilML model registry.
//!
//! Defines `ModelMeta` (a persisted model record with path, kind, dtype, and format),
//! and the enums `ModelKind`, `ModelDtype`, and `ModelFormat` that classify models
//! by their role, precision, and file format.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Metadata for a scanned model file.
///
/// Produced by the model scanner (`anvilml-registry`) after walking configured
/// directories. Contains the information needed to index, search, and reference
/// a model without loading it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelMeta {
    /// Unique model identifier, assigned by the scanner.
    pub id: String,
    /// Human-readable model name (e.g. `"stable-diffusion-v1-5"`).
    pub name: String,
    /// Filesystem path to the model file or directory.
    pub path: String,
    /// Role of this model in a generative pipeline.
    pub kind: ModelKind,
    /// Data precision used by this model's weights.
    pub dtype: ModelDtype,
    /// Storage format of the model file.
    pub format: ModelFormat,
    /// Size of the model file on disk in bytes.
    pub size_bytes: u64,
    /// Timestamp when this model was last scanned.
    pub scanned_at: DateTime<Utc>,
}

/// Category of a model in a generative AI pipeline.
///
/// Each variant represents a distinct role that a model file may serve.
/// A single pipeline (e.g. Stable Diffusion) typically uses multiple models
/// of different kinds (diffusion, VAE, text encoder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Denoising diffusion model — the core generative component.
    Diffusion,
    /// Text embedding encoder (e.g. CLIP text encoder).
    TextEncoder,
    /// Variational autoencoder for encoding/decoding latent space.
    Vae,
    /// LoRA (Low-Rank Adaptation) fine-tuning weights.
    Lora,
    /// ControlNet model for conditional generation guidance.
    ControlNet,
    /// Super-resolution / upscaling model.
    Upscale,
    /// Model kind could not be determined from available metadata.
    Unknown,
}

/// Data precision (quantization) of a model's weights.
///
/// Higher precision values (Fp32) produce better quality but use more
/// memory and compute. Lower precision values (Fp8, Fp4) enable running
/// larger models on hardware with limited VRAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelDtype {
    /// 32-bit floating point — full precision.
    Fp32,
    /// 16-bit floating point — common for inference.
    Fp16,
    /// Brain floating point — 16-bit with extended range.
    Bf16,
    /// 8-bit floating point — quantized for efficiency.
    Fp8,
    /// 4-bit floating point — heavily quantized.
    Fp4,
    /// Precision could not be determined from available metadata.
    Unknown,
}

/// Storage format of a model file on disk.
///
/// Different formats use different serialization schemes. Safetensors is
/// the recommended format for AnvilML as it provides fast, safe loading
/// without arbitrary code execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    /// Safetensors format — safe, fast, language-agnostic.
    Safetensors,
    /// PyTorch checkpoint format (legacy `.ckpt`).
    Ckpt,
    /// PyTorch saved tensor format (`.pt` / `.bin`).
    Pt,
    /// Generic binary format (`.bin`).
    Bin,
    /// Format could not be determined from available metadata.
    Unknown,
}
