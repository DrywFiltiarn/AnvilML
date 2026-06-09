//! Integration test for `ArtifactStore::save`.
//!
//! Verifies the full pipeline: base64-decode -> SHA-256 hash -> file write
//! -> DB insert -> job artifact_count increment.

use std::fs;
use std::path::PathBuf;

use anvilml_registry::open_in_memory;
use tempfile::TempDir;

use anvilml_server::artifact::{ArtifactStore, ArtifactStoreInput};
use base64::Engine as _;
use sha2::Digest as _;

/// A minimal valid 1×1 transparent PNG, base64-encoded.
///
/// This is a 42-byte PNG (8-byte signature + IHDR + IDAT + IEND) that
/// round-trips through the `image` crate without error.
const MINIMAL_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Build a fresh test environment with a temporary artifact directory
/// and an in-memory SQLite database with all migrations applied.
///
/// Also inserts a placeholder job row so the `artifact_count` UPDATE
/// affects exactly one row.
async fn setup_test_env() -> (TempDir, PathBuf, ArtifactStore, String) {
    let tmp = TempDir::new().expect("create temp dir");
    let artifact_dir = tmp.path().join("artifacts");
    fs::create_dir_all(&artifact_dir).expect("create artifact dir");

    let pool = open_in_memory().await.expect("open in-memory db");

    // Insert a placeholder job row.
    let job_id = "test-job-001".to_string();
    sqlx::query(
        "INSERT INTO jobs (id, status, graph, settings, artifact_count, created_at) \
         VALUES (?, 'Queued', '{}', '{}', 0, strftime('%s','now'))",
    )
    .bind(&job_id)
    .execute(&pool)
    .await
    .expect("insert test job");

    let store = ArtifactStore::new(artifact_dir.clone(), pool);

    (tmp, artifact_dir, store, job_id)
}

/// Full save pipeline: decode → hash → write → DB insert → count increment.
#[tokio::test]
async fn artifact_save() {
    let (_tmp, _artifact_dir, store, job_id) = setup_test_env().await;

    let meta_input = ArtifactStoreInput {
        width: 512,
        height: 512,
        seed: 42,
        steps: 20,
        prompt: "a cat".to_string(),
    };

    let meta = store
        .save(&job_id, MINIMAL_PNG_B64, meta_input.clone())
        .await
        .expect("save should succeed");

    // Compute expected hash (SHA-256 of the decoded PNG bytes).
    let bytes = base64::prelude::BASE64_STANDARD
        .decode(MINIMAL_PNG_B64)
        .expect("decode b64");
    let expected_hash = hex::encode(sha2::Sha256::digest(&bytes));

    // Assert: file exists at {hash[0..2]}/{hash}.png
    let prefix_dir = _artifact_dir.join(&expected_hash[..2]);
    let file_path = prefix_dir.join(format!("{expected_hash}.png"));
    assert!(
        file_path.exists(),
        "artifact file must exist at {file_path:?}"
    );

    // Assert: file content matches decoded bytes.
    let written = fs::read(&file_path).expect("read artifact file");
    assert_eq!(written, bytes, "file content must match decoded PNG");

    // Assert: metadata hash is correct.
    assert_eq!(meta.hash, expected_hash, "hash must match SHA-256 of file");
    assert_eq!(meta.job_id, job_id, "job_id must match");
    assert_eq!(meta.width, 512);
    assert_eq!(meta.height, 512);
    assert_eq!(meta.seed, 42);
    assert_eq!(meta.steps, 20);
    assert_eq!(meta.prompt, "a cat");

    // Assert: artifacts table has exactly one row.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifacts")
        .fetch_one(store.db())
        .await
        .expect("query artifacts count");
    assert_eq!(count, 1, "artifacts table must have one row");

    // Assert: the inserted artifact row has the correct hash.
    let row_hash: String = sqlx::query_scalar("SELECT hash FROM artifacts LIMIT 1")
        .fetch_one(store.db())
        .await
        .expect("query artifact hash");
    assert_eq!(row_hash, expected_hash);

    // Assert: jobs.artifact_count was incremented from 0 to 1.
    let artifact_count: i64 = sqlx::query_scalar("SELECT artifact_count FROM jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_one(store.db())
        .await
        .expect("query job artifact_count");
    assert_eq!(artifact_count, 1, "job artifact_count must be 1");
}
