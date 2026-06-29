//! ModelStore — SQLite-backed CRUD for ModelMeta rows.
//!
//! Provides `ModelStore`, the single persistence layer for model metadata.
//! All methods operate on the `models` table created by migration `001_initial.sql`.
//! `mtime_unix` is always inserted as `0` (a placeholder); the scanner populates
//! the real value in P6-A4.

use anvilml_core::{AnvilError, ModelDtype, ModelFormat, ModelKind, ModelMeta};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::path::PathBuf;

/// SQLite-backed persistence for `ModelMeta` records.
///
/// Wraps a `SqlitePool` and provides CRUD operations on the `models` table:
/// inserting or replacing metadata (`upsert`), fetching a single row by ID
/// (`get`), listing all rows optionally filtered by kind (`list`), and
/// deleting a row by ID (`delete`).
///
/// The `models` table schema is defined in `database/migrations/001_initial.sql`.
pub struct ModelStore {
    /// Database connection pool. All methods acquire a connection from this pool.
    pool: SqlitePool,
}

/// Helper struct for reading `models` table rows as plain values.
///
/// `ModelMeta` cannot be used directly with `sqlx::query_as!` because it contains
/// `PathBuf` and `DateTime<Utc>`, which sqlx does not natively map from SQLite
/// column types. This struct captures the raw column values as strings and integers,
/// then `ModelStore` converts them to `ModelMeta` fields manually.
///
/// The `FromRow` derive is required by sqlx's `query_as` to map SQL columns
/// to struct fields by name.
#[derive(sqlx::FromRow)]
struct ModelMetaRow {
    id: String,
    name: String,
    path: String,
    kind: String,
    dtype: String,
    format: String,
    size_bytes: i64,
    #[allow(dead_code)]
    // mtime_unix is populated by the scanner (P6-A4); not a field on ModelMeta.
    mtime_unix: i64,
    scanned_at: String,
}

impl ModelStore {
    /// Construct a new `ModelStore` backed by the given connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already had migrations applied.
    ///   The pool must be connected to a database containing the `models` table.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or replace a `ModelMeta` row in the `models` table.
    ///
    /// Uses `INSERT OR REPLACE` to handle both insert and update in a single
    /// statement, keyed by the `id` primary key. If the row already exists,
    /// it is replaced entirely.
    ///
    /// `mtime_unix` is always inserted as `0` — a placeholder that the scanner
    /// (P6-A4) populates with the real file modification time.
    ///
    /// # Arguments
    ///
    /// * `meta` — The model metadata to persist.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the SQL statement fails (e.g. connection error,
    /// constraint violation).
    /// Returns `AnvilError::Serde` if enum serialization fails (should never happen
    /// with known enum variants).
    #[tracing::instrument(fields(id = %meta.id), skip(self))]
    pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError> {
        // Serialize enum fields to snake_case text via serde_json — matches the
        // #[serde(rename_all = "snake_case")] attribute on the enum derives.
        let kind_text =
            serde_json::to_string(&meta.kind).map_err(|e| AnvilError::Serde(e.to_string()))?;
        let dtype_text =
            serde_json::to_string(&meta.dtype).map_err(|e| AnvilError::Serde(e.to_string()))?;
        let format_text =
            serde_json::to_string(&meta.format).map_err(|e| AnvilError::Serde(e.to_string()))?;

        // Strip the JSON quotes added by serde_json::to_string (e.g. "\"diffusion\""
        // → "diffusion") so the stored value is plain text matching the column type.
        let kind_clean = kind_text.trim_matches('"');
        let dtype_clean = dtype_text.trim_matches('"');
        let format_clean = format_text.trim_matches('"');

        // mtime_unix is always 0 — the scanner populates the real value in P6-A4.
        // This is a placeholder; INSERT OR REPLACE will overwrite it when the
        // scanner calls upsert with the actual modification time.
        let scanned_at = meta.scanned_at.to_rfc3339();

        sqlx::query(
            "INSERT OR REPLACE INTO models \
             (id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&meta.id)
        .bind(&meta.name)
        .bind(meta.path.to_string_lossy().into_owned())
        .bind(kind_clean)
        .bind(dtype_clean)
        .bind(format_clean)
        .bind(meta.size_bytes as i64)
        .bind(0i64) // placeholder — scanner populates real value
        .bind(scanned_at)
        .execute(&self.pool)
        .await?;

        tracing::debug!(id = %meta.id, "upserted model metadata");
        Ok(())
    }

    /// Fetch a single `ModelMeta` row by its `id` primary key.
    ///
    /// Returns `Ok(None)` if no row with the given ID exists.
    ///
    /// # Arguments
    ///
    /// * `id` — The model ID (SHA256 hex of first 1 MiB of the file).
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the query fails (e.g. connection error).
    #[tracing::instrument(fields(id = %id), skip(self))]
    pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError> {
        let row = sqlx::query_as::<_, ModelMetaRow>(
            "SELECT id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at \
             FROM models WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_meta(r))),
            None => Ok(None),
        }
    }

    /// List all `ModelMeta` rows, optionally filtered by model kind.
    ///
    /// When `kind` is `None`, all rows are returned. When `Some(k)`, only rows
    /// whose `kind` column matches `k` are returned.
    ///
    /// Returns an empty vector if no rows match.
    ///
    /// # Arguments
    ///
    /// * `kind` — Optional model kind filter.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the query fails (e.g. connection error).
    /// Returns `AnvilError::Serde` if the kind filter serializes unexpectedly.
    #[tracing::instrument(skip(self))]
    pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError> {
        let rows: Vec<ModelMetaRow> = match kind {
            Some(k) => {
                // Serialize the kind filter to snake_case text to match the stored value.
                let kind_text =
                    serde_json::to_string(&k).map_err(|e| AnvilError::Serde(e.to_string()))?;
                let kind_clean = kind_text.trim_matches('"');
                sqlx::query_as::<_, ModelMetaRow>(
                    "SELECT id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at \
                     FROM models WHERE kind = ?",
                )
                .bind(kind_clean)
                .fetch_all(&self.pool)
                .await?
            }
            None => sqlx::query_as::<_, ModelMetaRow>(
                "SELECT id, name, path, kind, dtype, format, size_bytes, mtime_unix, scanned_at \
                     FROM models",
            )
            .fetch_all(&self.pool)
            .await?,
        };

        Ok(rows.into_iter().map(|r| self.row_to_meta(r)).collect())
    }

    /// Delete a `ModelMeta` row by its `id` primary key.
    ///
    /// Returns `Ok(())` on success. No error if the row did not exist — SQL
    /// DELETE is a no-op for missing rows.
    ///
    /// # Arguments
    ///
    /// * `id` — The model ID to delete.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the SQL statement fails (e.g. connection error).
    #[tracing::instrument(fields(id = %id), skip(self))]
    pub async fn delete(&self, id: &str) -> Result<(), AnvilError> {
        sqlx::query("DELETE FROM models WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        tracing::debug!(id = %id, "deleted model metadata");
        Ok(())
    }
}

impl ModelStore {
    /// Convert a raw `ModelMetaRow` (string/integer fields from SQL) into a
    /// fully-typed `ModelMeta` struct.
    ///
    /// Each text field is parsed back through serde for enum types, `PathBuf`
    /// is constructed from the path string, and the RFC 3339 timestamp is
    /// parsed to `DateTime<Utc>`.
    fn row_to_meta(&self, row: ModelMetaRow) -> ModelMeta {
        // Deserialize enum variants from their snake_case text representation.
        // The #[serde(rename_all = "snake_case")] attribute means serde_json
        // produces lowercase text, and deserialization expects the same format.
        // These conversions cannot fail for values that were produced by
        // serde_json::to_string on valid enum variants, so .expect() is safe.
        let kind = serde_json::from_str::<ModelKind>(&format!("\"{}\"", row.kind))
            .expect("kind should parse — stored value comes from serde_json serialization");
        let dtype = serde_json::from_str::<ModelDtype>(&format!("\"{}\"", row.dtype))
            .expect("dtype should parse — stored value comes from serde_json serialization");
        let format = serde_json::from_str::<ModelFormat>(&format!("\"{}\"", row.format))
            .expect("format should parse — stored value comes from serde_json serialization");

        // Parse the RFC 3339 timestamp back to DateTime<Utc>.
        // The stored value comes from DateTime::to_rfc3339(), so it is always valid.
        let scanned_at = DateTime::parse_from_rfc3339(&row.scanned_at)
            .expect("scanned_at should be valid RFC 3339 — stored value comes from to_rfc3339()")
            .with_timezone(&Utc);

        ModelMeta {
            id: row.id,
            name: row.name,
            path: PathBuf::from(row.path),
            kind,
            dtype,
            format,
            size_bytes: row.size_bytes as u64,
            scanned_at,
        }
    }
}
