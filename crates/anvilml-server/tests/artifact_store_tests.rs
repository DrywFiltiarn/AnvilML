//! Integration tests for `ArtifactStore` content-addressed PNG storage.
//!
//! Tests cover: save-and-get roundtrip, deterministic hashing, list returns
//! saved artifact, save idempotency (INSERT OR IGNORE), and get returning
//! None for unknown hashes.

use anvilml_server::artifact::ArtifactStore;
use serial_test::serial;
use tempfile::TempDir;
use uuid::Uuid;

/// Build an in-memory `ArtifactStore` for tests.
///
/// Creates a fresh in-memory SQLite pool with migrations applied, then
/// constructs an `ArtifactStore` backed by a temporary directory.
/// The temp directory is returned so the test can inspect files on disk.
async fn test_store() -> (ArtifactStore, TempDir) {
    let pool = anvilml_registry::open_in_memory()
        .await
        .expect("in-memory pool for test store");
    let dir = tempfile::tempdir().expect("temp dir for test store");
    let store = ArtifactStore::new(dir.path().to_path_buf(), pool).await;
    (store, dir)
}

/// Verify that `save` writes a file to disk and records a row in the database,
/// and that `get` returns the correct path which, when read, contains the
/// exact input bytes.
///
/// Preconditions: None — the store is created fresh with an in-memory pool
/// and a temporary directory.
///
/// Steps:
/// 1. Create a known byte slice (64 bytes of PNG-like data).
/// 2. Call `save` with a random job_id.
/// 3. Call `get` with the returned hash.
/// 4. Verify the returned path exists and contains the exact input bytes.
#[tokio::test]
#[serial]
async fn test_save_and_get_roundtrip() {
    let (store, _dir) = test_store().await;

    // Use known bytes — a minimal PNG-like header for deterministic testing.
    // 64 bytes: 0x89 through 0xBE inclusive.
    let image_bytes: Vec<u8> = (0x89..=0xBE).collect();
    let job_id = Uuid::new_v4();

    let meta = store
        .save(job_id, &image_bytes)
        .await
        .expect("save should succeed");

    // Get the artifact by hash and verify the path exists with correct content.
    let path = store
        .get(&meta.hash)
        .await
        .expect("get should succeed")
        .expect("get should return Some for saved artifact");

    assert!(path.exists(), "artifact file should exist on disk");

    let written = tokio::fs::read(&path).await.expect("read artifact file");
    assert_eq!(
        written, image_bytes,
        "written file content must match input bytes exactly"
    );
}

/// Verify that calling `save` twice with identical bytes produces the same
/// hash both times, proving SHA-256 determinism.
///
/// Preconditions: None — the store is created fresh with an in-memory pool.
///
/// Steps:
/// 1. Create two different job_ids.
/// 2. Call `save` with the same bytes and different job_ids.
/// 3. Verify both `ArtifactMeta` results have identical `hash` values.
#[tokio::test]
#[serial]
async fn test_hash_is_deterministic() {
    let (store, _dir) = test_store().await;

    let image_bytes: Vec<u8> = (0x00..=0xFF).collect(); // 256 bytes
    let job_id_1 = Uuid::new_v4();
    let job_id_2 = Uuid::new_v4();

    let meta1 = store
        .save(job_id_1, &image_bytes)
        .await
        .expect("first save should succeed");
    let meta2 = store
        .save(job_id_2, &image_bytes)
        .await
        .expect("second save should succeed");

    assert_eq!(
        meta1.hash, meta2.hash,
        "SHA-256 hash must be deterministic for identical input bytes"
    );
}

/// Verify that `list(None)` returns all saved artifacts with correct metadata.
///
/// Preconditions: None — the store is created fresh with an in-memory pool.
///
/// Steps:
/// 1. Save an artifact with a known job_id and known bytes.
/// 2. Call `list(None)`.
/// 3. Verify the returned vector contains exactly one entry with the correct
///    job_id, hash, and size.
#[tokio::test]
#[serial]
async fn test_list_returns_saved_artifact() {
    let (store, _dir) = test_store().await;

    let image_bytes: Vec<u8> = (0xAA..=0xFF).collect(); // 96 bytes
    let job_id = Uuid::new_v4();

    let _meta = store
        .save(job_id, &image_bytes)
        .await
        .expect("save should succeed");

    let artifacts = store.list(None).await.expect("list should succeed");

    assert_eq!(
        artifacts.len(),
        1,
        "list should return exactly one artifact"
    );

    let artifact = &artifacts[0];
    assert_eq!(artifact.job_id, job_id, "artifact job_id must match");
    assert_eq!(
        artifact.size_bytes,
        image_bytes.len() as u64,
        "artifact size must match input bytes length"
    );
}

/// Verify that calling `save` twice with identical bytes does not create
/// duplicate database rows, proving the `INSERT OR IGNORE` idempotency.
///
/// Preconditions: None — the store is created fresh with an in-memory pool.
///
/// Steps:
/// 1. Save an artifact with known bytes and job_id.
/// 2. Save the same bytes again with the same job_id.
/// 3. Call `list(None)` and verify exactly one row exists.
/// 4. Verify both `save` calls returned the same hash.
#[tokio::test]
#[serial]
async fn test_save_is_idempotent() {
    let (store, _dir) = test_store().await;

    let image_bytes: Vec<u8> = (0x10..=0x5F).collect(); // 80 bytes
    let job_id = Uuid::new_v4();

    let meta1 = store
        .save(job_id, &image_bytes)
        .await
        .expect("first save should succeed");
    let meta2 = store
        .save(job_id, &image_bytes)
        .await
        .expect("second save should succeed");

    // Both saves must produce the same hash (deterministic content-addressing).
    assert_eq!(
        meta1.hash, meta2.hash,
        "both saves must produce the same hash"
    );

    // The database should have exactly one row — INSERT OR IGNORE prevents duplicates.
    let artifacts = store.list(None).await.expect("list should succeed");

    assert_eq!(
        artifacts.len(),
        1,
        "idempotent save must not create duplicate rows (expected 1, got {})",
        artifacts.len()
    );
}

/// Verify that `get` returns `None` for a hash that was never saved.
///
/// Preconditions: None — the store is created fresh with an in-memory pool
/// and no artifacts have been saved.
///
/// Steps:
/// 1. Call `get` with a random hex string that was never saved.
/// 2. Verify it returns `None`.
#[tokio::test]
#[serial]
async fn test_get_returns_none_for_unknown_hash() {
    let (store, _dir) = test_store().await;

    let result = store
        .get("nonexistent_hash_abcdef1234567890")
        .await
        .expect("get should not error for unknown hash");

    assert!(
        result.is_none(),
        "get should return None for a hash that was never saved"
    );
}
