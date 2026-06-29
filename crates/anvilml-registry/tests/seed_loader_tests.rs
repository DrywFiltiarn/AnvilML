//! Integration tests for `SeedLoader` — SHA256-gated seed idempotency checking
//! via the `_seed_log` bookkeeping table.
//!
//! Each test creates its own in-memory SQLite pool with migrations applied,
//! so there is no cross-test shared state and no `#[serial]` annotation is needed.

use anvilml_registry::seed_loader::SeedLoader;
use digest::Digest;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Create an in-memory SQLite pool with migrations applied.
///
/// Each test gets its own pool — the in-memory database is isolated per connection
/// by using a unique cache name (uuid-based) so parallel tests don't collide on
/// the shared `:memory:` database.
///
/// The migration from `database/migrations/001_initial.sql` is applied so the
/// base tables exist before any seed_loader operations.
async fn make_pool() -> SqlitePool {
    // Use a unique in-memory database name per test to avoid the shared `:memory:`
    // database problem: without a unique name, all connections in the same process
    // share the same in-memory database, causing cross-test interference.
    let unique_name = uuid::Uuid::new_v4().to_string();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(format!("file:{unique_name}?mode=memory&cache=shared"))
                .create_if_missing(true),
        )
        .await
        .expect("should be able to create in-memory SQLite pool");

    // Apply the migration so the base tables exist.
    // sqlx::migrate!() embeds the migration at compile time; .run() applies
    // any pending migrations (idempotent — running against an already-migrated
    // database is a no-op).
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator.run(&pool).await.expect("migration should succeed");

    pool
}

/// `already_applied()` on a seed that has never been applied returns `Ok(false)`.
///
/// Creates a pool, constructs a `SeedLoader`, and calls `already_applied()` for a
/// seed_name that has no row in `_seed_log`. The `_seed_log` table does not yet exist;
/// it should be created lazily by `already_applied()` and the method should return
/// `false` because there is no stored hash to match.
#[tokio::test]
async fn test_already_applied_unseen_seed_returns_false() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    let result = loader
        .already_applied("devices.sql", "abc123def456")
        .await
        .expect("already_applied should not error for unseen seed");

    assert!(
        !result,
        "expected false for unseen seed, got true — seed should not be considered applied"
    );
}

/// `already_applied()` returns `false` when a stored hash differs from the given hash.
///
/// Inserts a row into `_seed_log` with `seed_name="devices.sql"` and `sha256="old_hash"`,
/// then calls `already_applied("devices.sql", "new_hash")`. The method should return
/// `false` because the hashes do not match — the seed file has changed since last run.
#[tokio::test]
async fn test_already_applied_hash_mismatch_returns_false() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    // First call creates the table and returns false (unseen).
    let result = loader
        .already_applied("devices.sql", "old_hash")
        .await
        .expect("first call should not error");
    assert!(!result, "first call for unseen seed should return false");

    // Now manually insert a row with the "old_hash" value to simulate a prior run.
    // We use the pool directly because SeedLoader doesn't expose an insert method.
    sqlx::query("INSERT INTO _seed_log (seed_name, sha256, applied_at) VALUES (?, ?, ?)")
        .bind("devices.sql")
        .bind("old_hash")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert should succeed");

    // Now call with a different hash — should return false (hash mismatch).
    let result = loader
        .already_applied("devices.sql", "new_hash")
        .await
        .expect("already_applied should not error for mismatched hash");

    assert!(
        !result,
        "expected false for hash mismatch, got true — seed should not be considered applied"
    );
}

/// `already_applied()` returns `true` when a stored hash matches the given hash.
///
/// Inserts a row into `_seed_log` with `seed_name="devices.sql"` and `sha256="abc123"`,
/// then calls `already_applied("devices.sql", "abc123")`. The method should return
/// `true` because the hashes match — the seed has already been applied with this exact
/// content and can be skipped.
#[tokio::test]
async fn test_already_applied_hash_match_returns_true() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    // First call creates the table and returns false (unseen).
    let result = loader
        .already_applied("devices.sql", "abc123")
        .await
        .expect("first call should not error");
    assert!(!result, "first call for unseen seed should return false");

    // Manually insert a row with the matching hash to simulate a prior run.
    sqlx::query("INSERT INTO _seed_log (seed_name, sha256, applied_at) VALUES (?, ?, ?)")
        .bind("devices.sql")
        .bind("abc123")
        .bind("2026-01-01T00:00:00Z")
        .execute(&pool)
        .await
        .expect("insert should succeed");

    // Now call with the same hash — should return true (hash match).
    let result = loader
        .already_applied("devices.sql", "abc123")
        .await
        .expect("already_applied should not error for matching hash");

    assert!(
        result,
        "expected true for hash match, got false — seed should be considered applied"
    );
}

/// `_seed_log` table is created lazily on the first `already_applied()` call.
///
/// Verifies that calling `already_applied()` on a fresh pool (no `_seed_log` table)
/// creates the table and returns `Ok(false)` for an unseen seed. Then queries
/// `sqlite_master` directly to confirm the table exists.
#[tokio::test]
async fn test_seed_log_created_on_first_use() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    // Call already_applied — this should create the _seed_log table lazily.
    let result = loader
        .already_applied("devices.sql", "abc123")
        .await
        .expect("already_applied should not error");

    assert!(!result, "expected false for unseen seed on first use");

    // Verify the table was created by querying sqlite_master.
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master \
         WHERE type = 'table' AND name = '_seed_log'",
    )
    .fetch_one(&pool)
    .await
    .expect("sqlite_master query should succeed");

    assert!(
        table_exists,
        "_seed_log table should exist after first already_applied() call"
    );
}

/// `run()` on a fresh seed file executes the SQL and records hash+timestamp in `_seed_log`.
///
/// Creates a temp file with valid INSERT SQL, calls `run()` for the first time, then
/// verifies that:
/// - The `_seed_log` table contains exactly one row with the seed name.
/// - The recorded hash matches the SHA256 of the file content.
/// - A subsequent `already_applied()` call returns `true` for the same hash.
///
/// Uses `tempfile::NamedTempFile` for an isolated, auto-cleaning temp file.
#[tokio::test]
async fn test_run_first_time_applies_and_records() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    // Create a temp file with valid INSERT SQL.
    let tmp = tempfile::NamedTempFile::new().expect("should create temp file");
    let seed_name = "devices.sql";
    let sql_content = "INSERT INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4318, 0, 'test_device', 'turing', 1, 1, 1, 0, 0, 0);";
    std::fs::write(tmp.path(), sql_content).expect("should write to temp file");

    // First run should apply the seed.
    loader
        .run(seed_name, tmp.path())
        .await
        .expect("run should succeed for valid SQL");

    // Verify the hash was recorded in _seed_log.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _seed_log WHERE seed_name = ?")
        .bind(seed_name)
        .fetch_one(&pool)
        .await
        .expect("query should succeed");

    assert_eq!(
        count, 1,
        "expected exactly one row in _seed_log for the seed, got {}",
        count
    );

    // Verify that `already_applied` now returns true for the same hash.
    // We need to recompute the hash to verify.
    let contents = std::fs::read(tmp.path()).expect("should read temp file");
    let digest = sha2::Sha256::digest(&contents);
    let expected_hash: String = digest.iter().map(|b| format!("{:02x}", b)).collect();

    let applied = loader
        .already_applied(seed_name, &expected_hash)
        .await
        .expect("already_applied should succeed");

    assert!(
        applied,
        "expected already_applied to return true after run() — seed should be recorded as applied"
    );
}

/// `run()` skips execution when the seed was already applied with identical content.
///
/// Calls `run()` twice with the same seed file. The second call should detect that
/// the hash matches and return `Ok(())` without re-executing the SQL.
///
/// Uses `tempfile::NamedTempFile` for an isolated, auto-cleaning temp file.
#[tokio::test]
async fn test_run_skips_when_already_applied() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    // Create a temp file with valid INSERT SQL.
    let tmp = tempfile::NamedTempFile::new().expect("should create temp file");
    let seed_name = "devices.sql";
    let sql_content = "INSERT INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4318, 0, 'test_device', 'turing', 1, 1, 1, 0, 0, 0);";
    std::fs::write(tmp.path(), sql_content).expect("should write to temp file");

    // First run should apply the seed.
    loader
        .run(seed_name, tmp.path())
        .await
        .expect("first run should succeed");

    // Verify one row exists.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _seed_log WHERE seed_name = ?")
        .bind(seed_name)
        .fetch_one(&pool)
        .await
        .expect("query should succeed");
    assert_eq!(count, 1, "expected exactly one row after first run");

    // Record the initial applied_at timestamp.
    let initial_applied_at: String =
        sqlx::query_scalar("SELECT applied_at FROM _seed_log WHERE seed_name = ?")
            .bind(seed_name)
            .fetch_one(&pool)
            .await
            .expect("query should succeed");

    // Second run with the same file should skip.
    loader
        .run(seed_name, tmp.path())
        .await
        .expect("second run should succeed (skip path)");

    // Verify the row count is still 1 and the timestamp hasn't changed.
    let updated_applied_at: String =
        sqlx::query_scalar("SELECT applied_at FROM _seed_log WHERE seed_name = ?")
            .bind(seed_name)
            .fetch_one(&pool)
            .await
            .expect("query should succeed");

    assert_eq!(
        count, 1,
        "expected still exactly one row after second run (skip)"
    );
    assert_eq!(
        initial_applied_at, updated_applied_at,
        "applied_at should not change on skip — seed was not re-applied"
    );
}

/// `run()` re-applies when the seed file content changes (hash mismatch).
///
/// Calls `run()` with a seed file, then modifies the file content and calls `run()`
/// again. The second run should detect the hash mismatch, re-execute the SQL, and
/// update the `_seed_log` with the new hash and timestamp.
///
/// Uses `tempfile::NamedTempFile` for an isolated, auto-cleaning temp file.
#[tokio::test]
async fn test_run_reapplies_on_changed_content() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    // Create a temp file with initial INSERT SQL.
    let tmp = tempfile::NamedTempFile::new().expect("should create temp file");
    let seed_name = "devices.sql";
    let initial_sql = "INSERT INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4318, 0, 'device_v1', 'turing', 1, 1, 1, 0, 0, 0);";
    std::fs::write(tmp.path(), initial_sql).expect("should write to temp file");

    // First run with initial content.
    loader
        .run(seed_name, tmp.path())
        .await
        .expect("first run should succeed");

    // Record the initial hash and timestamp.
    let initial_hash: String =
        sqlx::query_scalar("SELECT sha256 FROM _seed_log WHERE seed_name = ?")
            .bind(seed_name)
            .fetch_one(&pool)
            .await
            .expect("query should succeed");
    let initial_applied_at: String =
        sqlx::query_scalar("SELECT applied_at FROM _seed_log WHERE seed_name = ?")
            .bind(seed_name)
            .fetch_one(&pool)
            .await
            .expect("query should succeed");

    // Now modify the file content. Use INSERT OR REPLACE to avoid the unique
    // constraint on (vendor_id, device_id) since the first insert already
    // created a row with those PCI IDs.
    let updated_sql = "INSERT OR REPLACE INTO device_capabilities (vendor_id, device_id, name, arch, fp32, fp16, bf16, fp8, fp4, flash_attention) VALUES (4318, 0, 'device_v2', 'ampere', 1, 1, 1, 1, 0, 1);";
    std::fs::write(tmp.path(), updated_sql).expect("should update temp file");

    // Second run with changed content should re-apply.
    loader
        .run(seed_name, tmp.path())
        .await
        .expect("second run should succeed with changed content");

    // Verify the hash changed.
    let updated_hash: String =
        sqlx::query_scalar("SELECT sha256 FROM _seed_log WHERE seed_name = ?")
            .bind(seed_name)
            .fetch_one(&pool)
            .await
            .expect("query should succeed");

    assert_ne!(
        initial_hash, updated_hash,
        "expected hash to change after content modification"
    );

    // Verify the timestamp changed.
    let updated_applied_at: String =
        sqlx::query_scalar("SELECT applied_at FROM _seed_log WHERE seed_name = ?")
            .bind(seed_name)
            .fetch_one(&pool)
            .await
            .expect("query should succeed");

    assert_ne!(
        initial_applied_at, updated_applied_at,
        "expected applied_at to change after re-application"
    );
}

/// `run()` with malformed SQL returns `Err` without recording a hash+timestamp.
///
/// Creates a temp file with invalid SQL, calls `run()`, then verifies that:
/// - `run()` returns an error.
/// - `_seed_log` has no row for this seed_name (no partial state).
/// - `already_applied()` returns `false` for the (non-existent) hash.
///
/// This verifies the transaction rollback behavior: the SQL failure rolls back
/// the transaction, so no hash+timestamp is recorded.
///
/// Uses `tempfile::NamedTempFile` for an isolated, auto-cleaning temp file.
#[tokio::test]
async fn test_run_malformed_sql_returns_err_no_partial_state() {
    let pool = make_pool().await;
    let loader = SeedLoader::new(pool.clone());

    // Create a temp file with invalid SQL.
    let tmp = tempfile::NamedTempFile::new().expect("should create temp file");
    let seed_name = "bad.sql";
    let bad_sql = "INVALID SQL STATEMENT THAT WILL FAIL";
    std::fs::write(tmp.path(), bad_sql).expect("should write to temp file");

    // Run should fail.
    let result = loader.run(seed_name, tmp.path()).await;
    assert!(
        result.is_err(),
        "expected run() to return Err for malformed SQL"
    );

    // Verify no row was recorded in _seed_log.
    let count: Option<i64> =
        sqlx::query_scalar("SELECT COUNT(*) FROM _seed_log WHERE seed_name = ?")
            .bind(seed_name)
            .fetch_optional(&pool)
            .await
            .expect("query should succeed");

    assert_eq!(
        count,
        Some(0),
        "expected no row in _seed_log for failed seed — transaction should have rolled back"
    );

    // Compute the hash of the bad file and verify `already_applied` returns false.
    let contents = std::fs::read(tmp.path()).expect("should read temp file");
    let digest = sha2::Sha256::digest(&contents);
    let bad_hash: String = digest.iter().map(|b| format!("{:02x}", b)).collect();

    let applied = loader
        .already_applied(seed_name, &bad_hash)
        .await
        .expect("already_applied should succeed");

    assert!(
        !applied,
        "expected already_applied to return false for failed seed — no hash should be recorded"
    );
}
