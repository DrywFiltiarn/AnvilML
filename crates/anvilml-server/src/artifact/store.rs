//! Content-addressed PNG artifact persistence.
//!
//! Implements `ArtifactStore` which decodes a base64-encoded PNG image,
//! computes its SHA-256 hash, writes it to a two-char-prefix-sharded
//! directory under `artifact_dir`, inserts the artifact metadata row
//! into SQLite, and increments the job's `artifact_count`.

use std::path::PathBuf;

use anvilml_core::types::artifact::{ArtifactSave, ArtifactSaveInput};
use base64::prelude::BASE64_STANDARD;
use base64::Engine as _;
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use sqlx::Row;
use sqlx::SqlitePool;
use thiserror::Error;
use utoipa::ToSchema;

/// Metadata about an artifact stored in the registry.
///
/// Matches the design spec (§4.2, §13) — a local type that mirrors the
/// `artifacts` table schema from migration `003_artifacts.sql`.
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct ArtifactMeta {
    /// SHA-256 hex digest of the artifact file bytes.
    pub hash: String,
    /// The job that produced this artifact.
    pub job_id: String,
    /// Image width in pixels.
    pub width: i64,
    /// Image height in pixels.
    pub height: i64,
    /// File format (e.g. "png").
    pub format: String,
    /// Generation seed.
    pub seed: i64,
    /// Number of diffusion steps.
    pub steps: i64,
    /// Generation prompt text.
    pub prompt: String,
    /// Unix timestamp (seconds) when the artifact was created.
    pub created_at: i64,
}

/// Input metadata carried by the caller into [`ArtifactStore::save`].
#[derive(Debug, Clone)]
pub struct ArtifactStoreInput {
    /// Image width in pixels.
    pub width: i64,
    /// Image height in pixels.
    pub height: i64,
    /// Generation seed.
    pub seed: i64,
    /// Number of diffusion steps.
    pub steps: i64,
    /// Generation prompt text.
    pub prompt: String,
}

/// Errors returned by [`ArtifactStore`].
#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("base64 decode failed: {0}")]
    Decode(#[from] base64::DecodeError),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
}

/// Content-addressed artifact store.
///
/// Stores decoded PNG images under `{artifact_dir}/{hash[0..2]}/{hash}.png`
/// and records metadata in the SQLite registry.
#[derive(Clone)]
pub struct ArtifactStore {
    artifact_dir: PathBuf,
    db: SqlitePool,
}

impl ArtifactStore {
    /// Create a new `ArtifactStore` backed by the given directory and database.
    pub fn new(artifact_dir: PathBuf, db: SqlitePool) -> Self {
        Self { artifact_dir, db }
    }

    /// Returns a reference to the underlying database pool.
    pub fn db(&self) -> &SqlitePool {
        &self.db
    }

    /// Decode, hash, persist, and record a single artifact.
    ///
    /// 1. Base64-decode `image_b64` → raw PNG bytes.
    /// 2. Compute SHA-256 → hex hash string.
    /// 3. Create directory `{artifact_dir}/{hash[0..2]}`.
    /// 4. Write `{artifact_dir}/{hash[0..2]}/{hash}.png`.
    /// 5. INSERT artifact metadata row into `artifacts` table.
    /// 6. UPDATE `jobs.artifact_count = artifact_count + 1` for `job_id`.
    ///
    /// Returns the constructed [`ArtifactMeta`].
    #[tracing::instrument(skip(self, image_b64, meta_input), fields(job_id = %job_id))]
    pub async fn save(
        &self,
        job_id: &str,
        image_b64: &str,
        meta_input: ArtifactStoreInput,
    ) -> Result<ArtifactMeta, ArtifactError> {
        // 1. Decode base64 → bytes.
        let bytes = BASE64_STANDARD.decode(image_b64)?;

        // 2. Compute SHA-256 → hex hash.
        let hash = hex::encode(Sha256::digest(&bytes));
        tracing::debug!(hash = %hash, "computed sha256 hash");

        // 3. Create prefix directory.
        let prefix_dir = self.artifact_dir.join(&hash[..2]);
        tokio::fs::create_dir_all(&prefix_dir).await?;

        // 4. Write file.
        let file_path = prefix_dir.join(format!("{hash}.png"));
        tracing::debug!(path = %file_path.display(), "writing artifact file");
        tokio::fs::write(&file_path, &bytes).await?;

        // 5. Insert artifact row.
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO artifacts (hash, job_id, width, height, format, seed, steps, prompt, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&hash)
        .bind(job_id)
        .bind(meta_input.width)
        .bind(meta_input.height)
        .bind("png")
        .bind(meta_input.seed)
        .bind(meta_input.steps)
        .bind(&meta_input.prompt)
        .bind(now)
        .execute(&self.db)
        .await?;

        // 6. Increment job artifact_count.
        sqlx::query("UPDATE jobs SET artifact_count = artifact_count + 1 WHERE id = ?")
            .bind(job_id)
            .execute(&self.db)
            .await?;

        Ok(ArtifactMeta {
            hash,
            job_id: job_id.to_string(),
            width: meta_input.width,
            height: meta_input.height,
            format: "png".to_string(),
            seed: meta_input.seed,
            steps: meta_input.steps,
            prompt: meta_input.prompt,
            created_at: now,
        })
    }

    /// Return the on-disk path where the artifact with the given hash is stored.
    ///
    /// Constructs `{artifact_dir}/{hash[0..2]}/{hash}.png` using the same
    /// two-char prefix sharding scheme as [`save()`].
    ///
    /// This method does **not** check whether the file exists — callers should
    /// verify existence (e.g. via `fs::metadata`) before serving.
    pub async fn get_path(&self, hash: &str) -> Result<PathBuf, ArtifactError> {
        let prefix_dir = self.artifact_dir.join(&hash[..2]);
        let file_path = prefix_dir.join(format!("{hash}.png"));
        tracing::debug!(hash = %hash, path = %file_path.display(), "resolved artifact path");
        Ok(file_path)
    }

    /// List artifacts, optionally filtered by `job_id` with pagination.
    ///
    /// Queries the `artifacts` table and returns matching rows sorted newest-first.
    ///
    /// # Arguments
    /// * `job_id` — Optional job UUID to filter by. `None` returns all artifacts.
    /// * `limit` — Maximum number of results (caller must clamp to [1, 1000]).
    /// * `before` — Optional Unix timestamp; only artifacts created before this time.
    #[tracing::instrument(skip(self), fields(job_id = ?job_id, before = ?before))]
    pub async fn list(
        &self,
        job_id: Option<String>,
        limit: u32,
        before: Option<i64>,
    ) -> Result<Vec<ArtifactMeta>, ArtifactError> {
        use sqlx::query_builder::QueryBuilder;

        let mut b = QueryBuilder::new(
            "SELECT hash, job_id, width, height, format, seed, steps, prompt, created_at FROM artifacts",
        );

        let mut has_where = false;

        if job_id.is_some() {
            b.push(" WHERE job_id = ");
            b.push_bind(job_id);
            has_where = true;
        }

        if let Some(ts) = before {
            if has_where {
                b.push(" AND");
            } else {
                b.push(" WHERE");
            }
            // `before` timestamp is a literal since it's already an i64 from the
            // parsed query parameter — safe to interpolate directly.
            b.push(format!(" created_at < {ts}"));
        }

        b.push(" ORDER BY created_at DESC LIMIT ");
        b.push_bind(limit);

        let sql_str = b.sql().as_ref().to_owned();
        tracing::debug!(query = %sql_str, "artifact_store: list query");

        let query = b.build();

        let rows = query
            .try_map(|row: sqlx::sqlite::SqliteRow| {
                Ok(ArtifactMeta {
                    hash: row.try_get("hash")?,
                    job_id: row.try_get("job_id")?,
                    width: row.try_get("width")?,
                    height: row.try_get("height")?,
                    format: row.try_get("format")?,
                    seed: row.try_get("seed")?,
                    steps: row.try_get("steps")?,
                    prompt: row.try_get("prompt")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .fetch_all(&self.db)
            .await?;

        tracing::debug!(count = rows.len(), "artifact_store: list returned");

        Ok(rows)
    }
}

#[async_trait::async_trait]
impl ArtifactSave for ArtifactStore {
    async fn save(
        &self,
        job_id: &str,
        image_b64: &str,
        meta: ArtifactSaveInput,
    ) -> Result<String, String> {
        let meta_input = ArtifactStoreInput {
            width: meta.width,
            height: meta.height,
            seed: meta.seed,
            steps: meta.steps,
            prompt: meta.prompt,
        };
        let artifact = self
            .save(job_id, image_b64, meta_input)
            .await
            .map_err(|e| e.to_string())?;
        Ok(artifact.hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    /// Create an in-memory SQLite pool with the artifacts table.
    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect in-memory SQLite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS artifacts (
                hash       TEXT PRIMARY KEY,
                job_id     TEXT    NOT NULL,
                width      INTEGER NOT NULL,
                height     INTEGER NOT NULL,
                format     TEXT    NOT NULL,
                seed       INTEGER NOT NULL,
                steps      INTEGER NOT NULL,
                prompt     TEXT    NOT NULL,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create artifacts table");

        pool
    }

    /// Create an artifact row in the artifacts table.
    async fn insert_artifact(pool: &SqlitePool, hash: &str, job_id: &str, created_at: i64) {
        sqlx::query(
            "INSERT INTO artifacts (hash, job_id, width, height, format, seed, steps, prompt, created_at) \
             VALUES (?, ?, 512, 512, 'png', 42, 20, 'test prompt', ?)",
        )
        .bind(hash)
        .bind(job_id)
        .bind(created_at)
        .execute(pool)
        .await
        .expect("insert artifact");
    }

    #[tokio::test]
    async fn list_empty_returns_empty_array() {
        let pool = setup_pool().await;
        let store = ArtifactStore::new(tempfile::tempdir().unwrap().keep(), pool);

        let result = store.list(None, 100, None).await.unwrap();
        assert!(result.is_empty(), "empty DB should return empty vec");
    }

    #[tokio::test]
    async fn list_with_job_id_filter() {
        let pool = setup_pool().await;
        let store = ArtifactStore::new(tempfile::tempdir().unwrap().keep(), pool.clone());

        let job_a = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let job_b = "b2c3d4e5-f6a7-8901-bcde-f12345678901";

        insert_artifact(&pool, "hash_a", job_a, 1000).await;
        insert_artifact(&pool, "hash_b", job_b, 1001).await;

        let result = store
            .list(Some(job_a.to_string()), 100, None)
            .await
            .unwrap();
        assert_eq!(result.len(), 1, "should return only job_a's artifact");
        assert_eq!(result[0].job_id, job_a);
        assert_eq!(result[0].hash, "hash_a");
    }

    #[tokio::test]
    async fn list_limit_clamped() {
        let pool = setup_pool().await;
        let store = ArtifactStore::new(tempfile::tempdir().unwrap().keep(), pool.clone());

        for i in 0..5 {
            insert_artifact(&pool, &format!("hash_{i}"), "job-x", 1000 + i as i64).await;
        }

        let result = store.list(None, 2, None).await.unwrap();
        assert_eq!(result.len(), 2, "limit=2 must return exactly 2 artifacts");
        // Newest first: hash_4, hash_3
        assert_eq!(result[0].hash, "hash_4");
        assert_eq!(result[1].hash, "hash_3");
    }

    #[tokio::test]
    async fn list_before_filter() {
        let pool = setup_pool().await;
        let store = ArtifactStore::new(tempfile::tempdir().unwrap().keep(), pool.clone());

        insert_artifact(&pool, "hash_old", "job-y", 1000).await;
        insert_artifact(&pool, "hash_mid", "job-y", 2000).await;
        insert_artifact(&pool, "hash_new", "job-y", 3000).await;

        // before=2500 with created_at < 2500 should return hash_mid (2000) and hash_old (1000),
        // but NOT hash_new (3000).
        let result = store.list(None, 100, Some(2500)).await.unwrap();
        assert_eq!(result.len(), 2, "before=2500 should return 2 artifacts");
        assert_eq!(result[0].hash, "hash_mid");
        assert_eq!(result[1].hash, "hash_old");
    }
}
