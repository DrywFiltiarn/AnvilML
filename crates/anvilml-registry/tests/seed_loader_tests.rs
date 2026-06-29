//! Integration tests for `SeedLoader` — SHA256-gated seed idempotency checking
//! via the `_seed_log` bookkeeping table.
//!
//! Each test creates its own in-memory SQLite pool with migrations applied,
//! so there is no cross-test shared state and no `#[serial]` annotation is needed.

use anvilml_registry::seed_loader::SeedLoader;
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
    let loader = SeedLoader::new(pool);

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
