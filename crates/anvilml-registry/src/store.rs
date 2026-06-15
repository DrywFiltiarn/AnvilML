//! Model store — CRUD operations for `ModelMeta` in SQLite.
//!
//! This module provides `ModelStore`, the persistent storage layer for model
//! metadata. It owns the SQLite connection pool and implements upsert, get,
//! list, and delete operations on the `models` table.
//!
//! The `models` table schema (from `001_initial.sql`):
//! - `id TEXT PRIMARY KEY` — SHA256 hex digest of model file
//! - `name TEXT NOT NULL` — human-readable model name
//! - `path TEXT NOT NULL` — filesystem path to model
//! - `kind TEXT NOT NULL` — ModelKind enum (snake_case)
//! - `dtype TEXT NOT NULL` — ModelDtype enum (snake_case)
//! - `format TEXT NOT NULL` — ModelFormat enum (snake_case)
//! - `size_bytes INTEGER NOT NULL` — file size in bytes
//! - `scanned_at TEXT NOT NULL` — ISO 8601 UTC timestamp

use sqlx::{Row, SqlitePool};
use tracing::{debug, instrument};

use anvilml_core::{AnvilError, ModelKind, ModelMeta};

/// Persistent storage for model metadata backed by SQLite.
///
/// Wraps a `SqlitePool` and provides CRUD operations on the `models` table.
/// All methods return `Result<T, AnvilError>`, with `sqlx::Error` automatically
/// converted to `AnvilError::Db` via the `#[from]` attribute on the variant.
pub struct ModelStore {
    pool: SqlitePool,
}

impl ModelStore {
    /// Create a new `ModelStore` backed by the given SQLite connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already been configured with WAL mode
    ///   and has the `models` table created (via migrations).
    ///
    /// # Returns
    ///
    /// A new `ModelStore` instance. This constructor performs no I/O.
    pub async fn new(pool: SqlitePool) -> Self {
        ModelStore { pool }
    }

    /// Insert or replace a model record in the database.
    ///
    /// Uses `INSERT OR REPLACE` to ensure idempotent upserts: if a row with the
    /// same `id` (PRIMARY KEY) already exists, it is deleted and the new row
    /// is inserted. This is the correct behavior for the model scanner which
    /// may re-scan the same directory and produce identical metadata.
    ///
    /// # Arguments
    ///
    /// * `meta` — The `ModelMeta` record to persist.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the INSERT fails (e.g. connection lost,
    /// constraint violation, or schema mismatch).
    #[instrument(skip(self, meta), fields(id = %meta.id, kind = ?meta.kind))]
    pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError> {
        debug!(id = %meta.id, kind = ?meta.kind, "upserting model");

        // Use raw query() instead of query_as!() — we don't need the result
        // row, and query_as! requires DATABASE_URL for online mode. The
        // INSERT OR REPLACE ensures idempotency: if the model ID already
        // exists, the old row is deleted and the new one inserted. This
        // matches the scanner's behavior of re-scanning directories that
        // may produce updated metadata for the same model file.
        //
        // Use a transaction to ensure the write is committed before the
        // connection is returned to the pool. Without this, the connection
        // might be returned before the implicit transaction commits.
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT OR REPLACE INTO models \
             (id, name, path, kind, dtype, format, size_bytes, scanned_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&meta.id)
        .bind(&meta.name)
        .bind(&meta.path)
        .bind(meta.kind.to_string())
        .bind(meta.dtype.to_string())
        .bind(meta.format.to_string())
        .bind(meta.size_bytes as i64)
        .bind(meta.scanned_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(())
    }

    /// Retrieve a single model by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` — The model's unique identifier (SHA256 hex digest).
    ///
    /// # Returns
    ///
    /// `Some(ModelMeta)` if a model with the given ID exists, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the query fails (e.g. connection lost).
    pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError> {
        let row = sqlx::query(
            "SELECT id, name, path, kind, dtype, format, size_bytes, scanned_at \
             FROM models WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        // Convert the raw SQL row into a ModelMeta. The enum fields (kind,
        // dtype, format) are stored as TEXT in SQLite and must be read as
        // String then parsed via FromStr. This is necessary because the
        // enums don't implement sqlx::Type<Sqlite>.
        match row {
            Some(row) => {
                let meta = ModelMeta {
                    id: row.get("id"),
                    name: row.get("name"),
                    path: row.get("path"),
                    kind: row
                        .get::<String, _>("kind")
                        .parse()
                        .map_err(AnvilError::Internal)?,
                    dtype: row
                        .get::<String, _>("dtype")
                        .parse()
                        .map_err(AnvilError::Internal)?,
                    format: row
                        .get::<String, _>("format")
                        .parse()
                        .map_err(AnvilError::Internal)?,
                    size_bytes: row.get::<i64, _>("size_bytes") as u64,
                    scanned_at: row.get("scanned_at"),
                };
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    /// List all models, optionally filtered by kind.
    ///
    /// # Arguments
    ///
    /// * `kind` — If `Some`, only models of the given kind are returned.
    ///   If `None`, all models are returned.
    ///
    /// # Returns
    ///
    /// A `Vec<ModelMeta>` containing the matching models. Empty vec if no
    /// models match.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the query fails.
    pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError> {
        let query = if let Some(kind) = kind {
            // Filter by kind — the WHERE clause narrows results to only
            // models matching the specified kind. This is used by the
            // device capability store to find models of a specific type.
            sqlx::query(
                "SELECT id, name, path, kind, dtype, format, size_bytes, scanned_at \
                 FROM models WHERE kind = ?",
            )
            .bind(kind.to_string())
        } else {
            // No filter — return all models.
            sqlx::query(
                "SELECT id, name, path, kind, dtype, format, size_bytes, scanned_at \
                 FROM models",
            )
        };

        let rows = query.fetch_all(&self.pool).await?;

        let mut models = Vec::with_capacity(rows.len());
        for row in rows {
            // Parse enum fields from TEXT strings via FromStr.
            let meta = ModelMeta {
                id: row.get("id"),
                name: row.get("name"),
                path: row.get("path"),
                kind: row
                    .get::<String, _>("kind")
                    .parse()
                    .map_err(AnvilError::Internal)?,
                dtype: row
                    .get::<String, _>("dtype")
                    .parse()
                    .map_err(AnvilError::Internal)?,
                format: row
                    .get::<String, _>("format")
                    .parse()
                    .map_err(AnvilError::Internal)?,
                size_bytes: row.get::<i64, _>("size_bytes") as u64,
                scanned_at: row.get("scanned_at"),
            };
            models.push(meta);
        }

        Ok(models)
    }

    /// Delete a model by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` — The model's unique identifier.
    ///
    /// # Returns
    ///
    /// `true` if a model was deleted, `false` if no model with that ID exists.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the DELETE fails.
    #[instrument(skip(self), fields(id = %id))]
    pub async fn delete(&self, id: &str) -> Result<bool, AnvilError> {
        debug!(id = %id, "deleting model");

        // Use a transaction to ensure the DELETE is committed before the
        // connection is returned to the pool. Without this, the connection
        // might be returned before the implicit transaction commits.
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        // Check rows_affected to determine if a model was actually deleted.
        // SQLite DELETE returns 0 rows affected when the WHERE clause
        // matches no rows (non-existent ID).
        Ok(result.rows_affected() > 0)
    }
}
