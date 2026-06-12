//! ModelRegistry — SQLite-backed store for model metadata.

use std::collections::HashSet;
use std::path::PathBuf;

use anvilml_core::config::ModelDirConfig;
use anvilml_core::{DType, ModelKind, ModelMeta};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use anvilml_core::error::AnvilError;
use anvilml_core::ModelMetaPatch;

/// Convert a `sqlx::Error` into an `AnvilError::DbError`.
fn sqlx_error(err: sqlx::Error) -> AnvilError {
    AnvilError::DbError(err.to_string())
}

/// Normalise a path string for consistent DB storage and comparison.
///
/// Two transformations are applied in order:
///
/// 1. Strip the Windows UNC extended-length prefix `\\?\` that
///    [`std::fs::canonicalize`] produces on Windows. This prefix is an
///    implementation detail of the Win32 API and must not appear in stored paths.
/// 2. Replace all backslashes with forward slashes so that paths written on
///    Windows and paths written on Linux compare equal when the same logical
///    file is referenced.
///
/// After this function every path in the `models` table is an absolute
/// forward-slash string with no UNC prefix, regardless of the platform that
/// wrote it.
fn norm_path(p: &str) -> String {
    let stripped = p.strip_prefix(r"\\?\").unwrap_or(p);
    stripped.replace('\\', "/")
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
    ///
    /// The path is normalised via [`norm_path`] before storage: the Windows
    /// `\\?\` UNC prefix is stripped and backslashes are converted to forward
    /// slashes. This ensures every row in the table is platform-neutral and
    /// that the stale-detection pass in [`Self::rescan`] can compare DB paths
    /// against scanner-produced paths using simple string equality.
    pub async fn upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO models
               (id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        )
        .bind(&meta.id)
        .bind(&meta.name)
        .bind(norm_path(&meta.path.to_string_lossy()))
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
    /// model into the registry. Returns `(upserted, removed_stale)`.
    ///
    /// This operation is idempotent: calling it multiple times over the same
    /// files will not create duplicates because `upsert` uses `INSERT OR REPLACE`.
    /// Stale rows (files deleted from disk) are automatically removed.
    ///
    /// # Path consistency guarantee
    ///
    /// [`crate::scanner::scan_dirs`] canonicalises every path to an absolute
    /// form before returning it. [`Self::upsert`] then calls [`norm_path`] to
    /// strip the Windows `\\?\` prefix and convert backslashes to forward
    /// slashes before writing to SQLite. The stale-detection pass here
    /// applies the same [`norm_path`] transformation to every DB row and
    /// compares against the normalised fresh set, so the check is exact and
    /// platform-neutral on both Windows and Linux.
    pub async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<(usize, usize), AnvilError> {
        let metas = crate::scanner::scan_dirs(dirs).await;

        // Build the set of normalised absolute paths produced by this scan.
        // upsert() writes norm_path() strings, so the comparison is exact.
        let fresh_paths: HashSet<String> = metas
            .iter()
            .map(|m| norm_path(&m.path.to_string_lossy()))
            .collect();

        // Upsert all discovered models.
        for meta in &metas {
            self.upsert(meta).await?;
        }

        let upserted = metas.len();

        // Fetch all DB rows. Any row whose normalised path is absent from
        // fresh_paths is stale (the file was deleted or moved).
        let all_rows: Vec<(String, String)> = sqlx::query_as("SELECT id, path FROM models")
            .fetch_all(&self.pool)
            .await
            .map_err(sqlx_error)?;

        let stale_ids: Vec<String> = all_rows
            .into_iter()
            .filter(|(_, path)| !fresh_paths.contains(&norm_path(path)))
            .map(|(id, _)| id)
            .collect();

        for id in &stale_ids {
            tracing::debug!(model_id = %id, "rescan: removed stale model");
            sqlx::query("DELETE FROM models WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(sqlx_error)?;
        }

        let removed = stale_ids.len();
        Ok((upserted, removed))
    }

    /// Partially update a model's metadata in the registry.
    ///
    /// Only the fields set to `Some` in `patch` are applied; absent fields
    /// are left untouched. After applying the patch, VRAM is recomputed from
    /// the (possibly changed) dtype and the record is upserted.
    ///
    /// Returns `Ok(None)` if no model with the given ID exists.
    pub async fn patch_meta(
        &self,
        id: &str,
        patch: ModelMetaPatch,
    ) -> Result<Option<ModelMeta>, AnvilError> {
        let current = self.get(id).await?;

        let mut updated = match current {
            Some(meta) => meta,
            None => return Ok(None),
        };

        if let Some(dtype) = patch.dtype_hint {
            updated.dtype_hint = dtype;
        }

        if let Some(kind) = patch.kind {
            updated.kind = kind;
        }

        updated.vram_estimate_mib =
            crate::scanner::vram_estimate_mib(updated.size_bytes, updated.dtype_hint);

        self.upsert(&updated).await?;

        Ok(Some(updated))
    }
}
