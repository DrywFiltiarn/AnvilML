//! Content-addressed PNG artifact storage.
//!
//! `ArtifactStore` persists PNG images produced by job execution to disk
//! using their SHA-256 content hash as the filename. This ensures that
//! identical images are never stored twice (idempotent saves). Metadata
//! is recorded in the `artifacts` SQLite table for later retrieval and
//! listing.
//!
//! # Idempotency
//!
//! Both the filesystem write and the database insert are idempotent:
//! - `std::fs::metadata(path).is_ok()` skips the write if the file
//!   already exists on disk, avoiding unnecessary I/O.
//! - `INSERT OR IGNORE` on the `hash` UNIQUE constraint silently skips
//!   duplicate inserts at the database level.
//!
//! # Thread safety
//!
//! `ArtifactStore` is `!Sync` by default because it holds a `PathBuf`
//! (not `Arc<PathBuf>`). Callers should share the store via `Arc` if
//! multiple threads need concurrent access.

use anvilml_core::types::ArtifactMeta;
use anvilml_core::AnvilError;
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

type Result<T> = std::result::Result<T, AnvilError>;

/// Content-addressed PNG artifact storage backend.
///
/// Persists PNG images to disk by their SHA-256 hash and records metadata
/// in the `artifacts` SQLite table. Identical image bytes are never stored
/// twice — the `save` method checks both disk and database before writing.
///
/// # Fields
///
/// * `dir` — The root directory where artifact files are stored. Files
///   are placed at `{dir}/{hash}.png`.
/// * `db` — The shared SQLite connection pool for artifact metadata.
///   `SqlitePool` is internally thread-safe and can be cloned cheaply.
pub struct ArtifactStore {
    /// Root directory for artifact files.
    ///
    /// Each artifact is stored as `{dir}/{hash}.png`. The directory is
    /// created on construction if it does not exist.
    dir: PathBuf,

    /// Shared SQLite connection pool for artifact metadata.
    ///
    /// `SqlitePool` is internally thread-safe and cheap to clone, so
    /// no additional Arc/Mutex wrapping is needed.
    db: sqlx::SqlitePool,
}

impl ArtifactStore {
    /// Create a new `ArtifactStore` backed by the given directory and database pool.
    ///
    /// Ensures the artifact directory exists on disk by calling
    /// `std::fs::create_dir_all`. This is idempotent — if the directory
    /// already exists, the call is a no-op.
    ///
    /// # Arguments
    ///
    /// * `dir` — The root directory where artifact PNG files will be stored.
    /// * `db` — The shared SQLite connection pool for artifact metadata.
    pub async fn new(dir: PathBuf, db: sqlx::SqlitePool) -> Self {
        // Ensure the artifact directory exists. create_dir_all is a no-op
        // if the directory already exists, making this safe to call
        // multiple times. We use blocking I/O here because directory
        // creation is a rare, fast operation that doesn't benefit from
        // async.
        std::fs::create_dir_all(&dir).expect("failed to create artifact directory");

        Self { dir, db }
    }

    /// Save an artifact image to disk and record its metadata in the database.
    ///
    /// The image is stored at `{dir}/{sha256_hex_digest}.png`. If a file with
    /// the same hash already exists on disk, the write is skipped (idempotent).
    /// The database insert uses `INSERT OR IGNORE` to prevent duplicate rows.
    ///
    /// # Arguments
    ///
    /// * `job_id` — The UUID of the job that produced this artifact.
    /// * `image_bytes` — The raw PNG image bytes to persist.
    ///
    /// # Returns
    ///
    /// The `ArtifactMeta` for the saved artifact, including the computed hash,
    /// file path, size, and dimensions. Returns `AnvilError::Io` if the file
    /// write fails, or `AnvilError::Db` if the database operation fails.
    #[tracing::instrument(skip(self))]
    pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta> {
        // Compute the SHA-256 hash of the image bytes. This hash serves as
        // the content address — identical bytes always produce the same hash,
        // enabling deduplication at both the filesystem and database levels.
        let mut hasher = Sha256::new();
        hasher.update(image_bytes);
        let result = hasher.finalize();
        let hash = format!("{:x}", result);

        // Build the file path as {dir}/{hash}.png. The .png extension is
        // always added because ArtifactStore only handles PNG images.
        let path = self.dir.join(format!("{}.png", hash));

        // Check if the file already exists on disk. If so, skip the write
        // to avoid unnecessary I/O — the hash is content-addressed, so
        // identical bytes would produce the same file.
        if std::fs::metadata(&path).is_ok() {
            // File already exists — the content is already stored. We still
            // need to ensure the database row exists (it may have been deleted
            // while the file persisted), so we fall through to the INSERT.
            tracing::debug!(
                hash = %hash,
                path = %path.display(),
                "artifact file already exists on disk, skipping write"
            );
        } else {
            // File does not exist — write the image bytes to disk. This is
            // the first time we're seeing this content hash.
            tokio::fs::write(&path, image_bytes)
                .await
                .map_err(AnvilError::Io)?;
        }

        let size_bytes = image_bytes.len() as i64;
        let created_at = Utc::now();

        // Insert into the artifacts table. The INSERT OR IGNORE clause
        // ensures idempotency at the database level: if a row with the
        // same hash already exists (UNIQUE constraint), the insert is
        // silently skipped. This handles the case where the same image
        // is produced by multiple jobs.
        //
        // Note: width and height are bound as Option<String>::None (NULL in
        // SQLite) because we don't parse the PNG header in this task — they
        // will be populated by a future task that extracts image dimensions.
        sqlx::query(
            "INSERT OR IGNORE INTO artifacts (job_id, hash, path, size_bytes, created_at, width, height)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(job_id.to_string())
        .bind(&hash)
        .bind(path.to_string_lossy().to_string())
        .bind(size_bytes)
        .bind(created_at.to_rfc3339())
        .bind::<Option<String>>(None) // width — to be populated by PNG header parsing
        .bind::<Option<String>>(None) // height — to be populated by PNG header parsing
        .execute(&self.db)
        .await?;

        // Query back the row to return correct metadata. We must read after
        // write because INSERT OR IGNORE means the row might have already
        // existed — we need the actual stored values, not the ones we tried
        // to insert.
        let row = sqlx::query(
            "SELECT id, job_id, hash, path, size_bytes, created_at, width, height
             FROM artifacts WHERE hash = ?",
        )
        .bind(&hash)
        .fetch_one(&self.db)
        .await?;

        // Map the database row to ArtifactMeta. Width and height may be NULL
        // in the database if they haven't been populated yet — we default to
        // 0 in that case. The job_id column is TEXT in SQLite, so we parse it
        // as a String first, then into a Uuid.
        let width: Option<i64> = row.get("width");
        let height: Option<i64> = row.get("height");
        let job_id_str: String = row.get("job_id");

        let meta = ArtifactMeta {
            // The `id` column is INTEGER PRIMARY KEY AUTOINCREMENT — read as i64
            // and format as a string to match ArtifactMeta's id field type.
            id: row.get::<i64, _>("id").to_string(),
            job_id: job_id_str.parse().expect("job_id should be a valid UUID"),
            hash,
            width: width.unwrap_or(0) as u32,
            height: height.unwrap_or(0) as u32,
            path: row.get::<String, _>("path"),
            size_bytes: row.get::<i64, _>("size_bytes") as u64,
            created_at: row.get("created_at"),
        };

        // Log the save event at DEBUG level for operational visibility.
        // This helps operators track artifact production rates and sizes.
        tracing::debug!(
            hash = %meta.hash,
            job_id = %job_id,
            size_bytes = meta.size_bytes,
            "artifact saved"
        );

        Ok(meta)
    }

    /// Look up an artifact's file path by its content hash.
    ///
    /// Returns `Some(path)` if an artifact with the given hash exists in
    /// the database, or `None` if no such artifact is recorded.
    ///
    /// # Arguments
    ///
    /// * `hash` — The SHA-256 hex digest to look up.
    ///
    /// # Returns
    ///
    /// `Some(PathBuf)` containing the filesystem path to the artifact file,
    /// or `None` if no artifact with this hash exists in the database.
    /// Returns `AnvilError::Db` if the database query fails.
    pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>> {
        // Query for the file path of an artifact by its content hash.
        // Returns None if no row matches — this is a normal case, not an error.
        let row = sqlx::query("SELECT path FROM artifacts WHERE hash = ?")
            .bind(hash)
            .fetch_optional(&self.db)
            .await?;

        Ok(row.map(|row| PathBuf::from(row.get::<String, _>("path"))))
    }

    /// List artifact metadata, optionally filtered by job ID.
    ///
    /// Returns all artifacts if `job_id` is `None`, or only artifacts
    /// belonging to the specified job if `job_id` is `Some`. Returns
    /// an empty vector (not an error) when no artifacts match.
    ///
    /// # Arguments
    ///
    /// * `job_id` — Optional job ID to filter by. If `None`, all artifacts
    ///   are returned.
    ///
    /// # Returns
    ///
    /// A `Vec<ArtifactMeta>` containing the metadata for each matching artifact.
    /// Returns an empty vec when no artifacts match (not an error).
    /// Returns `AnvilError::Db` if the database query fails.
    #[tracing::instrument(skip(self))]
    pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>> {
        // Build the query with an optional WHERE clause. When job_id is
        // Some, we filter to only that job's artifacts. When None, we
        // return all artifacts.
        let query = if let Some(jid) = job_id {
            sqlx::query(
                "SELECT id, job_id, hash, path, size_bytes, created_at, width, height
                 FROM artifacts WHERE job_id = ?",
            )
            .bind(jid.to_string())
        } else {
            sqlx::query(
                "SELECT id, job_id, hash, path, size_bytes, created_at, width, height
                 FROM artifacts",
            )
        };

        let rows = query.fetch_all(&self.db).await?;

        // Map database rows to ArtifactMeta structs. Width and height may
        // be NULL — we default to 0 in that case. The job_id column is TEXT
        // in SQLite, so we parse it as a String first, then into a Uuid.
        let mut artifacts = Vec::with_capacity(rows.len());
        for row in rows {
            let width: Option<i64> = row.get("width");
            let height: Option<i64> = row.get("height");
            let job_id_str: String = row.get("job_id");

            artifacts.push(ArtifactMeta {
                // The `id` column is INTEGER PRIMARY KEY AUTOINCREMENT — read as i64
                // and format as a string to match ArtifactMeta's id field type.
                id: row.get::<i64, _>("id").to_string(),
                job_id: job_id_str.parse().expect("job_id should be a valid UUID"),
                hash: row.get::<String, _>("hash"),
                width: width.unwrap_or(0) as u32,
                height: height.unwrap_or(0) as u32,
                path: row.get::<String, _>("path"),
                size_bytes: row.get::<i64, _>("size_bytes") as u64,
                created_at: row.get("created_at"),
            });
        }

        Ok(artifacts)
    }
}
