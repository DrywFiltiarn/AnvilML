//! Content-addressed PNG artifact persistence.
//!
//! Implements `ArtifactStore` which decodes a base64-encoded PNG image,
//! computes its SHA-256 hash, writes it to a two-char-prefix-sharded
//! directory under `artifact_dir`, inserts the artifact metadata row
//! into SQLite, and increments the job's `artifact_count`.

use std::path::PathBuf;

use base64::prelude::BASE64_STANDARD;
use base64::Engine as _;
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use thiserror::Error;

/// Metadata about an artifact stored in the registry.
///
/// Matches the design spec (§4.2, §13) — a local type that mirrors the
/// `artifacts` table schema from migration `003_artifacts.sql`.
#[derive(Debug, Clone)]
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
}
