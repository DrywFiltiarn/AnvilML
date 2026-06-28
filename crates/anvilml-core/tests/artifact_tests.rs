//! Tests for `ArtifactMeta` serde roundtrips and JSON field verification.
//!
//! All tests construct types via the public API, serialise to JSON,
//! deserialise back, and assert equality. No I/O or env vars are used.

use anvilml_core::types::ArtifactMeta;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use uuid::Uuid;

/// A full `ArtifactMeta` with all fields populated serialises to JSON,
/// deserialises back to an equal value, and the raw JSON parses to
/// confirm field names are the expected snake_case identifiers.
#[test]
fn test_artifact_meta_serde_roundtrip() {
    let meta = ArtifactMeta {
        hash: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2".to_string(),
        job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        width: 1024,
        height: 1024,
        seed: 42,
        steps: 30,
        created_at: DateTime::parse_from_rfc3339("2026-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc),
        file_path: PathBuf::from("artifacts/a1b2c3d4.png"),
    };

    let json = serde_json::to_string(&meta).expect("failed to serialise ArtifactMeta");
    let roundtripped: ArtifactMeta =
        serde_json::from_str(&json).expect("failed to deserialise ArtifactMeta");

    assert_eq!(
        roundtripped, meta,
        "roundtripped ArtifactMeta does not equal original"
    );

    // Parse the JSON to confirm field names are correct snake_case identifiers.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");
    assert!(
        parsed.get("hash").is_some(),
        "JSON must contain 'hash' field"
    );
    assert!(
        parsed.get("job_id").is_some(),
        "JSON must contain 'job_id' field"
    );
    assert!(
        parsed.get("width").is_some(),
        "JSON must contain 'width' field"
    );
    assert!(
        parsed.get("height").is_some(),
        "JSON must contain 'height' field"
    );
    assert!(
        parsed.get("seed").is_some(),
        "JSON must contain 'seed' field"
    );
    assert!(
        parsed.get("steps").is_some(),
        "JSON must contain 'steps' field"
    );
    assert!(
        parsed.get("created_at").is_some(),
        "JSON must contain 'created_at' field"
    );
    assert!(
        parsed.get("file_path").is_some(),
        "JSON must contain 'file_path' field"
    );
}

/// A SHA-256 hex hash (64 lowercase hexadecimal characters) roundtrips
/// correctly through serde JSON — the `hash` field is the primary key
/// and must survive serialisation without any transformation.
#[test]
fn test_artifact_meta_hash_format() {
    let meta = ArtifactMeta {
        hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        width: 512,
        height: 512,
        seed: 0,
        steps: 1,
        created_at: DateTime::parse_from_rfc3339("2026-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&Utc),
        file_path: PathBuf::from("artifacts/0000.png"),
    };

    let json = serde_json::to_string(&meta).expect("failed to serialise ArtifactMeta");
    let roundtripped: ArtifactMeta =
        serde_json::from_str(&json).expect("failed to deserialise ArtifactMeta");

    // The hash must survive the roundtrip byte-for-byte.
    assert_eq!(
        roundtripped.hash, meta.hash,
        "SHA-256 hex hash must survive serde roundtrip unchanged"
    );

    // Verify the hash is exactly 64 lowercase hex characters.
    assert_eq!(
        meta.hash.len(),
        64,
        "SHA-256 hex hash must be 64 characters, got {}",
        meta.hash.len()
    );
    assert!(
        meta.hash.chars().all(|c| c.is_ascii_hexdigit()),
        "SHA-256 hex hash must contain only hex characters"
    );
}

/// The JSON output of `ArtifactMeta` contains all eight expected
/// snake_case field names with the correct types.
#[test]
fn test_artifact_meta_field_names() {
    let meta = ArtifactMeta {
        hash: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
        job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        width: 768,
        height: 1024,
        seed: -1,
        steps: 50,
        created_at: DateTime::parse_from_rfc3339("2026-06-28T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        file_path: PathBuf::from("artifacts/test.png"),
    };

    let json = serde_json::to_string(&meta).expect("failed to serialise ArtifactMeta");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json is valid");

    // Verify each field name and its expected type.
    assert_eq!(
        parsed["hash"],
        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
    );
    assert!(
        parsed["job_id"].is_string(),
        "job_id must be a string in JSON"
    );
    assert_eq!(parsed["width"], 768);
    assert_eq!(parsed["height"], 1024);
    assert_eq!(parsed["seed"], -1);
    assert_eq!(parsed["steps"], 50);
    assert!(
        parsed["created_at"].is_string(),
        "created_at must be a string (RFC 3339) in JSON"
    );
    assert_eq!(parsed["file_path"], "artifacts/test.png");

    // Verify that no unexpected fields are present — exactly 8 keys.
    assert_eq!(
        parsed
            .as_object()
            .expect("top-level must be an object")
            .len(),
        8,
        "JSON must contain exactly 8 fields"
    );
}
