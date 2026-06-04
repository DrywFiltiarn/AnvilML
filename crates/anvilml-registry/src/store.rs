//! ModelRegistry — SQLite-backed store for model metadata.

use std::path::PathBuf;

use anvilml_core::config::ModelDirConfig;
use anvilml_core::{DType, ModelKind, ModelMeta};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use anvilml_core::error::AnvilError;

/// Convert a `sqlx::Error` into an `AnvilError::DbError`.
fn sqlx_error(err: sqlx::Error) -> AnvilError {
    AnvilError::DbError(err.to_string())
}

/// Tuple representing a single row from the `models` table.
type ModelRow = (
    String, // id
    String, // name
    String, // path
    String, // kind
    i64,    // size_bytes
    String, // dtype_hint
    i64,    // vram_estimate_mib
    String, // scanned_at
);

/// SQLite-backed model registry store.
pub struct ModelRegistry {
    pool: SqlitePool,
}

impl ModelRegistry {
    /// Create a new `ModelRegistry` backed by the given SQLite connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update a model's metadata in the registry.
    ///
    /// Uses `INSERT OR REPLACE` so that calling this with an existing `id`
    /// updates all columns to the provided values.
    pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO models
               (id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(&meta.id)
        .bind(&meta.name)
        .bind(meta.path.to_string_lossy().to_string())
        .bind(serde_json::to_string(&meta.kind).map_err(|e| AnvilError::Json(e.to_string()))?)
        .bind(meta.size_bytes as i64)
        .bind(serde_json::to_string(&meta.dtype_hint).map_err(|e| AnvilError::Json(e.to_string()))?)
        .bind(meta.vram_estimate_mib as i64)
        .bind(meta.scanned_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;

        Ok(())
    }

    /// Look up a single model by its ID.
    ///
    /// Returns `Ok(None)` if no row with the given ID exists, or
    /// `Ok(Some(meta))` with all eight columns deserialized back into
    /// a [`ModelMeta`] struct.
    pub async fn get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError> {
        let row: Option<ModelRow> = sqlx::query_as(
            "SELECT id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at \
             FROM models WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?;

        match row {
            Some((id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at)) => {
                let kind: ModelKind =
                    serde_json::from_str(&kind).map_err(|e| AnvilError::Json(e.to_string()))?;
                let dtype_hint: DType = serde_json::from_str(&dtype_hint)
                    .map_err(|e| AnvilError::Json(e.to_string()))?;
                let scanned_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&scanned_at)
                    .map_err(|e| AnvilError::Json(e.to_string()))?
                    .with_timezone(&Utc);

                Ok(Some(ModelMeta {
                    id,
                    name,
                    path: PathBuf::from(path),
                    kind,
                    size_bytes: size_bytes as u64,
                    dtype_hint,
                    vram_estimate_mib: vram_estimate_mib as u32,
                    scanned_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// List all scanned model metadata, optionally filtered by kind.
    ///
    /// Returns rows ordered by `name ASC`. When `kind` is `Some`, only
    /// models whose kind matches the given value are returned.
    pub async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError> {
        let rows: Vec<ModelRow> = match kind {
            Some(k) => {
                let kind_json = serde_json::to_string(&k).map_err(|e| AnvilError::Json(e.to_string()))?;
                sqlx::query_as(
                    "SELECT id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at \
                     FROM models WHERE kind = ?",
                )
                .bind(&kind_json)
                .fetch_all(&self.pool)
                .await
                .map_err(sqlx_error)?
            }
            None => {
                sqlx::query_as(
                    "SELECT id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at \
                     FROM models ORDER BY name ASC",
                )
                .fetch_all(&self.pool)
                .await
                .map_err(sqlx_error)?
            }
        };

        let mut results = Vec::with_capacity(rows.len());
        for (id, name, path, kind_str, size_bytes, dtype_hint, vram_estimate_mib, scanned_at) in
            rows
        {
            let kind: ModelKind =
                serde_json::from_str(&kind_str).map_err(|e| AnvilError::Json(e.to_string()))?;
            let dtype_hint: DType =
                serde_json::from_str(&dtype_hint).map_err(|e| AnvilError::Json(e.to_string()))?;
            let scanned_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&scanned_at)
                .map_err(|e| AnvilError::Json(e.to_string()))?
                .with_timezone(&Utc);

            results.push(ModelMeta {
                id,
                name,
                path: PathBuf::from(path),
                kind,
                size_bytes: size_bytes as u64,
                dtype_hint,
                vram_estimate_mib: vram_estimate_mib as u32,
                scanned_at,
            });
        }

        Ok(results)
    }

    /// Scan the configured model directories and upsert every discovered
    /// model into the registry. Returns the total number of models processed.
    ///
    /// This operation is idempotent: calling it multiple times over the same
    /// files will not create duplicates because `upsert` uses `INSERT OR REPLACE`.
    /// Stale rows (files deleted from disk) are NOT removed — manual cleanup
    /// is required via a separate mechanism.
    pub async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<u32, AnvilError> {
        let metas = crate::scanner::scan_dirs(dirs).await;
        for meta in &metas {
            self.upsert(meta).await?;
        }
        Ok(metas.len() as u32)
    }
}
