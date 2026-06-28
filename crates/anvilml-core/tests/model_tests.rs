//! Tests for `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` serde roundtrips.
//!
//! All tests construct types via the public API, serialise to JSON,
//! deserialise back, and assert equality. No I/O or env vars are used.

use anvilml_core::types::*;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// Each of the seven `ModelKind` variants serialises to the correct `snake_case`
/// JSON string and roundtrips back to an equal value.
#[test]
fn test_model_kind_serde_snake_case() {
    let variants: [(ModelKind, &str); 7] = [
        (ModelKind::Diffusion, "diffusion"),
        (ModelKind::TextEncoder, "text_encoder"),
        (ModelKind::Vae, "vae"),
        (ModelKind::Lora, "lora"),
        (ModelKind::ControlNet, "control_net"),
        (ModelKind::Upscale, "upscale"),
        (ModelKind::Unknown, "unknown"),
    ];

    for (kind, expected_json) in variants {
        let json = serde_json::to_string(&kind).expect("failed to serialise ModelKind");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "ModelKind::{:?} JSON mismatch",
            kind
        );

        let roundtripped: ModelKind =
            serde_json::from_str(&json).expect("failed to deserialise ModelKind");
        assert_eq!(
            kind, roundtripped,
            "ModelKind::{:?} roundtrip mismatch",
            kind
        );
    }
}

/// Each of the six `ModelDtype` variants serialises to the correct `snake_case`
/// JSON string and roundtrips back to an equal value.
#[test]
fn test_model_dtype_serde_snake_case() {
    let variants: [(ModelDtype, &str); 6] = [
        (ModelDtype::Fp32, "fp32"),
        (ModelDtype::Fp16, "fp16"),
        (ModelDtype::Bf16, "bf16"),
        (ModelDtype::Fp8, "fp8"),
        (ModelDtype::Fp4, "fp4"),
        (ModelDtype::Unknown, "unknown"),
    ];

    for (dtype, expected_json) in variants {
        let json = serde_json::to_string(&dtype).expect("failed to serialise ModelDtype");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "ModelDtype::{:?} JSON mismatch",
            dtype
        );

        let roundtripped: ModelDtype =
            serde_json::from_str(&json).expect("failed to deserialise ModelDtype");
        assert_eq!(
            dtype, roundtripped,
            "ModelDtype::{:?} roundtrip mismatch",
            dtype
        );
    }
}

/// Each of the five `ModelFormat` variants serialises to the correct `snake_case`
/// JSON string and roundtrips back to an equal value.
#[test]
fn test_model_format_serde_snake_case() {
    let variants: [(ModelFormat, &str); 5] = [
        (ModelFormat::Safetensors, "safetensors"),
        (ModelFormat::Ckpt, "ckpt"),
        (ModelFormat::Pt, "pt"),
        (ModelFormat::Bin, "bin"),
        (ModelFormat::Unknown, "unknown"),
    ];

    for (format, expected_json) in variants {
        let json = serde_json::to_string(&format).expect("failed to serialise ModelFormat");
        assert_eq!(
            json,
            format!("\"{expected_json}\""),
            "ModelFormat::{:?} JSON mismatch",
            format
        );

        let roundtripped: ModelFormat =
            serde_json::from_str(&json).expect("failed to deserialise ModelFormat");
        assert_eq!(
            format, roundtripped,
            "ModelFormat::{:?} roundtrip mismatch",
            format
        );
    }
}

/// A `ModelMeta` with all fields populated serialises to JSON and roundtrips
/// back to an equal value, including the `PathBuf` → `String` conversion.
#[test]
fn test_model_meta_serde_roundtrip() {
    let meta = ModelMeta {
        id: "a1b2c3d4e5f6".to_string(),
        name: "test-model".to_string(),
        path: PathBuf::from("models/test.safetensors"),
        kind: ModelKind::Diffusion,
        dtype: ModelDtype::Fp16,
        format: ModelFormat::Safetensors,
        size_bytes: 6_442_529_280,
        scanned_at: DateTime::parse_from_rfc3339("2026-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc),
    };

    let json = serde_json::to_string(&meta).expect("failed to serialise ModelMeta");
    let roundtripped: ModelMeta =
        serde_json::from_str(&json).expect("failed to deserialise ModelMeta");

    assert_eq!(
        roundtripped, meta,
        "roundtripped ModelMeta does not equal original"
    );

    // Verify the JSON contains the expected snake_case field names.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert_eq!(parsed["id"], "a1b2c3d4e5f6");
    assert_eq!(parsed["name"], "test-model");
    assert_eq!(parsed["path"], "models/test.safetensors");
    assert_eq!(parsed["kind"], "diffusion");
    assert_eq!(parsed["dtype"], "fp16");
    assert_eq!(parsed["format"], "safetensors");
    assert_eq!(parsed["size_bytes"].as_u64(), Some(6_442_529_280));
}
