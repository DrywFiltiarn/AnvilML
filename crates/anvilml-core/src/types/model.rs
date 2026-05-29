//! Model metadata types — model identification, kind, and data type.
//!
//! All types are pure serializable data: zero I/O, zero async. They derive
//! `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ModelKind — the kind of ML model
// ---------------------------------------------------------------------------

/// The kind (category) of an ML model in the AnvilML system.
///
/// These match the MVP set from `ANVILML_DESIGN.md §4.2`.
#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    ToSchema,
)]
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

// ---------------------------------------------------------------------------
// DType — ML data type
// ---------------------------------------------------------------------------

/// The numeric data type used by a model's weights and activations.
#[derive(
    Serialize,
    Deserialize,
    Clone,
    Debug,
    PartialEq,
    Eq,
    ToSchema,
)]
pub enum DType {
    /// 32-bit IEEE 754 floating point (default).
    F32,
    /// 16-bit IEEE 754 half precision.
    F16,
    /// 8-bit signed integer (quantized).
    I8,
    /// Brain floating point 16-bit.
    BF16,
}

// ---------------------------------------------------------------------------
// ModelMeta — metadata about a registered model
// ---------------------------------------------------------------------------

/// Metadata describing a registered ML model in the system.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ToSchema)]
pub struct ModelMeta {
    /// Unique identifier for this model (UUID v4).
    pub id: Uuid,

    /// Human-readable name of the model.
    pub name: String,

    /// The kind/category of model.
    pub kind: ModelKind,

    /// Primary data type of the model weights.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtype: Option<DType>,

    /// Filesystem path to the model files.
    pub path: String,
}

impl ModelMeta {
    /// Create a new `ModelMeta` with default `DType::F32`.
    pub fn new(id: Uuid, name: String, kind: ModelKind, path: String) -> Self {
        Self {
            id,
            name,
            kind,
            dtype: Some(DType::F32),
            path,
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
        for dtype in [DType::F32, DType::F16, DType::I8, DType::BF16] {
            let json = serde_json::to_string(&dtype).unwrap();
            let back: DType = serde_json::from_str(&json).unwrap();
            assert_eq!(dtype, back, "failed for {:?}", dtype);
        }
    }

    #[test]
    fn dtype_eq() {
        assert_eq!(DType::F32, DType::F32);
        assert_ne!(DType::F16, DType::BF16);
    }

    // ------------------------------------------------------------------
    // ModelMeta — construction and serialization
    // ------------------------------------------------------------------

    #[test]
    fn model_meta_new() {
        let id = Uuid::new_v4();
        let meta = ModelMeta::new(
            id,
            "my-model".into(),
            ModelKind::Diffusion,
            "/models/my-model".into(),
        );
        assert_eq!(meta.id, id);
        assert_eq!(meta.name, "my-model");
        assert_eq!(meta.kind, ModelKind::Diffusion);
        assert_eq!(meta.dtype, Some(DType::F32));
        assert_eq!(meta.path, "/models/my-model");
    }

    #[test]
    fn model_meta_serialization_round_trip() {
        let meta = ModelMeta {
            id: Uuid::new_v4(),
            name: "test-model".into(),
            kind: ModelKind::Clip,
            dtype: Some(DType::F16),
            path: "/models/clip".into(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: ModelMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, back);
    }

    #[test]
    fn model_meta_skip_none_dtype() {
        // Manually construct with None dtype to test skip_serializing_if
        let meta = ModelMeta {
            id: Uuid::new_v4(),
            name: "minimal".into(),
            kind: ModelKind::Vae,
            dtype: None,
            path: "/models/vae".into(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("dtype"));
    }
}
