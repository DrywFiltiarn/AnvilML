//! Model metadata types — model identification, kind, and data type.
//!
//! All types are pure serializable data: zero I/O, zero async. They derive
//! `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// ModelKind — the kind of ML model
// ---------------------------------------------------------------------------

/// The kind (category) of an ML model in the AnvilML system.
///
/// These match the MVP set from `ANVILML_DESIGN.md §4.2`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, ToSchema)]
pub enum ModelKind {
    /// Classifier / text encoder model (e.g. CLIP).
    Clip,
    /// Text-to-image diffusion model.
    Diffusion,
    /// Variational autoencoder.
    Vae,
    /// Low-rank adaptation LoRA weights.
    Lora,
    /// Spatial conditioning network.
    ControlNet,
    /// U-Net backbone (standalone).
    Unet,
    /// Super-resolution / upscaling model.
    Upscale,
}

impl From<crate::config::ModelKind> for ModelKind {
    fn from(k: crate::config::ModelKind) -> Self {
        match k {
            crate::config::ModelKind::Clip => ModelKind::Clip,
            crate::config::ModelKind::Diffusion => ModelKind::Diffusion,
            crate::config::ModelKind::Vae => ModelKind::Vae,
            crate::config::ModelKind::Lora => ModelKind::Lora,
            crate::config::ModelKind::ControlNet => ModelKind::ControlNet,
            crate::config::ModelKind::Unet => ModelKind::Unet,
            crate::config::ModelKind::Upscale => ModelKind::Upscale,
        }
    }
}

// ---------------------------------------------------------------------------
// DType — ML data type
// ---------------------------------------------------------------------------

/// The numeric data type used by a model's weights and activations.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, ToSchema)]
pub enum DType {
    /// 32-bit IEEE 754 floating point (default).
    F32,
    /// 16-bit IEEE 754 half precision.
    F16,
    /// 8-bit signed integer (quantized).
    I8,
    /// Brain floating point 16-bit.
    BF16,
    /// 8-bit quantized (int8-like, often used for LLMs).
    Q8,
    /// 4-bit quantized (int4-like, aggressive compression).
    Q4,
    /// Unknown / future dtype — non-exhaustive sentinel.
    Unknown,
}

// ---------------------------------------------------------------------------
// ModelMeta — metadata about a registered model
// ---------------------------------------------------------------------------

/// Metadata describing a registered ML model in the system.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct ModelMeta {
    /// Unique identifier for this model (first 16 hex chars of SHA256 of the
    /// canonical path string).
    pub id: String,

    /// Human-readable name of the model.
    pub name: String,

    /// The kind/category of model.
    pub kind: ModelKind,

    /// Primary data type of the model weights (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtype: Option<DType>,

    /// Inferred data type hint from filename/dirname patterns.
    pub dtype_hint: DType,

    /// Filesystem path to the model files.
    pub path: String,

    /// File size in bytes.
    pub size_bytes: u64,

    /// Estimated VRAM usage in mebibytes (based on size_bytes and dtype factor).
    pub vram_estimate_mib: u64,

    /// ISO 8601 UTC timestamp when this model was last scanned.
    pub scanned_at: String,
}

impl ModelMeta {
    /// Create a new `ModelMeta` with default `DType::F32` dtype_hint and
    /// `scanned_at` set to the current time.
    pub fn new(
        id: String,
        name: String,
        kind: ModelKind,
        path: String,
        size_bytes: u64,
        vram_estimate_mib: u64,
    ) -> Self {
        use chrono::Utc;
        Self {
            id,
            name,
            kind,
            dtype: Some(DType::F32),
            dtype_hint: DType::F32,
            path,
            size_bytes,
            vram_estimate_mib,
            scanned_at: Utc::now().to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // ModelKind — serialization round-trip
    // ------------------------------------------------------------------

    #[test]
    fn model_kind_serialization_round_trip() {
        for kind in [
            ModelKind::Clip,
            ModelKind::Diffusion,
            ModelKind::Vae,
            ModelKind::Lora,
            ModelKind::ControlNet,
            ModelKind::Unet,
            ModelKind::Upscale,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: ModelKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back, "failed for {:?}", kind);
        }
    }

    #[test]
    fn model_kind_eq() {
        assert_eq!(ModelKind::Diffusion, ModelKind::Diffusion);
        assert_ne!(ModelKind::Clip, ModelKind::Diffusion);
    }

    // ------------------------------------------------------------------
    // DType — serialization round-trip
    // ------------------------------------------------------------------

    #[test]
    fn dtype_serialization_round_trip() {
        for dtype in [
            DType::F32,
            DType::F16,
            DType::I8,
            DType::BF16,
            DType::Q8,
            DType::Q4,
            DType::Unknown,
        ] {
            let json = serde_json::to_string(&dtype).unwrap();
            let back: DType = serde_json::from_str(&json).unwrap();
            assert_eq!(dtype, back, "failed for {:?}", dtype);
        }
    }

    #[test]
    fn dtype_eq() {
        assert_eq!(DType::F32, DType::F32);
        assert_ne!(DType::F16, DType::BF16);
        assert_eq!(DType::Q8, DType::Q8);
        assert_eq!(DType::Q4, DType::Q4);
        assert_eq!(DType::Unknown, DType::Unknown);
    }

    // ------------------------------------------------------------------
    // ModelMeta — construction and serialization
    // ------------------------------------------------------------------

    #[test]
    fn model_meta_new() {
        let meta = ModelMeta::new(
            "abc123def4567890".into(),
            "my-model".into(),
            ModelKind::Diffusion,
            "/models/my-model".into(),
            1_000_000_000,
            500,
        );
        assert_eq!(meta.id, "abc123def4567890");
        assert_eq!(meta.name, "my-model");
        assert_eq!(meta.kind, ModelKind::Diffusion);
        assert_eq!(meta.dtype, Some(DType::F32));
        assert_eq!(meta.dtype_hint, DType::F32);
        assert_eq!(meta.path, "/models/my-model");
        assert_eq!(meta.size_bytes, 1_000_000_000);
        assert_eq!(meta.vram_estimate_mib, 500);
        assert!(!meta.scanned_at.is_empty());
    }

    #[test]
    fn model_meta_serialization_round_trip() {
        let meta = ModelMeta {
            id: "sha256hex12345678".into(),
            name: "test-model".into(),
            kind: ModelKind::Clip,
            dtype: Some(DType::F16),
            dtype_hint: DType::F16,
            path: "/models/clip".into(),
            size_bytes: 500_000_000,
            vram_estimate_mib: 250,
            scanned_at: "2026-01-01T00:00:00+00:00".into(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: ModelMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, back);
    }

    #[test]
    fn model_meta_skip_none_dtype() {
        // Manually construct with None dtype to test skip_serializing_if
        let meta = ModelMeta {
            id: "sha256hex12345678".into(),
            name: "minimal".into(),
            kind: ModelKind::Vae,
            dtype: None,
            dtype_hint: DType::F32,
            path: "/models/vae".into(),
            size_bytes: 100_000_000,
            vram_estimate_mib: 50,
            scanned_at: "2026-01-01T00:00:00+00:00".into(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains(",\"dtype\":"));
    }
}
