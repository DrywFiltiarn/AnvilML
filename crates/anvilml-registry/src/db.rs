//! Database pool creation and migration runner.
//!
//! Provides `create_pool()` — the single entry point for obtaining a `SqlitePool`
//! that has WAL mode enabled and all migrations from `database/migrations/` applied.

use std::path::Path;

use sqlx::SqlitePool;
use sqlx::migrate::MigrateError;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tracing::debug;

use anvilml_core::AnvilError;

/// Create a `SqlitePool` connected to the database at *db_path*, enabling WAL mode
/// and running all pending migrations.
///
/// This is the single entry point for database access in the registry crate. Every
/// caller should use this function rather than constructing a `SqlitePool` directly,
/// because it guarantees:
/// - The parent directory of `db_path` exists (created if necessary).
/// - WAL journaling mode is enabled for better concurrency.
/// - All SQL migration files in `database/migrations/` are applied (idempotent).
///
/// # Arguments
///
/// * `db_path` — Filesystem path to the SQLite database file. The parent directory
///   will be created if it does not exist.
///
/// # Errors
///
/// Returns `AnvilError::Io` if the directory cannot be created, or
/// `AnvilError::Db` if the pool connection, WAL setup, or migration fails.
pub async fn create_pool(db_path: &Path) -> Result<SqlitePool, AnvilError> {
    // Ensure the parent directory exists so that `create_pool("./data/anvilml.db")`
    // works even when the `data/` directory has not been created yet.
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Build the pool with default settings — sqlx's default max connections (4 for
    // SQLite) is appropriate for a single-process application with SQLite storage.
    // Use SqliteConnectOptions to build the connection options from the Path,
    // then connect with the pool.
    let connect_opts = SqliteConnectOptions::new().filename(db_path);
    let pool = SqlitePoolOptions::new().connect_with(connect_opts).await?;

    // Enable WAL mode for better concurrent read/write performance. SQLite's default
    // journal mode (DELETE) serialises all access; WAL allows readers and writers to
    // proceed concurrently. The PRAGMA returns the active mode in the result row.
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    debug!(db_path = %db_path.display(), "WAL journal mode enabled");

    // Run all migration files from `database/migrations/` in filename-sorted order.
    // The `sqlx::migrate!()` macro embeds the migration file list at compile time
    // (the path must be a string literal), and `.run()` executes them in order.
    // This is idempotent — running against an already-migrated database is a no-op.
    // MigrateError converts to sqlx::Error::Migrate via From impl, then to AnvilError::Db.
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&pool)
        .await
        .map_err(|e: MigrateError| AnvilError::Db(sqlx::Error::Migrate(Box::new(e))))?;

    Ok(pool)
}

// No inline tests — pool creation is tested via integration tests in
// `crates/anvilml-registry/tests/db_tests.rs` which exercise the full
// migration and WAL setup path.
