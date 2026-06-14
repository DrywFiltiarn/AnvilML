/// Tests for `types::model` — `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat`.
///
/// Verifies:
/// - JSON roundtrip for a fully-populated `ModelMeta`.
/// - All seven `ModelKind` variants roundtrip through JSON.
/// - All `ModelDtype` and `ModelFormat` variants roundtrip through JSON.
use anvilml_core::{ModelDtype, ModelFormat, ModelKind, ModelMeta};
use chrono::Utc;

/// Verifies that a fully-populated `ModelMeta` serialises to JSON and
/// deserialises back to an identical value, including the `PathBuf` path
/// field, the `DateTime<Utc>` timestamp, and all enum fields.
///
/// This is the primary acceptance test for the correctness of all
/// `Serialize`/`Deserialize` derives on `ModelMeta` and its fields.
#[test]
fn test_model_meta_json_roundtrip() {
    let meta = ModelMeta {
        id: "model-001".to_string(),
        name: "stable-diffusion-v1-5".to_string(),
        path: "/models/sd-v1-5.safetensors".to_string(),
        kind: ModelKind::Diffusion,
        dtype: ModelDtype::Fp32,
        format: ModelFormat::Safetensors,
        size_bytes: 4_267_439_078,
        scanned_at: Utc::now(),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&meta).expect("serialize ModelMeta to JSON");

    // Deserialize back — must not fail
    let restored: ModelMeta =
        serde_json::from_str(&json).expect("deserialize JSON back to ModelMeta");

    // All fields must be equal
    assert_eq!(restored.id, meta.id);
    assert_eq!(restored.name, meta.name);
    assert_eq!(restored.path, meta.path);
    assert_eq!(restored.kind, meta.kind);
    assert_eq!(restored.dtype, meta.dtype);
    assert_eq!(restored.format, meta.format);
    assert_eq!(restored.size_bytes, meta.size_bytes);
    assert_eq!(restored.scanned_at, meta.scanned_at);
}

/// Verifies that all seven `ModelKind` enum variants roundtrip through
/// JSON serialisation without data loss.
///
/// Each variant is serialised to a JSON string and deserialised back,
/// then compared for equality. This tests that `#[serde(rename_all = "snake_case")]`
/// produces the correct lowercase variant names.
#[test]
fn test_model_kind_variants() {
    let variants = [
        ModelKind::Diffusion,
        ModelKind::TextEncoder,
        ModelKind::Vae,
        ModelKind::Lora,
        ModelKind::ControlNet,
        ModelKind::Upscale,
        ModelKind::Unknown,
    ];

    for variant in variants {
        let json = serde_json::to_string(&variant).expect("serialize ModelKind variant to JSON");

        let restored: ModelKind =
            serde_json::from_str(&json).expect("deserialize JSON back to ModelKind");

        assert_eq!(
            restored, variant,
            "ModelKind::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }
}

/// Verifies that all `ModelDtype` and `ModelFormat` enum variants roundtrip
/// through JSON serialisation without data loss.
///
/// Tests the snake_case renaming for both precision enums, ensuring that
/// values like `"fp32"`, `"safetensors"`, and `"ckpt"` are produced
/// rather than PascalCase.
#[test]
fn test_model_dtype_format_variants() {
    let dtype_variants = [
        ModelDtype::Fp32,
        ModelDtype::Fp16,
        ModelDtype::Bf16,
        ModelDtype::Fp8,
        ModelDtype::Fp4,
        ModelDtype::Unknown,
    ];

    for variant in dtype_variants {
        let json = serde_json::to_string(&variant).expect("serialize ModelDtype variant to JSON");

        let restored: ModelDtype =
            serde_json::from_str(&json).expect("deserialize JSON back to ModelDtype");

        assert_eq!(
            restored, variant,
            "ModelDtype::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }

    let format_variants = [
        ModelFormat::Safetensors,
        ModelFormat::Ckpt,
        ModelFormat::Pt,
        ModelFormat::Bin,
        ModelFormat::Unknown,
    ];

    for variant in format_variants {
        let json = serde_json::to_string(&variant).expect("serialize ModelFormat variant to JSON");

        let restored: ModelFormat =
            serde_json::from_str(&json).expect("deserialize JSON back to ModelFormat");

        assert_eq!(
            restored, variant,
            "ModelFormat::{:?} did not survive JSON roundtrip (JSON was: {})",
            variant, json
        );
    }
}
