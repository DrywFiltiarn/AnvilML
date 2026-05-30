//! Database connection and migration management.

use std::path::Path;

use sqlx::SqlitePool;

use anvilml_core::AnvilError;

/// Opens a SQLite database at `path`, sets WAL-mode pragmas, and applies all
/// migrations from `../../backend/migrations`.
///
/// Returns a ready-to-use `SqlitePool`.
pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError> {
    let pool = SqlitePool::connect(&format!("sqlite:{}", path.display()))
        .await
        .map_err(|e| AnvilError::DbError(format!("failed to connect database: {e}")))?;

    // Set WAL mode pragmas.
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await
        .map_err(|e| AnvilError::DbError(format!("failed to set journal_mode: {e}")))?;
    sqlx::query("PRAGMA synchronous=NORMAL")
        .execute(&pool)
        .await
        .map_err(|e| AnvilError::DbError(format!("failed to set synchronous: {e}")))?;
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .map_err(|e| AnvilError::DbError(format!("failed to enable foreign_keys: {e}")))?;

    // Apply all migrations.
    sqlx::migrate!("../../backend/migrations")
        .run(&pool)
        .await
        .map_err(|e| AnvilError::DbError(format!("migration failed: {e}")))?;

    Ok(pool)
}
