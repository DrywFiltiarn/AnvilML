//! Model domain types per ANVILML_DESIGN §4.2.
//!
//! Defines `DType`, `ModelMeta` and re-exports the existing `ModelKind`
//! from the config module to avoid duplication within this crate.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Re-use the existing ModelKind from config to avoid duplication.
pub use crate::config::ModelKind;

/// Data type of model weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    /// 32-bit floating point.
    F32,
    /// 16-bit IEEE floating point.
    F16,
    /// Brain floating point (16-bit).
    BF16,
    /// 8-bit float E4M3, torch float8_e4m3fn.
    #[serde(rename = "f8_e4m3")]
    F8E4M3,
    /// 8-bit float E5M2, torch float8_e5m2.
    #[serde(rename = "f8_e5m2")]
    F8E5M2,
    /// 8-bit integer quantization.
    Q8,
    /// 4-bit integer quantization.
    Q4,
    /// Unknown or unspecified data type.
    #[default]
    Unknown,
}

/// Metadata about a scanned model file.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelMeta {
    /// Unique identifier for the model (opaque string).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Filesystem path to the model weights.
    #[schema(value_type = String)]
    pub path: PathBuf,
    /// Kind/category of the model.
    pub kind: ModelKind,
    /// Size of the model file in bytes.
    pub size_bytes: u64,
    /// Hint about the data type of the model weights.
    #[serde(default)]
    pub dtype_hint: DType,
    /// Estimated VRAM consumption in MiB.
    #[serde(default)]
    pub vram_estimate_mib: u32,
    /// When this model was last scanned by the registry.
    #[serde(default = "default_scanned_at")]
    pub scanned_at: DateTime<Utc>,
}

fn default_scanned_at() -> DateTime<Utc> {
    Utc::now()
}

impl Default for ModelMeta {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            path: PathBuf::new(),
            kind: ModelKind::default(),
            size_bytes: 0,
            dtype_hint: DType::default(),
            vram_estimate_mib: 0,
            scanned_at: default_scanned_at(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtype_variants() {
        let variants: Vec<DType> = vec![
            DType::F32,
            DType::F16,
            DType::BF16,
            DType::F8E4M3,
            DType::F8E5M2,
            DType::Q8,
            DType::Q4,
            DType::Unknown,
        ];

        assert_eq!(variants.len(), 8, "must have exactly 8 variants");

        // All variants must be distinct.
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert_ne!(variants[i], variants[j]);
            }
        }

        // Self-equality.
        assert_eq!(DType::F32, DType::F32);
        assert_eq!(DType::Unknown, DType::Unknown);
    }

    #[test]
    fn dtype_default_is_unknown() {
        assert_eq!(DType::default(), DType::Unknown);
    }

    #[test]
    fn dtype_roundtrip_json() {
        let dtypes = [
            DType::F32,
            DType::F16,
            DType::BF16,
            DType::F8E4M3,
            DType::F8E5M2,
            DType::Q8,
            DType::Q4,
            DType::Unknown,
        ];
        for &dtype in &dtypes {
            let json = serde_json::to_string(&dtype).expect("serialize DType");
            let restored: DType = serde_json::from_str(&json).expect("deserialize DType");
            assert_eq!(restored, dtype);
        }
    }

    #[test]
    fn dtype_f8_serde_strings() {
        assert_eq!(
            serde_json::to_string(&DType::F8E4M3).expect("serialize F8E4M3"),
            "\"f8_e4m3\""
        );
        assert_eq!(
            serde_json::to_string(&DType::F8E5M2).expect("serialize F8E5M2"),
            "\"f8_e5m2\""
        );
    }

    #[test]
    fn model_meta_roundtrip() {
        let now = Utc::now();
        let meta = ModelMeta {
            id: "model-001".to_string(),
            name: "Stable Diffusion XL".to_string(),
            path: PathBuf::from("/models/sdxl/v1.safetensors"),
            kind: ModelKind::Diffusion,
            size_bytes: 6_700_000_000,
            dtype_hint: DType::F16,
            vram_estimate_mib: 8192,
            scanned_at: now,
        };

        let json = serde_json::to_string(&meta).expect("serialize ModelMeta");
        let restored: ModelMeta = serde_json::from_str(&json).expect("deserialize ModelMeta");

        assert_eq!(restored.id, meta.id);
        assert_eq!(restored.name, meta.name);
        assert_eq!(restored.path, meta.path);
        assert_eq!(restored.kind, meta.kind);
        assert_eq!(restored.size_bytes, meta.size_bytes);
        assert_eq!(restored.dtype_hint, meta.dtype_hint);
        assert_eq!(restored.vram_estimate_mib, meta.vram_estimate_mib);
        assert_eq!(restored.scanned_at, meta.scanned_at);
    }

    #[test]
    fn model_meta_defaults() {
        let minimal = serde_json::json!({
            "id": "model-002",
            "name": "Minimal Model",
            "path": "/models/minimal.safetensors",
            "kind": "upscale",
            "size_bytes": 100,
        });

        let meta: ModelMeta = serde_json::from_value(minimal).expect("minimal ModelMeta parses");

        assert_eq!(meta.id, "model-002");
        assert_eq!(meta.name, "Minimal Model");
        assert_eq!(meta.path, PathBuf::from("/models/minimal.safetensors"));
        assert_eq!(meta.kind, ModelKind::Upscale);
        assert_eq!(meta.size_bytes, 100);
        assert_eq!(meta.dtype_hint, DType::Unknown);
        assert_eq!(meta.vram_estimate_mib, 0);
    }

    #[test]
    fn model_meta_default_impl() {
        let meta = ModelMeta::default();
        assert!(meta.id.is_empty());
        assert!(meta.name.is_empty());
        assert!(meta.path.as_os_str().is_empty());
        assert_eq!(meta.kind, ModelKind::Upscale);
        assert_eq!(meta.size_bytes, 0);
        assert_eq!(meta.dtype_hint, DType::Unknown);
        assert_eq!(meta.vram_estimate_mib, 0);
    }

    #[test]
    fn model_meta_scanned_at_default() {
        let minimal = serde_json::json!({
            "id": "model-003",
            "name": "No Timestamp",
            "path": "/models/no_ts.safetensors",
            "kind": "clip",
            "size_bytes": 50,
        });

        let meta: ModelMeta = serde_json::from_value(minimal).expect("parses without scanned_at");
        // When scanned_at is absent, it should default to Utc::now() —
        // we only verify it's a valid DateTime (non-zero).
        assert!(meta.scanned_at.timestamp_millis() > 0);
    }

    #[test]
    fn model_meta_serde_json_preserves_all_fields() {
        let meta = ModelMeta {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            path: PathBuf::from("/tmp/test.safetensors"),
            kind: ModelKind::Lora,
            size_bytes: 42,
            dtype_hint: DType::Q8,
            vram_estimate_mib: 512,
            scanned_at: Utc::now(),
        };

        let json = serde_json::to_string_pretty(&meta).expect("serialize ModelMeta");
        assert!(json.contains("\"id\": \"test-model\""));
        assert!(json.contains("\"kind\": \"lora\""));
        assert!(json.contains("\"dtype_hint\": \"q8\""));
        assert!(json.contains("\"vram_estimate_mib\": 512"));
    }
}
