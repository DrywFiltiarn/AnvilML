//! Database connection, PRAGMA configuration, and migration runner.
//!
//! This module provides a single entry point — [`open`] — that creates a
//! `SqlitePool` with the recommended SQLite pragmas (WAL mode, NORMAL
//! synchronous, foreign keys enabled) and runs all embedded migrations.

use std::path::Path;

use anvilml_core::error::AnvilError;
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

/// Convert a `sqlx::Error` into an `AnvilError::DbError`.
fn sqlx_error(err: sqlx::Error) -> AnvilError {
    AnvilError::DbError(err.to_string())
}

/// Convert a migration error into an `AnvilError::DbError`.
fn migrate_error(err: MigrateError) -> AnvilError {
    AnvilError::DbError(err.to_string())
}

/// Embedded migrations loaded at compile time from `../../backend/migrations`.
// The path is relative to CARGO_MANIFEST_DIR of this crate
// (crates/anvilml-registry/), so `../../backend/migrations` resolves to
// the workspace-level backend/migrations/ directory.
static MIGRATIONS: Migrator = sqlx::migrate!("../../backend/migrations");

/// Open a SQLite database at the given path, configure pragmas, run migrations,
/// and return a ready-to-use connection pool.
pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError> {
    let pool =
        SqlitePoolOptions::new()
            .max_connections(5)
            .connect(path.to_str().ok_or_else(|| {
                AnvilError::DbError("database path contains invalid UTF-8".into())
            })?)
            .await
            .map_err(sqlx_error)?;

    // Configure SQLite pragmas.
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await
        .map_err(sqlx_error)?;

    sqlx::query("PRAGMA synchronous=NORMAL")
        .execute(&pool)
        .await
        .map_err(sqlx_error)?;

    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .map_err(sqlx_error)?;

    // Run embedded migrations.
    MIGRATIONS.run(&pool).await.map_err(migrate_error)?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening a temporary database must create the three expected tables.
    #[tokio::test]
    async fn test_open_creates_tables() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();

        let pool = open(path).await.unwrap();

        // Verify all three tables exist in sqlite_master.
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name IN ('jobs','models','artifacts')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(count, 3, "expected jobs, models, and artifacts tables");

        // Verify each table individually.
        for table in ["jobs", "models", "artifacts"] {
            let exists: i64 = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(exists, 1, "{table} table should exist");
        }
    }
}
