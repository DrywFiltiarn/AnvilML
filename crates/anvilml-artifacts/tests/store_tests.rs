/// Integration tests for `store.rs` — `ArtifactStore` operations.
///
/// These tests verify the complete artifact lifecycle: save with hash-based
/// deduplication, retrieval by hash, listing with optional job filter, and
/// idempotent save behavior.
///
/// Each test creates its own in-memory database via a single-connection
/// `SqlitePool`, ensuring complete database isolation between tests.
use anvilml_artifacts::ArtifactStore;
use sqlx::pool::PoolOptions;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Create an in-memory SQLite pool with a single connection.
///
/// Uses `max_connections(1)` because SQLite's `:memory:` URL creates a
/// private database per connection. With more than one connection in the
/// pool, different pool members would see different databases.
///
/// Also creates the `artifacts` table that the `ArtifactStore` expects.
async fn open_in_memory() -> SqlitePool {
    PoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect to in-memory DB")
}

/// Ensure the `artifacts` table exists in the given pool.
///
/// The `ArtifactStore` assumes the table exists; this helper creates it
/// so tests can exercise store methods without the full migration system.
async fn ensure_artifacts_table(pool: &SqlitePool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS artifacts (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            job_id     TEXT    NOT NULL,
            hash       TEXT    NOT NULL UNIQUE,
            path       TEXT    NOT NULL,
            size_bytes INTEGER NOT NULL,
            created_at TEXT    NOT NULL,
            width      INTEGER,
            height     INTEGER
        )",
    )
    .execute(pool)
    .await
    .expect("create artifacts table");
}

/// Helper to create a default `ArtifactStore` for tests.
///
/// Creates a temp directory for artifact storage and an in-memory database,
/// then constructs the store. The temp directory is returned so the caller
/// can verify file paths on disk.
async fn make_store() -> (ArtifactStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("create temp dir for artifacts");
    let pool = open_in_memory().await;
    ensure_artifacts_table(&pool).await;
    let store = ArtifactStore::new(dir.path().to_path_buf(), pool).await;
    (store, dir)
}

/// Verifies that `save()` writes an artifact file and records metadata,
/// and that `get()` retrieves the correct path.
///
/// Creates a 128-byte PNG-like artifact, saves it, then looks it up by
/// the returned hash. Asserts the path exists on disk and matches the
/// expected `{dir}/{hash}.png` pattern.
#[tokio::test]
async fn test_save_and_get() {
    let (store, _dir) = make_store().await;

    let job_id = Uuid::new_v4();
    let image_bytes: Vec<u8> = (0..128).map(|i| (i % 256) as u8).collect();

    let meta = store
        .save(job_id, &image_bytes)
        .await
        .expect("save should succeed");

    // Verify the hash is a 64-character lowercase hex string.
    assert_eq!(
        meta.hash.len(),
        64,
        "SHA-256 hash must be 64 hex characters, got {}",
        meta.hash.len()
    );
    assert!(
        meta.hash.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be lowercase hex"
    );

    // Verify get() returns the correct path.
    let retrieved_path = store
        .get(&meta.hash)
        .await
        .expect("get should succeed")
        .expect("artifact should exist in database");

    // The path should be {dir}/{hash}.png.
    let expected_filename = format!("{}.png", meta.hash);
    assert!(
        retrieved_path.ends_with(&expected_filename),
        "path {} should end with {}",
        retrieved_path.display(),
        expected_filename
    );
}

/// Verifies that saving identical bytes twice writes the file only once
/// and produces a single database row.
///
/// Saves the same 128-byte image twice with different job IDs. Asserts
/// that only one file exists on disk and that `list(None)` returns exactly
/// one artifact (due to `INSERT OR IGNORE` on the hash UNIQUE constraint).
#[tokio::test]
async fn test_save_idempotency() {
    let (store, _dir) = make_store().await;

    let job_id_a = Uuid::new_v4();
    let job_id_b = Uuid::new_v4();
    let image_bytes: Vec<u8> = vec![0x89; 256];

    // Save the same bytes under two different job IDs.
    let meta_a = store
        .save(job_id_a, &image_bytes)
        .await
        .expect("first save should succeed");
    let meta_b = store
        .save(job_id_b, &image_bytes)
        .await
        .expect("second save should succeed");

    // Both saves must produce the same hash (content-addressed).
    assert_eq!(
        meta_a.hash, meta_b.hash,
        "identical bytes must produce the same hash"
    );

    // list(None) should return exactly one artifact (INSERT OR IGNORE).
    let all = store.list(None).await.expect("list should succeed");
    assert_eq!(
        all.len(),
        1,
        "idempotent save must produce exactly one artifact, found {}",
        all.len()
    );
}

/// Verifies that `list(None)` returns all artifacts when no filter is applied.
///
/// Saves three artifacts for three different jobs, then calls `list(None)`
/// and asserts that all three are returned.
#[tokio::test]
async fn test_list_all() {
    let (store, _dir) = make_store().await;

    let job_a = Uuid::new_v4();
    let job_b = Uuid::new_v4();
    let job_c = Uuid::new_v4();

    store.save(job_a, &[1; 64]).await.expect("save artifact 1");
    store.save(job_b, &[2; 64]).await.expect("save artifact 2");
    store.save(job_c, &[3; 64]).await.expect("save artifact 3");

    let all = store.list(None).await.expect("list should succeed");

    assert_eq!(
        all.len(),
        3,
        "list without filter must return all 3 artifacts, found {}",
        all.len()
    );
}

/// Verifies that `list(Some(job_id))` returns only artifacts for the
/// specified job.
///
/// Saves three artifacts: two for job A and one for job B. Asserts that
/// filtering by job A returns exactly two artifacts and filtering by
/// job B returns exactly one.
#[tokio::test]
async fn test_list_filtered() {
    let (store, _dir) = make_store().await;

    let job_a = Uuid::new_v4();
    let job_b = Uuid::new_v4();

    // Save two artifacts for job_a.
    store
        .save(job_a, &[1; 64])
        .await
        .expect("save artifact 1 for job_a");
    store
        .save(job_a, &[2; 64])
        .await
        .expect("save artifact 2 for job_a");

    // Save one artifact for job_b.
    store
        .save(job_b, &[3; 64])
        .await
        .expect("save artifact 1 for job_b");

    // Filter by job_a — should return exactly 2.
    let job_a_artifacts = store
        .list(Some(job_a))
        .await
        .expect("list for job_a should succeed");
    assert_eq!(
        job_a_artifacts.len(),
        2,
        "list filtered by job_a must return 2 artifacts, found {}",
        job_a_artifacts.len()
    );

    // Filter by job_b — should return exactly 1.
    let job_b_artifacts = store
        .list(Some(job_b))
        .await
        .expect("list for job_b should succeed");
    assert_eq!(
        job_b_artifacts.len(),
        1,
        "list filtered by job_b must return 1 artifact, found {}",
        job_b_artifacts.len()
    );

    // Verify each artifact in the filtered list belongs to the correct job.
    for meta in &job_a_artifacts {
        assert_eq!(meta.job_id, job_a, "filtered artifact must belong to job_a");
    }
    for meta in &job_b_artifacts {
        assert_eq!(meta.job_id, job_b, "filtered artifact must belong to job_b");
    }
}

/// Verifies that `get()` returns `None` for a nonexistent hash.
///
/// Creates a fresh store with no artifacts, then calls `get()` with an
/// arbitrary hash string. Asserts that the result is `None`, not an error.
#[tokio::test]
async fn test_get_missing_hash() {
    let (store, _dir) = make_store().await;

    let nonexistent_hash = "0000000000000000000000000000000000000000000000000000000000000000";

    let result = store
        .get(nonexistent_hash)
        .await
        .expect("get should not error for missing hash");

    assert!(
        result.is_none(),
        "get for nonexistent hash must return None"
    );
}
