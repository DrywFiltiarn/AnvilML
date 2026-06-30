//! ArtifactStore — content-addressed PNG artifact storage with SQLite metadata persistence.
//!
//! Provides `ArtifactStore`, the persistence layer for generated PNG artifacts.
//! Artifacts are stored by content hash (SHA-256) in a configurable directory,
//! and metadata is persisted in an SQLite database.
//!
//! The `artifacts` table schema is created automatically on first save via
//! `CREATE TABLE IF NOT EXISTS` as a safety net — the canonical schema lives
//! in `database/migrations/002_artifacts.sql`. When the migration runner is
//! wired in, the inline DDL becomes a no-op.

use anvilml_core::{AnvilError, ArtifactMeta};
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::SqlitePool;
use std::path::PathBuf;
use uuid::Uuid;

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
    /// Uses `CREATE TABLE IF NOT EXISTS` as a safety net so that `save()`
    /// remains functional even when the migration runner has not yet been
    /// wired into pool creation. The schema matches `002_artifacts.sql`
    /// exactly, so when that migration runs first, the inline DDL becomes
    /// a no-op.
    async fn ensure_artifacts_table(&self) -> Result<(), AnvilError> {
        // Inline DDL is a temporary fallback — the canonical schema lives in
        // database/migrations/002_artifacts.sql.  CREATE TABLE IF NOT EXISTS
        // is idempotent, so this is safe to run on every save() call.
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

    /// Retrieve a saved artifact by its content hash.
    ///
    /// Reads the PNG file at `{artifact_dir}/{hash}.png` from disk and returns
    /// its bytes. Returns `Ok(None)` if no file exists for the given hash
    /// (file not found). Returns `Err` for any other I/O error (permission
    /// denied, truncated file, etc.).
    ///
    /// This is a pure filesystem read — it does not query the database.
    /// The database row may exist without the file (partial save), or
    /// the file may exist without the row (prior to P6-B1's DB persistence).
    ///
    /// # Arguments
    ///
    /// * `hash` — The SHA-256 hex content address to look up.
    ///
    /// # Returns
    ///
    /// `Ok(bytes)` with the PNG file contents if the file exists,
    /// `Ok(None)` if no file exists for this hash,
    /// or `Err(AnvilError::Io)` for other filesystem errors.
    #[tracing::instrument(fields(artifact_dir = %self.artifact_dir.display()), skip(self))]
    pub async fn get(&self, hash: &str) -> Result<Option<Vec<u8>>, AnvilError> {
        // Construct the content-addressed file path: {artifact_dir}/{hash}.png
        // The .png extension is appended because all artifacts are stored as PNG.
        let file_path = self.artifact_dir.join(format!("{hash}.png"));

        // Attempt to read the file. If it doesn't exist, return None
        // rather than an error — this is the expected "not found" path
        // for a content-addressed store.
        match std::fs::read(&file_path) {
            Ok(bytes) => {
                tracing::debug!(hash = %hash, bytes = bytes.len(), "artifact read from disk");
                Ok(Some(bytes))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(hash = %hash, "artifact not found on disk");
                Ok(None)
            }
            Err(err) => {
                // Any other I/O error (permission denied, etc.) propagates
                // as an Io error via the From<std::io::Error> impl on AnvilError.
                tracing::error!(hash = %hash, error = %err, "failed to read artifact from disk");
                Err(err.into())
            }
        }
    }

    /// List artifact metadata, optionally filtered by job ID.
    ///
    /// Queries the `artifacts` table and returns all rows, or only rows
    /// matching the given `job_id` when `Some(job_id)` is provided.
    /// Returns an empty vector when no rows match (not an error).
    ///
    /// # Arguments
    ///
    /// * `job_id` — Optional job UUID to filter by. `None` returns all rows.
    ///
    /// # Returns
    ///
    /// A `Vec<ArtifactMeta>` containing all matching artifact metadata rows,
    /// or an empty vector if no rows match.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the database query fails.
    #[tracing::instrument(fields(artifact_dir = %self.artifact_dir.display()), skip(self))]
    pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>, AnvilError> {
        // Ensure the artifacts table exists before querying.
        // This is necessary because list() can be called without any prior save(),
        // and ensure_artifacts_table() is only called during save().
        self.ensure_artifacts_table().await?;

        // Build the SQL query: SELECT all artifact columns from the artifacts table.
        // When job_id is Some, add a WHERE clause to filter by that job.
        // The WHERE clause uses parameter binding (? placeholder) to prevent SQL injection.
        // We use query() + manual mapping because ArtifactMeta contains PathBuf,
        // which is not a native sqlx SQLite type.
        let rows = if let Some(jid) = job_id {
            // Filter by job_id — the WHERE clause uses a bound parameter (?)
            // so the UUID is safely serialised as a TEXT value.
            sqlx::query(
                "SELECT hash, job_id, width, height, seed, steps, created_at, file_path \
                 FROM artifacts WHERE job_id = ?",
            )
            .bind(jid.to_string())
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| map_row(&row))
            .collect::<Result<Vec<_>, _>>()?
        } else {
            // No filter — return all rows.
            sqlx::query(
                "SELECT hash, job_id, width, height, seed, steps, created_at, file_path \
                 FROM artifacts",
            )
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| map_row(&row))
            .collect::<Result<Vec<_>, _>>()?
        };

        tracing::debug!(count = rows.len(), job_id = ?job_id, "list completed");
        Ok(rows)
    }
}

/// Map a sqlx SQLite row to an `ArtifactMeta`.
///
/// This is a helper used by `list()` to convert raw database rows into
/// the domain type. Each column is fetched by name and converted to the
/// appropriate Rust type (TEXT → String, INTEGER → u32/i64, etc.).
fn map_row(row: &sqlx::sqlite::SqliteRow) -> Result<ArtifactMeta, sqlx::Error> {
    // Fetch each column by name. sqlx's try_get handles the native
    // type mapping (TEXT → String, INTEGER → u32/i64, etc.).
    // PathBuf is converted from the TEXT column via String::into().
    let hash: String = row.try_get("hash")?;
    let job_id_str: String = row.try_get("job_id")?;
    let job_id: Uuid = job_id_str
        .parse()
        .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let width: u32 = row.try_get("width")?;
    let height: u32 = row.try_get("height")?;
    let seed: i64 = row.try_get("seed")?;
    let steps: u32 = row.try_get("steps")?;
    let created_at_str: String = row.try_get("created_at")?;
    let created_at: chrono::DateTime<chrono::Utc> = created_at_str
        .parse()
        .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    let file_path: PathBuf = row.try_get::<String, _>("file_path")?.into();

    Ok(ArtifactMeta {
        hash,
        job_id,
        width,
        height,
        seed,
        steps,
        created_at,
        file_path,
    })
}
