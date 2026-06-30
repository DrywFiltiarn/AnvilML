//! Integration tests for `ArtifactStore` — content-addressed PNG artifact save.
//!
//! Each test creates its own in-memory SQLite pool with a unique cache name
//! (uuid-based) and its own temp directory, so there is no cross-test shared
//! state and no `#[serial]` annotation is needed.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::ArtifactMeta;
use chrono::Utc;
use sha2::Digest;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::fs;
use std::path::PathBuf;

/// A minimal 64×64 black PNG for testing.
///
/// This is a valid PNG file (137 80 78 71 header + IHDR + IDAT + IEND chunks)
/// that is small enough to be convenient in tests but large enough to exercise
/// the hash computation meaningfully.
const TEST_PNG: &[u8] = include_bytes!("fixtures/test_64x64_black.png");

/// A minimal 64×64 white PNG for testing (different content = different hash).
///
/// Same dimensions as TEST_PNG but with inverted pixel data.
const TEST_PNG_WHITE: &[u8] = include_bytes!("fixtures/test_64x64_white.png");

/// Create an in-memory SQLite pool with a unique cache name.
///
/// Each test gets its own pool — the in-memory database is isolated per
/// connection by using a unique cache name (uuid-based) so parallel tests
/// don't collide on the shared `:memory:` database.
///
/// No migrations are applied here — the `artifacts` table is created
/// automatically on first save via `CREATE TABLE IF NOT EXISTS`.
async fn make_pool() -> SqlitePool {
    // Use a unique in-memory database name per test to avoid the shared
    // `:memory:` database problem: without a unique name, all connections
    // in the same process share the same in-memory database, causing
    // cross-test interference.
    let unique_name = uuid::Uuid::new_v4().to_string();

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(format!("file:{unique_name}?mode=memory&cache=shared"))
                .create_if_missing(true),
        )
        .await
        .expect("should be able to create in-memory SQLite pool")
}

/// Construct an `ArtifactMeta` with test values.
///
/// The file_path is a synthetic temporary path — it does not need to
/// exist on disk because the store only persists metadata, not the file
/// itself. The hash field is set to a placeholder; the actual hash is
/// computed from the PNG bytes by `save()`.
fn test_meta() -> ArtifactMeta {
    ArtifactMeta {
        hash: "placeholder".to_string(),
        job_id: uuid::Uuid::new_v4(),
        width: 64,
        height: 64,
        seed: 42,
        steps: 20,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/artifacts/placeholder.png"),
    }
}

/// `save()` writes the PNG file to `{artifact_dir}/{hash}.png`, returns the
/// correct SHA-256 hex hash, and persists the metadata row.
///
/// Creates a tempdir and an `ArtifactStore` pointing to it (with an in-memory
/// SQLite pool), calls `save()` with a known PNG byte slice, then verifies:
/// the file exists at the expected path, the file size matches the input,
/// the returned hash matches the computed SHA-256 of the input, and the DB
/// row exists.
#[tokio::test]
async fn test_save_writes_file_once() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    // Clone the pool so both the store and the test can use it — the test
    // queries the DB directly to verify the row was persisted.
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool.clone());

    let meta = test_meta();
    let hash = store
        .save(TEST_PNG, &meta)
        .await
        .expect("save should succeed");

    // Verify the file exists at the expected content-addressed path.
    let expected_path = tempdir.path().join(format!("{hash}.png"));
    assert!(
        expected_path.exists(),
        "artifact file should exist at {expected_path:?}"
    );

    // Verify the file size matches the input PNG bytes.
    let metadata = fs::metadata(&expected_path).expect("file metadata should be readable");
    assert_eq!(
        metadata.len() as usize,
        TEST_PNG.len(),
        "file size should match input PNG size"
    );

    // Verify the returned hash matches the computed SHA-256 of the input.
    let expected_hash: String = sha2::Sha256::new()
        .chain_update(TEST_PNG)
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    assert_eq!(
        hash, expected_hash,
        "returned hash should match SHA-256 of input"
    );

    // Verify the DB row exists by querying the artifacts table.
    // Use the test's own pool variable (not the private store.pool field).
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifacts")
        .fetch_one(&pool)
        .await
        .expect("query should succeed");
    assert_eq!(
        count, 1,
        "exactly one row should exist in the artifacts table"
    );
}

/// Calling `save()` twice with identical bytes does not create a second file
/// and does not return an error.
///
/// Same setup as `test_save_writes_file_once`, but calls `save()` twice with
/// the same PNG bytes. Asserts: exactly 1 file in the artifact directory,
/// both calls return `Ok(hash)`, and the file content is unchanged.
#[tokio::test]
async fn test_duplicate_save_does_not_duplicate_or_error() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    let meta = test_meta();

    // First save — should create the file.
    let hash1 = store
        .save(TEST_PNG, &meta)
        .await
        .expect("first save should succeed");

    // Second save with identical bytes — should not error.
    let hash2 = store
        .save(TEST_PNG, &meta)
        .await
        .expect("duplicate save should not error");

    // Both calls should return the same hash.
    assert_eq!(hash1, hash2, "both saves should return the same hash");

    // Exactly 1 file should exist in the artifact directory.
    let file_count = fs::read_dir(tempdir.path())
        .expect("artifact dir should be readable")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "png"))
        .count();
    assert_eq!(
        file_count, 1,
        "exactly 1 PNG file should exist, got {file_count}"
    );

    // File content should be identical to the original PNG.
    let written = fs::read(tempdir.path().join(format!("{hash1}.png")))
        .expect("written file should be readable");
    assert_eq!(
        written, TEST_PNG,
        "file content should match original PNG bytes"
    );
}

/// Two different PNG byte slices produce two different hashes and two
/// different files.
///
/// Creates a tempdir and `ArtifactStore`, calls `save()` with two different
/// PNG byte slices, then verifies: both files exist, the two hashes are
/// different, and each file's content matches its corresponding input.
#[tokio::test]
async fn test_different_content_produces_different_hash() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    let meta1 = test_meta();
    let meta2 = ArtifactMeta {
        seed: 137,
        ..meta1.clone()
    };

    let hash1 = store
        .save(TEST_PNG, &meta1)
        .await
        .expect("first save should succeed");
    let hash2 = store
        .save(TEST_PNG_WHITE, &meta2)
        .await
        .expect("second save should succeed");

    // The two hashes must be different.
    assert_ne!(
        hash1, hash2,
        "different PNG content should produce different hashes"
    );

    // Both files should exist.
    let path1 = tempdir.path().join(format!("{hash1}.png"));
    let path2 = tempdir.path().join(format!("{hash2}.png"));
    assert!(path1.exists(), "first artifact file should exist");
    assert!(path2.exists(), "second artifact file should exist");

    // Each file's content should match its corresponding input.
    let written1 = fs::read(&path1).expect("first file should be readable");
    let written2 = fs::read(&path2).expect("second file should be readable");
    assert_eq!(
        written1, TEST_PNG,
        "first file content should match first PNG input"
    );
    assert_eq!(
        written2, TEST_PNG_WHITE,
        "second file content should match second PNG input"
    );
}

/// `save()` followed by `get()` returns the exact original PNG bytes.
///
/// Creates a tempdir and `ArtifactStore`, calls `save()` with a known PNG,
/// then calls `get()` with the returned hash and verifies the bytes match
/// the original input exactly.
#[tokio::test]
async fn test_save_then_get_roundtrips() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    let meta = test_meta();
    let hash = store
        .save(TEST_PNG, &meta)
        .await
        .expect("save should succeed");

    // Retrieve the artifact by hash.
    let retrieved = store.get(&hash).await.expect("get should succeed");

    // The retrieved bytes must match the original PNG exactly.
    assert!(
        retrieved.is_some(),
        "get should return Some for a hash that was just saved"
    );
    assert_eq!(
        retrieved.unwrap(),
        TEST_PNG,
        "retrieved bytes should match original PNG content"
    );
}

/// `get()` on a hash that was never saved returns `Ok(None)`.
///
/// Creates an empty tempdir and `ArtifactStore`, then calls `get()` with a
/// random hash that does not correspond to any saved file. Asserts the
/// result is `Ok(None)` — not an error, not `Some`.
#[tokio::test]
async fn test_get_unknown_hash_returns_none() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    // Use a hash that was never saved — a string of hex digits that
    // does not match any file in the artifact directory.
    let unknown_hash = "0000000000000000000000000000000000000000000000000000000000000000";

    let result = store
        .get(unknown_hash)
        .await
        .expect("get should not error for an unknown hash");

    assert!(
        result.is_none(),
        "get should return None for a hash that was never saved"
    );
}

/// After saving two different PNGs, `get()` for each hash returns the
/// correct file's content — not the other file's content.
///
/// Creates a tempdir and `ArtifactStore`, saves two different PNGs
/// (black and white), then calls `get()` for each hash and verifies
/// each returns its own content, proving content-addressed retrieval
/// is not confused by having multiple files in the same directory.
#[tokio::test]
async fn test_get_after_duplicate_save_returns_original_content() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    let meta1 = test_meta();
    let meta2 = ArtifactMeta {
        seed: 137,
        ..meta1.clone()
    };

    // Save two different PNGs.
    let hash1 = store
        .save(TEST_PNG, &meta1)
        .await
        .expect("first save should succeed");
    let hash2 = store
        .save(TEST_PNG_WHITE, &meta2)
        .await
        .expect("second save should succeed");

    // The two hashes must be different.
    assert_ne!(
        hash1, hash2,
        "different PNGs should produce different hashes"
    );

    // Retrieve each by its own hash and verify the content.
    let retrieved1 = store
        .get(&hash1)
        .await
        .expect("get for first hash should succeed");
    let retrieved2 = store
        .get(&hash2)
        .await
        .expect("get for second hash should succeed");

    assert_eq!(
        retrieved1.unwrap(),
        TEST_PNG,
        "get(hash1) should return the first PNG's content, not the second's"
    );
    assert_eq!(
        retrieved2.unwrap(),
        TEST_PNG_WHITE,
        "get(hash2) should return the second PNG's content, not the first's"
    );
}

/// `list(Some(job_id))` returns only artifacts matching the given job ID.
///
/// Creates a tempdir and `ArtifactStore`, saves two artifacts under different
/// job IDs, then calls `list(Some(job_id_a))` and verifies only the artifact
/// with the matching job ID is returned.
#[tokio::test]
async fn test_list_with_job_id_filter() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    // Generate two distinct job IDs.
    let job_id_a = uuid::Uuid::new_v4();
    let job_id_b = uuid::Uuid::new_v4();

    // Save one artifact under job_id_a.
    let meta_a = ArtifactMeta {
        hash: "placeholder_a".to_string(),
        job_id: job_id_a,
        width: 64,
        height: 64,
        seed: 42,
        steps: 20,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/artifacts/a.png"),
    };
    store
        .save(TEST_PNG, &meta_a)
        .await
        .expect("first save should succeed");

    // Save one artifact under job_id_b.
    let meta_b = ArtifactMeta {
        hash: "placeholder_b".to_string(),
        job_id: job_id_b,
        width: 64,
        height: 64,
        seed: 42,
        steps: 20,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/artifacts/b.png"),
    };
    store
        .save(TEST_PNG_WHITE, &meta_b)
        .await
        .expect("second save should succeed");

    // List with job_id_a filter — should return exactly 1 row.
    let results = store
        .list(Some(job_id_a))
        .await
        .expect("list with job_id filter should succeed");
    assert_eq!(
        results.len(),
        1,
        "list(Some(job_id_a)) should return exactly 1 artifact, got {}",
        results.len()
    );
    assert_eq!(
        results[0].job_id, job_id_a,
        "the returned artifact should have job_id_a"
    );
}

/// `list(None)` returns all artifact rows regardless of job ID.
///
/// Creates a tempdir and `ArtifactStore`, saves three artifacts under
/// two different job IDs, then calls `list(None)` and verifies all
/// three rows are returned.
#[tokio::test]
async fn test_list_without_filter_returns_all() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    let job_id_a = uuid::Uuid::new_v4();
    let job_id_b = uuid::Uuid::new_v4();

    // Save three artifacts under two different job IDs — use three unique
    // byte slices so each produces a different hash (same-content saves are
    // idempotent via INSERT OR IGNORE, so we need distinct content for 3 rows).
    let meta_a1 = ArtifactMeta {
        hash: "placeholder_a1".to_string(),
        job_id: job_id_a,
        width: 64,
        height: 64,
        seed: 42,
        steps: 20,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/artifacts/a1.png"),
    };
    store
        .save(TEST_PNG, &meta_a1)
        .await
        .expect("first save should succeed");

    let meta_a2 = ArtifactMeta {
        hash: "placeholder_a2".to_string(),
        job_id: job_id_a,
        width: 64,
        height: 64,
        seed: 137,
        steps: 30,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/artifacts/a2.png"),
    };
    store
        .save(TEST_PNG_WHITE, &meta_a2)
        .await
        .expect("second save should succeed");

    // Create a third unique byte slice by modifying TEST_PNG — flipping a
    // byte ensures a different SHA-256 hash while keeping the same format.
    let mut modified_png = TEST_PNG.to_vec();
    modified_png[10] ^= 0xff;

    let meta_b = ArtifactMeta {
        hash: "placeholder_b".to_string(),
        job_id: job_id_b,
        width: 64,
        height: 64,
        seed: 42,
        steps: 20,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/artifacts/b.png"),
    };
    store
        .save(&modified_png, &meta_b)
        .await
        .expect("third save should succeed");

    // List without filter — should return all 3 rows.
    let results = store
        .list(None)
        .await
        .expect("list without filter should succeed");
    assert_eq!(
        results.len(),
        3,
        "list(None) should return all 3 artifacts, got {}",
        results.len()
    );
}

/// `list(None)` on an empty table returns an empty `Vec`, not an error.
///
/// Creates a tempdir and `ArtifactStore` with no saves, then calls
/// `list(None)` and verifies the result is an empty vector with
/// `len() == 0`.
#[tokio::test]
async fn test_list_empty_table_returns_empty_vec() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let pool = make_pool().await;
    let store = ArtifactStore::new(tempdir.path().to_path_buf(), pool);

    // No saves — the artifacts table is empty.
    let results = store
        .list(None)
        .await
        .expect("list on empty table should not error");
    assert!(
        results.is_empty(),
        "list(None) on empty table should return an empty Vec, got {} rows",
        results.len()
    );
}
