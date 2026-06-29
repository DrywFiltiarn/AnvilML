//! ModelScanner — directory-walking scanner that derives `ModelMeta` from files on disk.
//!
//! The scanner walks a directory tree, computes a stable SHA256-based model ID from the
//! first 1 MiB of each file, infers architecture family from the parent directory name,
//! and deduces data type and file format from filename substrings. A file already in the
//! store with unchanged size and mtime is skipped — never re-hashed.
//!
//! This module is the bridge between the filesystem and the `ModelStore` persistence layer.

use crate::store::ModelStore;
use anvilml_core::{AnvilError, ModelDtype, ModelFormat, ModelKind, ModelMeta};
use chrono::Utc;
use digest::Digest;
use sha2::Sha256;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Directory-walking scanner that derives `ModelMeta` from files on disk.
///
/// The scanner owns a `ModelStore` (not a reference) because `scan_dir()` is async and
/// needs to call store methods for deduplication checks and upserts. `ModelStore` wraps
/// an `Arc`-backed `SqlitePool`, so ownership transfer is cheap.
///
/// # Usage
///
/// ```no_run
/// # use anvilml_registry::{ModelScanner, create_pool};
/// # use std::path::Path;
/// # async fn example() -> Result<(), anvilml_core::AnvilError> {
/// let pool = create_pool(Path::new("./anvilml.db")).await?;
/// let scanner = ModelScanner::new(pool);
/// let results = scanner.scan_dir(Path::new("/models"), 2).await?;
/// # Ok::<_, anvilml_core::AnvilError>(())
/// # }
/// ```
pub struct ModelScanner {
    /// Persistence layer for model metadata.
    store: ModelStore,
}

impl ModelScanner {
    /// Construct a new `ModelScanner` backed by the given connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already had migrations applied.
    ///   The pool must be connected to a database containing the `models` table.
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self {
            store: ModelStore::new(pool),
        }
    }

    /// Walk the directory tree rooted at *root* up to *depth* levels and derive
    /// `ModelMeta` for each file found.
    ///
    /// For each file:
    /// 1. Checks if the file already exists in the store with unchanged size + mtime.
    ///    If so, skips it without re-hashing.
    /// 2. Computes a SHA256 hash of the first 1 MiB (or whole file if smaller).
    /// 3. Infers `ModelKind` from the directory component relative to root.
    /// 4. Infers `ModelDtype` and `ModelFormat` from the filename.
    /// 5. Upserts the resulting `ModelMeta` into the store.
    ///
    /// # Arguments
    ///
    /// * `root` — The directory to begin scanning.
    /// * `depth` — Maximum depth to recurse. A depth of `0` scans only files directly
    ///   inside `root` (no subdirectory traversal). A depth of `2` scans `root`, one
    ///   level of subdirectories, and two levels deep.
    ///
    /// # Returns
    ///
    /// A vector of `ModelMeta` for all files that were scanned (not skipped).
    /// Files skipped due to unchanged size+mtime are not included in the result.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if a directory cannot be read or a file cannot be
    /// opened. Returns `AnvilError::Db` if the store query or upsert fails.
    #[tracing::instrument(fields(root = %root.display(), depth), skip(self))]
    pub async fn scan_dir(&self, root: &Path, depth: u32) -> Result<Vec<ModelMeta>, AnvilError> {
        // Queue-based BFS walk: (directory_path, current_depth).
        // Using a Vec as a queue with index-based iteration avoids allocation overhead
        // of a Deque for this use case.
        let mut queue: Vec<(PathBuf, u32)> = vec![(root.to_path_buf(), 0)];
        let mut results = Vec::new();

        while let Some((dir, current_depth)) = queue.pop() {
            // Skip directories that don't exist — they may have been deleted between
            // the time we queued them and when we got to them.
            if !dir.exists() {
                debug!(path = %dir.display(), "skipping non-existent directory");
                continue;
            }

            // Read directory entries.
            // The entries may include both files and subdirectories.
            let entries = std::fs::read_dir(&dir)?;

            for entry in entries {
                let entry = entry?;
                let entry_path = entry.path();

                // Determine if this entry is a file or directory.
                // Directories are enqueued for further traversal; files are scanned.
                if entry_path.is_dir() {
                    // Enqueue subdirectories for further traversal if we haven't
                    // reached the depth limit yet.
                    if current_depth < depth {
                        queue.push((entry_path, current_depth + 1));
                    }
                } else {
                    // This is a file — scan it.
                    match self.scan_file(&dir, &entry_path).await {
                        Some(meta) => results.push(meta),
                        None => {
                            // File was skipped (unchanged) — no log needed at INFO level.
                            // DEBUG log would be noisy for large directories.
                        }
                    }
                }
            }
        }

        debug!(scanned = results.len(), "scan complete");
        Ok(results)
    }

    /// Scan a single file: dedup check, hash, infer metadata, upsert.
    ///
    /// Returns `Some(ModelMeta)` if the file was scanned and upserted, `None` if
    /// it was skipped due to unchanged size+mtime.
    ///
    /// The dedup check compares the file's current size and mtime against the stored
    /// values via `get_path_info`, which reads directly from the database. If both
    /// match, the file is unchanged and we skip re-hashing to save I/O and CPU.
    async fn scan_file(&self, dir: &Path, file_path: &Path) -> Option<ModelMeta> {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Get file metadata (size, mtime).
        let metadata = match std::fs::metadata(file_path) {
            Ok(m) => m,
            Err(e) => {
                // File disappeared between directory listing and metadata read.
                // This is a race condition — log and skip rather than failing the scan.
                debug!(
                    path = %file_path.display(),
                    error = %e,
                    "file disappeared between read_dir and metadata"
                );
                return None;
            }
        };

        let size_bytes = metadata.len();
        let mtime_unix = metadata
            .modified()
            .ok()
            .and_then(|t| {
                // Try to get system time; if the filesystem doesn't support
                // modification time (e.g. some network filesystems), fall back.
                use std::time::SystemTime;
                t.duration_since(SystemTime::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
            })
            .unwrap_or(0);

        // Check dedup: if the file already exists in the store with matching size
        // and mtime, skip re-hashing. This is the primary optimization — large
        // model files (several GB) should not be re-hashed on every rescan.
        // We query `get_path_info` directly for the stored size+mtime, since
        // `ModelMeta` (returned by `list()`) does not include `mtime_unix`.
        if let Some(Some((stored_size, stored_mtime))) =
            self.store.get_path_info(file_path).await.ok()
            && stored_size == size_bytes
            && stored_mtime == mtime_unix
        {
            debug!(
                path = %file_path.display(),
                size = size_bytes,
                mtime = mtime_unix,
                "skipping unchanged file"
            );
            return None;
        }

        // Compute SHA256 hash of the first 1 MiB of the file.
        let hash = match Self::hash_file(file_path).await {
            Ok(h) => h,
            Err(e) => {
                // Hash failure is not fatal — log and skip this file.
                // The scan continues with remaining files.
                tracing::warn!(
                    path = %file_path.display(),
                    error = %e,
                    "hash failed, skipping file"
                );
                return None;
            }
        };

        // Infer metadata from filesystem context.
        // The relative directory component determines the model architecture family.
        let kind = self.infer_kind(dir);
        let dtype = self.infer_dtype(&file_name);
        let format = self.infer_format(file_path);

        let meta = ModelMeta {
            id: hash,
            name: file_name,
            path: file_path.to_path_buf(),
            kind,
            dtype,
            format,
            size_bytes,
            // Populate mtime_unix from the file's actual modification time.
            // This enables the dedup check: on subsequent scans, we compare
            // the stored mtime against the file's current mtime to detect changes.
            mtime_unix,
            scanned_at: Utc::now(),
        };

        // Upsert into the store. Uses INSERT OR REPLACE so this handles both
        // new files (insert) and changed files (replace) in a single statement.
        if let Err(e) = self.store.upsert(&meta).await {
            tracing::warn!(
                path = %file_path.display(),
                error = %e,
                "upsert failed, skipping file"
            );
            return None;
        }

        debug!(
            path = %file_path.display(),
            hash = %meta.id,
            kind = ?meta.kind,
            dtype = ?meta.dtype,
            "scanned file"
        );

        Some(meta)
    }

    /// Compute the SHA256 hex digest of the first 1 MiB (or whole file if smaller).
    ///
    /// Opens the file, reads up to 1 MiB in one read, and computes the SHA256 hash.
    /// The result is returned as a lowercase hex string (64 characters).
    ///
    /// # Arguments
    ///
    /// * `path` — The filesystem path to the file to hash.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the file cannot be opened or read.
    async fn hash_file(path: &Path) -> Result<String, AnvilError> {
        const ONE_MIB: usize = 1024 * 1024; // 1 MiB

        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();

        // Read up to 1 MiB in a single read. For files smaller than 1 MiB,
        // the entire file is hashed. For larger files, only the first 1 MiB
        // is used — this is the model ID derivation contract.
        let mut buffer = vec![0u8; ONE_MIB];
        let bytes_read = std::io::Read::read(&mut file, &mut buffer)?;

        // Only hash the bytes that were actually read (handles files < 1 MiB).
        hasher.update(&buffer[..bytes_read]);

        let result = hasher.finalize();
        // Convert the 32-byte result to a lowercase hex string (64 characters).
        // sha2 0.11's Digest::finalize() returns a GenericArray<u8, U32> which
        // does not implement Debug or LowerHex formatting — we convert manually.
        let hex: String = result.iter().map(|b| format!("{:02x}", b)).collect();
        Ok(hex)
    }

    /// Infer `ModelKind` from the directory component relative to the scan root.
    ///
    /// Matches the directory name against known model architecture directories:
    /// `diffusion`, `text_encoders`, `vae`. All other directory names map to `Unknown`.
    ///
    /// # Arguments
    ///
    /// * `dir` — The directory path relative to the scan root.
    fn infer_kind(&self, dir: &Path) -> ModelKind {
        // Extract the last path component (the directory name).
        // For example: "diffusion" from "diffusion/model.safetensors".
        let dir_name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        match dir_name.as_str() {
            "diffusion" => ModelKind::Diffusion,
            "text_encoders" => ModelKind::TextEncoder,
            "vae" => ModelKind::Vae,
            _ => ModelKind::Unknown,
        }
    }

    /// Infer `ModelDtype` from the filename (case-insensitive substring matching).
    ///
    /// Checks for dtype indicators in priority order: `fp8_e4m3fn`, `fp8_e5m2`, `fp8`,
    /// `fp16`, `bf16`, `fp32`. The first match wins. No match → `Unknown`.
    ///
    /// # Arguments
    ///
    /// * `filename` — The file name string (e.g. `"model_fp8_e4m3fn.safetensors"`).
    fn infer_dtype(&self, filename: &str) -> ModelDtype {
        let lower = filename.to_lowercase();

        // Check fp8 variants first (more specific patterns) before the generic `fp8`.
        // This ensures `fp8_e4m3fn` matches `Fp8` and not `Unknown`.
        if lower.contains("fp8_e4m3fn") || lower.contains("fp8_e5m2") || lower.contains("fp8") {
            ModelDtype::Fp8
        } else if lower.contains("fp16") {
            ModelDtype::Fp16
        } else if lower.contains("bf16") {
            ModelDtype::Bf16
        } else if lower.contains("fp32") {
            ModelDtype::Fp32
        } else {
            ModelDtype::Unknown
        }
    }

    /// Infer `ModelFormat` from the file extension (case-insensitive).
    ///
    /// Matches against known extensions: `.safetensors`, `.ckpt`, `.pt`, `.pth`,
    /// `.bin`, `.gguf`. All other extensions map to `Unknown`.
    ///
    /// # Arguments
    ///
    /// * `path` — The file path to extract the extension from.
    fn infer_format(&self, path: &Path) -> ModelFormat {
        // Extract the extension (without the leading dot).
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "safetensors" => ModelFormat::Safetensors,
            "ckpt" => ModelFormat::Ckpt,
            "pt" | "pth" => ModelFormat::Pt,
            "bin" | "gguf" => ModelFormat::Bin,
            _ => ModelFormat::Unknown,
        }
    }
}
