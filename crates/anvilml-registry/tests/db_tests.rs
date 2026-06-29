//! Integration tests for `create_pool` — pool creation, migration application,
//! WAL mode, and migration idempotency.
//!
//! Each test creates its own temporary file, so no `#[serial]` annotation is needed —
//! there is no cross-test shared state.

use anvilml_registry::create_pool;
use tempfile::NamedTempFile;

/// `create_pool()` succeeds against a temporary file and the pool can execute queries.
///
/// Creates a temp file, calls `create_pool()` to create/open the database and run
/// migrations, then executes a simple `SELECT 1` query to verify the pool is functional.
#[tokio::test]
async fn test_pool_creation_succeeds() {
    let temp_file = create_temp_db();
    let path = temp_file.path();

    let pool = create_pool(path).await.expect("create_pool should succeed");

    // Verify the pool can execute queries — proves the connection is valid.
    let result: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("should be able to execute SELECT 1");
    assert_eq!(result, 1);
}

/// Migrations create both `models` and `device_capabilities` tables.
///
/// Creates a pool against a temp file (which triggers migration execution), then
/// queries `sqlite_master` to verify both tables defined in `001_initial.sql` exist.
#[tokio::test]
async fn test_migrations_create_tables() {
    let temp_file = create_temp_db();
    let path = temp_file.path();

    let pool = create_pool(path).await.expect("create_pool should succeed");

    // Query sqlite_master for table names — only look at tables (not indexes).
    let tables: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("sqlite_master query should succeed");

    assert!(
        tables.contains(&"device_capabilities".to_string()),
        "expected 'device_capabilities' table to exist; found: {tables:?}"
    );
    assert!(
        tables.contains(&"models".to_string()),
        "expected 'models' table to exist; found: {tables:?}"
    );
}

/// WAL journal mode is active after pool creation.
///
/// After `create_pool()` enables WAL mode via `PRAGMA journal_mode=WAL`, this test
/// queries `PRAGMA journal_mode` to confirm the result is `"wal"`.
#[tokio::test]
async fn test_wal_mode_enabled() {
    let temp_file = create_temp_db();
    let path = temp_file.path();

    let pool = create_pool(path).await.expect("create_pool should succeed");

    // Query the current journal mode — should be "wal".
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA query should succeed");
    assert_eq!(
        mode.to_lowercase(),
        "wal",
        "expected WAL journal mode, got: {mode}"
    );
}

/// Running migrations twice against the same database is idempotent.
///
/// Creates a first pool (which runs migrations), then creates a second pool against
/// the same file (which runs migrations again). Both should succeed without error,
/// proving migrations are idempotent.
#[tokio::test]
async fn test_migrations_idempotent() {
    let temp_file = create_temp_db();
    let path = temp_file.path();

    // First pool creation runs migrations.
    let pool1 = create_pool(path)
        .await
        .expect("first create_pool should succeed");

    // Second pool creation runs migrations again — should be a no-op.
    let pool2 = create_pool(path)
        .await
        .expect("second create_pool should succeed");

    // Both pools should be functional.
    let _: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool1)
        .await
        .expect("pool1 should execute queries");
    let _: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool2)
        .await
        .expect("pool2 should execute queries");
}

/// Creates a temporary file for database storage.
///
/// The file is created but not written to; `create_pool()` will create the SQLite
/// database file at this path. The `NamedTempFile` guard ensures the file is
/// automatically deleted when the test completes.
fn create_temp_db() -> NamedTempFile {
    NamedTempFile::new().expect("should be able to create temp file")
}
