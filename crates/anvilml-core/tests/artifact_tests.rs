/// Tests for `types::artifact` — `ArtifactMeta`.
///
/// Verifies:
/// - JSON roundtrip for a fully-populated `ArtifactMeta`.
/// - Default `ArtifactMeta` produces a well-formed struct.
/// - A SHA-256 hex hash string roundtrips correctly through JSON.
use anvilml_core::ArtifactMeta;
use chrono::Utc;
use uuid::Uuid;

/// Verifies that a fully-populated `ArtifactMeta` serialises to JSON and
/// deserialises back to an identical value, including the `Uuid` job_id,
/// the `PathBuf` path field, and the `DateTime<Utc>` timestamp.
///
/// This is the primary acceptance test for the correctness of all
/// `Serialize`/`Deserialize` derives on `ArtifactMeta`.
#[test]
fn test_artifact_meta_json_roundtrip() {
    let artifact = ArtifactMeta {
        id: "artifact-001".to_string(),
        job_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        hash: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2".to_string(),
        width: 1920,
        height: 1080,
        path: "/artifacts/output_001.png".to_string(),
        size_bytes: 2_456_789,
        created_at: Utc::now(),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&artifact).expect("serialize ArtifactMeta to JSON");

    // Deserialize back — must not fail
    let restored: ArtifactMeta =
        serde_json::from_str(&json).expect("deserialize JSON back to ArtifactMeta");

    // All fields must be equal
    assert_eq!(restored.id, artifact.id);
    assert_eq!(restored.job_id, artifact.job_id);
    assert_eq!(restored.hash, artifact.hash);
    assert_eq!(restored.path, artifact.path);
    assert_eq!(restored.size_bytes, artifact.size_bytes);
    assert_eq!(restored.width, artifact.width);
    assert_eq!(restored.height, artifact.height);
    assert_eq!(restored.created_at, artifact.created_at);
}

/// Verifies that `ArtifactMeta` derives `Default` and produces a
/// well-formed struct with zero/empty defaults.
///
/// The default `id` is an empty string, `job_id` is the UUID zero value,
/// `hash` is an empty string, `path` is an empty string, and
/// `size_bytes` is `0`. The `created_at` timestamp defaults to the
/// UNIX epoch (1970-01-01T00:00:00Z).
///
/// This default is used as a placeholder in API scaffolding and test fixtures.
#[test]
fn test_artifact_meta_default() {
    let artifact = ArtifactMeta::default();
    assert!(
        artifact.id.is_empty(),
        "default ArtifactMeta id must be empty string"
    );
    assert!(
        artifact.job_id == Uuid::default(),
        "default ArtifactMeta job_id must be the UUID zero value"
    );
    assert!(
        artifact.hash.is_empty(),
        "default ArtifactMeta hash must be empty string"
    );
    assert!(
        artifact.path.is_empty(),
        "default ArtifactMeta path must be empty string"
    );
    assert_eq!(artifact.size_bytes, 0);
}

/// Verifies that a SHA-256 hex hash string (64 lowercase hex characters)
/// roundtrips correctly through JSON serialisation.
///
/// Ensures the `hash: String` field does not introduce any unexpected
/// escaping, truncation, or case transformation during serialization.
#[test]
fn test_artifact_hash_format() {
    // 64 lowercase hex characters — valid SHA-256 digest
    let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    assert_eq!(hash.len(), 64, "hash must be exactly 64 characters");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must contain only hex digits"
    );

    let artifact = ArtifactMeta {
        id: "hash-test".to_string(),
        job_id: Uuid::new_v4(),
        hash: hash.to_string(),
        width: 0,
        height: 0,
        path: "/tmp/test.bin".to_string(),
        size_bytes: 1024,
        created_at: Utc::now(),
    };

    let json = serde_json::to_string(&artifact).expect("serialize ArtifactMeta to JSON");
    let restored: ArtifactMeta =
        serde_json::from_str(&json).expect("deserialize JSON back to ArtifactMeta");

    assert_eq!(
        restored.hash, artifact.hash,
        "SHA-256 hash must survive JSON roundtrip"
    );
    assert_eq!(
        restored.width, artifact.width,
        "width must survive JSON roundtrip"
    );
    assert_eq!(
        restored.height, artifact.height,
        "height must survive JSON roundtrip"
    );
}
