use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utoipa::ToSchema;

/// Metadata about a discovered model file.
///
/// This struct is the primary output of the model scanner: it captures
/// the stable identity, location, architecture family, data type, file
/// format, size, modification time, and scan timestamp of a single
/// model file on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ModelMeta {
    /// Stable identifier: SHA256 hex of the first 1 MiB of the file.
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Filesystem path to the model file.
    #[schema(value_type = String)]
    pub path: PathBuf,
    /// The model's architecture family.
    pub kind: ModelKind,
    /// The model's data type / precision.
    pub dtype: ModelDtype,
    /// The model file format.
    pub format: ModelFormat,
    /// File size in bytes.
    pub size_bytes: u64,
    /// File modification time as Unix epoch seconds (populated by the scanner).
    pub mtime_unix: i64,
    /// Timestamp when this metadata was scanned.
    pub scanned_at: DateTime<Utc>,
}

/// The architecture family of a model file.
///
/// Each variant names a distinct role a model plays in the AnvilML
/// computation graph — diffusion base model, text encoder, VAE, LoRA
/// adapter, ControlNet, or upscaler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// A diffusion base model (UNet or transformer).
    Diffusion,
    /// A CLIP / T5 text encoder.
    TextEncoder,
    /// A Variational Autoencoder for encoding/decoding latents.
    Vae,
    /// A LoRA (Low-Rank Adaptation) adapter weights.
    Lora,
    /// A ControlNet conditioning network.
    ControlNet,
    /// An image upscaler / super-resolution model.
    Upscale,
    /// The architecture family could not be determined.
    Unknown,
}

/// The data type or precision of a model's weights.
///
/// This enum covers the most common weight precisions encountered in
/// generative-AI model files, from full FP32 through FP4 quantisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelDtype {
    /// 32-bit floating point.
    Fp32,
    /// 16-bit floating point.
    Fp16,
    /// Brain floating point — 16-bit range with FP32 exponent.
    Bf16,
    /// 8-bit floating point.
    Fp8,
    /// 4-bit floating point.
    Fp4,
    /// The dtype could not be determined from the file.
    Unknown,
}

/// The storage format of a model file.
///
/// Each variant names a file format used to serialise model weights
/// on disk. The `Unknown` variant is used when the file extension or
/// header does not match any recognised format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    /// Safetensors — zero-code execution, memory-mapped loading.
    Safetensors,
    /// PyTorch checkpoint (.ckpt) — legacy format with optional code exec.
    Ckpt,
    /// Generic PyTorch tensor save (.pt / .pth).
    Pt,
    /// GGUF / general binary format.
    Bin,
    /// The file format could not be determined.
    Unknown,
}
