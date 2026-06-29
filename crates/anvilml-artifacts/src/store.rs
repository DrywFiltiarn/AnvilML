//! ArtifactStore — content-addressed PNG artifact storage with SQLite metadata persistence.
//!
//! Provides `ArtifactStore`, the persistence layer for generated PNG artifacts.
//! Artifacts are stored by content hash (SHA-256) in a configurable directory,
//! and metadata is persisted in an SQLite database.
//!
//! The `artifacts` table schema is created automatically on first save via
//! `CREATE TABLE IF NOT EXISTS` — this is a temporary measure until P6-B2
//! introduces the formal migration file `002_artifacts.sql`. When that
//! migration runs, the table will already exist, making the migration
//! a no-op for this table.

use anvilml_core::{AnvilError, ArtifactMeta};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::PathBuf;

/// Content-addressed PNG artifact storage backed by SQLite metadata.
///
/// Wraps an artifact directory and a SQLite connection pool. Artifacts are
/// stored as `{artifact_dir}/{sha256_hex}.png` files, and their metadata
/// is persisted in an `artifacts` table in the database.
///
/// The `save()` method is idempotent: calling it twice with the same PNG
/// bytes does not create a duplicate file or return an error — it simply
/// returns the same hash. This allows callers to retry safely.
pub struct ArtifactStore {
    /// Directory where PNG artifact files are stored.
    ///
    /// Each artifact is written as `{artifact_dir}/{sha256_hex}.png`.
    artifact_dir: PathBuf,
    /// Database connection pool for artifact metadata persistence.
    ///
    /// All metadata operations acquire a connection from this pool.
    pool: SqlitePool,
}

impl ArtifactStore {
    /// Construct a new `ArtifactStore` backed by the given directory and database pool.
    ///
    /// # Arguments
    ///
    /// * `artifact_dir` — Directory where PNG artifact files will be stored.
    ///   The directory is created if it does not exist (on first save).
    /// * `pool` — A `SqlitePool` connected to a database. The `artifacts` table
    ///   is created automatically on first `save()` call via `CREATE TABLE IF NOT EXISTS`.
    pub fn new(artifact_dir: PathBuf, pool: SqlitePool) -> Self {
        Self { artifact_dir, pool }
    }

    /// Save a PNG artifact by content hash.
    ///
    /// Computes the SHA-256 hex digest of `png_bytes`, writes the file to
    /// `{artifact_dir}/{hash}.png` only if it does not already exist (idempotent
    /// duplicate-save), and persists the artifact metadata row in the database.
    ///
    /// # Arguments
    ///
    /// * `png_bytes` — The raw PNG file bytes to store.
    /// * `meta` — Artifact metadata to persist (job_id, dimensions, seed, etc.).
    ///   The `hash` field in `meta` is ignored — the hash is computed from
    ///   `png_bytes`. The `file_path` field is also ignored — it is computed
    ///   from the artifact directory and the content hash.
    ///
    /// # Returns
    ///
    /// The SHA-256 hex digest of `png_bytes` on success.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the filesystem operation fails
    /// (directory creation, file write).
    /// Returns `AnvilError::Db` if the database operation fails
    /// (table creation, insert).
    #[tracing::instrument(fields(artifact_dir = %self.artifact_dir.display()), skip(self, png_bytes))]
    pub async fn save(&self, png_bytes: &[u8], meta: &ArtifactMeta) -> Result<String, AnvilError> {
        // Compute SHA-256 hex digest of the PNG bytes — this is the content
        // address that serves as the primary key for both the filesystem
        // and database rows. Using sha2 0.11.x: Digest trait provides
        // update() and finalize() methods on Sha256.
        let mut hasher = Sha256::new();
        hasher.update(png_bytes);
        // Convert the GenericArray<u8, N> output to a lowercase hex string.
        // finalize() returns a GenericArray which does not implement LowerHex,
        // so we format each byte individually.
        let hash: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();

        // Construct the file path: {artifact_dir}/{hash}.png
        let file_path = self.artifact_dir.join(format!("{hash}.png"));

        // Idempotent duplicate-save check: if the file already exists,
        // skip the write and return the hash. This allows callers to retry
        // safely without side effects.
        if file_path.exists() {
            tracing::debug!(
                hash = %hash,
                file_path = %file_path.display(),
                "artifact already exists — skipping write"
            );
            // Persist the metadata row even on duplicate — the DB row may
            // be missing if the caller crashed between file write and DB insert.
        } else {
            // Ensure the artifact directory exists before writing.
            // create_dir_all is idempotent — it succeeds if the directory
            // already exists, and creates parent directories as needed.
            std::fs::create_dir_all(&self.artifact_dir)?;

            // Write the PNG bytes to the content-addressed file path.
            std::fs::write(&file_path, png_bytes)?;

            tracing::debug!(
                hash = %hash,
                file_path = %file_path.display(),
                "artifact written to disk"
            );
        }

        // Ensure the artifacts table exists. This is a temporary measure
        // until P6-B2 introduces the formal migration 002_artifacts.sql.
        // CREATE TABLE IF NOT EXISTS is idempotent — it does nothing if
        // the table already exists, so it is safe to run on every save.
        self.ensure_artifacts_table().await?;

        // Persist the artifact metadata row keyed by hash.
        // Convert Uuid to string and DateTime<Utc> to RFC 3339 for SQL binding.
        // PathBuf is converted to string via to_string_lossy.
        let job_id_str = meta.job_id.to_string();
        let created_at_str = meta.created_at.to_rfc3339();
        let file_path_str = meta.file_path.to_string_lossy().into_owned();

        // Use INSERT OR IGNORE so that a duplicate save (same hash) is a
        // no-op at the DB level — matching the idempotent file-write behavior.
        sqlx::query(
            "INSERT OR IGNORE INTO artifacts \
             (hash, job_id, width, height, seed, steps, created_at, file_path) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&hash)
        .bind(&job_id_str)
        .bind(meta.width as i64)
        .bind(meta.height as i64)
        .bind(meta.seed)
        .bind(meta.steps as i64)
        .bind(created_at_str)
        .bind(file_path_str)
        .execute(&self.pool)
        .await?;

        tracing::debug!(hash = %hash, "artifact metadata persisted");

        Ok(hash)
    }

    /// Ensure the `artifacts` table exists in the database.
    ///
    /// Uses `CREATE TABLE IF NOT EXISTS` to avoid depending on a migration
    /// file that has not yet been introduced (deferred to P6-B2).
    ///
    /// The schema matches `002_artifacts.sql` exactly, so when that
    /// migration runs, the table will already exist and the migration
    /// becomes a no-op for this table.
    async fn ensure_artifacts_table(&self) -> Result<(), AnvilError> {
        // defers_to: P6-B2 — migration file 002_artifacts.sql replaces this
        // inline DDL when the migration system is introduced.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS artifacts (\
             hash TEXT PRIMARY KEY, \
             job_id TEXT NOT NULL, \
             width INTEGER NOT NULL, \
             height INTEGER NOT NULL, \
             seed INTEGER NOT NULL, \
             steps INTEGER NOT NULL, \
             created_at TEXT NOT NULL, \
             file_path TEXT NOT NULL)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
