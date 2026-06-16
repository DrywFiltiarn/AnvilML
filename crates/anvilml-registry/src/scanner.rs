//! Model directory scanner for AnvilML.
//!
//! Walks configured model directories, inspects `.safetensors` files, and derives
//! metadata (`ModelKind` from parent directory name, `ModelDtype` from filename,
//! `ModelFormat` from extension, `ModelMeta::id` from SHA256 of first 1 MiB).
//!
//! This module implements the model scanner described in `ANVILML_DESIGN.md`.
//! It is invoked at server startup to populate the model registry.

use std::path::Path;

use tokio::io::AsyncReadExt;

use anvilml_core::{ModelDirConfig, ModelDtype, ModelFormat, ModelKind, ModelMeta};
use chrono::Utc;
use sha2::{Digest, Sha256};
use tracing::info;

/// Zero-size unit struct that performs model directory scanning.
///
/// Contains no state — all metadata is derived from the input parameters
/// (`ModelDirConfig` slice and file system contents). Construct with
/// `ModelScanner` (the unit value) and call `scan()`.
pub struct ModelScanner;

impl ModelScanner {
    /// Scan configured model directories and return metadata for all discovered
    /// model files.
    ///
    /// Walks each directory in `dirs`, inspects `.safetensors` files, and derives
    /// `ModelKind` from the parent directory name, `ModelDtype` from the filename,
    /// and `ModelFormat` from the file extension. The model ID is the SHA256 hex
    /// digest of the first 1 MiB of file content.
    ///
    /// **Note:** Recursive directory walking is not yet implemented. Only the
    /// top-level directory of each `ModelDirConfig` is scanned. The `recursive`
    /// and `max_depth` fields on `ModelDirConfig` are accepted but ignored by
    /// this version of the scanner.
    ///
    /// # Arguments
    ///
    /// * `dirs` — Slice of `ModelDirConfig` specifying directories to scan.
    ///
    /// # Returns
    ///
    /// A `Vec<ModelMeta>` containing metadata for every discovered `.safetensors`
    /// model file. Non-`.safetensors` files and non-existent directories are
    /// silently skipped (logged at DEBUG level).
    #[tracing::instrument(skip(self, dirs))]
    pub async fn scan(&self, dirs: &[ModelDirConfig]) -> Vec<ModelMeta> {
        let mut results = Vec::new();

        for dir_config in dirs {
            // Check if the directory exists before attempting to read it.
            // Using `std::fs::metadata` (sync) for the existence check avoids
            // an extra async call — the directory check is a local filesystem
            // operation that is fast and does not benefit from async.
            if std::fs::metadata(&dir_config.path).is_err() {
                // Directory does not exist — log and skip to next directory.
                // This is a common case during development when model directories
                // are not yet populated, so we use DEBUG level (not WARN) to avoid
                // noise in production logs.
                tracing::debug!(
                    path = %dir_config.path.display(),
                    reason = "directory_not_found",
                    "skipping model directory"
                );
                continue;
            }

            // Open the directory for async reading. `tokio::fs::read_dir` yields
            // entries asynchronously, which is important when scanning directories
            // on network filesystems or slow storage.
            let mut entries = match tokio::fs::read_dir(&dir_config.path).await {
                Ok(entries) => entries,
                Err(e) => {
                    // If we cannot read the directory (permissions, etc.), log
                    // and skip — do not propagate the error to avoid a single
                    // bad directory from crashing the entire scan.
                    tracing::debug!(
                        path = %dir_config.path.display(),
                        error = %e,
                        "failed to read directory"
                    );
                    continue;
                }
            };

            while let Some(entry) = entries.next_entry().await.transpose() {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        // Individual entry read error (e.g. stale symlink).
                        // Skip it rather than aborting the entire scan.
                        tracing::debug!(
                            path = %dir_config.path.display(),
                            error = %e,
                            "failed to read directory entry"
                        );
                        continue;
                    }
                };

                let entry_path = entry.path();

                // Skip non-file entries (subdirectories, symlinks to directories,
                // device files, etc.). We only process regular files to avoid
                // following directory structures or reading special files.
                let metadata = match tokio::fs::metadata(&entry_path).await {
                    Ok(m) => m,
                    Err(_) => {
                        // TOCTOU race: file disappeared between `read_dir` and
                        // `metadata` call. Skip silently.
                        continue;
                    }
                };

                if !metadata.is_file() {
                    // Not a regular file — skip. This includes subdirectories,
                    // symlinks to directories, sockets, etc.
                    tracing::debug!(
                        path = %entry_path.display(),
                        reason = "not_a_file",
                        "skipping non-file entry"
                    );
                    continue;
                }

                // Only process `.safetensors` files. Other formats (`.ckpt`,
                // `.pt`, `.bin`) are currently unsupported by the scanner.
                // Safetensors is the recommended format per the model format docs
                // because it provides fast, safe loading without arbitrary code
                // execution.
                let filename = entry.file_name().to_string_lossy().into_owned();

                if !filename.ends_with(".safetensors") {
                    // Not a .safetensors file — skip. Future tasks may add support
                    // for other formats by extending this filter and the format
                    // inference logic.
                    tracing::debug!(
                        path = %entry_path.display(),
                        reason = "unsupported_format",
                        "skipping non-safetensors file"
                    );
                    continue;
                }

                // All checks passed — derive metadata and construct ModelMeta.
                let dir_name = entry
                    .path()
                    .parent()
                    .and_then(|p| p.file_name())
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();

                let id = match compute_id(&entry_path).await {
                    Ok(id) => id,
                    Err(e) => {
                        // Hash computation failed (e.g. file unreadable). Log and
                        // skip this file rather than aborting the scan.
                        tracing::debug!(
                            path = %entry_path.display(),
                            error = %e,
                            "failed to compute model id"
                        );
                        continue;
                    }
                };

                let file_size = std::fs::metadata(&entry_path).map(|m| m.len()).unwrap_or(0);

                // Attempt to read dtype from the safetensors header. The header is
                // authoritative — it reflects the actual stored tensor precision
                // regardless of what the filename claims. Falls back to filename
                // inference only if the header read fails (e.g. truncated file,
                // wrong magic, permissions error).
                let dtype = match self.read_dtype_from_header(&entry_path).await {
                    Ok(dt) => dt,
                    Err(e) => {
                        tracing::debug!(
                            path = %entry_path.display(),
                            error = %e,
                            "safetensors header read failed, falling back to filename inference"
                        );
                        self.infer_dtype(&filename)
                    }
                };

                results.push(ModelMeta {
                    id,
                    name: filename.clone(),
                    path: entry_path.to_string_lossy().into_owned(),
                    kind: self.infer_kind(&dir_name),
                    dtype,
                    format: self.infer_format(&filename),
                    size_bytes: file_size,
                    scanned_at: Utc::now(),
                });
            }
        }

        // Log the scan completion summary. This is a mandatory INFO log point
        // per ENVIRONMENT.md §9 (Model scan → Scan completed).
        let dir_paths: Vec<String> = dirs
            .iter()
            .map(|d| d.path.to_string_lossy().into_owned())
            .collect();

        info!(
            count = results.len(),
            dir = %dir_paths.join(","),
            "model scan completed"
        );

        results
    }

    /// Infer the `ModelKind` from a directory name.
    ///
    /// Performs a case-insensitive match on the directory component to determine
    /// the model's role in the generative pipeline. Unknown names map to
    /// `ModelKind::Unknown`.
    ///
    /// # Arguments
    ///
    /// * `dir_name` — The file name component of the parent directory
    ///   (e.g. `"diffusion"`, `"text_encoders"`, `"vae"`).
    ///
    /// # Returns
    ///
    /// The `ModelKind` variant corresponding to the directory name, or
    /// `ModelKind::Unknown` if no match is found.
    pub(crate) fn infer_kind(&self, dir_name: &str) -> ModelKind {
        // Convert to lowercase for case-insensitive matching. This handles
        // edge cases like "Diffusion/" or "TEXT_ENCODERS" correctly, and is
        // simpler than multiple `starts_with` checks.
        let lower = dir_name.to_lowercase();

        match lower.as_str() {
            "diffusion" => ModelKind::Diffusion,
            "text_encoders" | "clip" => ModelKind::TextEncoder,
            "vae" => ModelKind::Vae,
            "loras" | "lora" => ModelKind::Lora,
            "controlnet" => ModelKind::ControlNet,
            "upscale" => ModelKind::Upscale,
            _ => ModelKind::Unknown,
        }
    }

    /// Infer the `ModelDtype` from a filename.
    ///
    /// Performs a case-insensitive substring search on the filename to detect
    /// precision indicators (`fp8`, `fp16`, `bf16`, `fp32`). The check order
    /// is intentional: `fp8` is checked before `fp16` to correctly handle
    /// filenames that mention both precisions (e.g. `model_fp16_fp8.safetensors`).
    ///
    /// # Arguments
    ///
    /// * `filename` — The file name component (e.g. `"model_fp8.safetensors"`).
    ///
    /// # Returns
    ///
    /// The `ModelDtype` variant corresponding to the detected precision, or
    /// `ModelDtype::Unknown` if no precision indicator is found.
    pub(crate) fn infer_dtype(&self, filename: &str) -> ModelDtype {
        // Check order matters: fp8 must come before fp16 to correctly handle
        // filenames like "model_fp16_fp8_quantized.safetensors" where both
        // strings appear. The `fp8` substring does not overlap with `fp16`,
        // but checking fp8 first ensures quantized files are classified as Fp8.
        let lower = filename.to_lowercase();

        if lower.contains("fp8") {
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

    /// Infer the `ModelFormat` from a filename extension.
    ///
    /// Matches the file extension (case-insensitive) against known model
    /// formats. Safetensors is the recommended format per the crate docs.
    ///
    /// # Arguments
    ///
    /// * `filename` — The file name component (e.g. `"model.safetensors"`).
    ///
    /// # Returns
    ///
    /// The `ModelFormat` variant corresponding to the file extension, or
    /// `ModelFormat::Unknown` if no known extension is found.
    fn infer_format(&self, filename: &str) -> ModelFormat {
        // Use `ends_with` with case-insensitive comparison. This is simpler
        // than parsing extensions via `path::Path::extension()` because we
        // already have the filename string and want to avoid allocating a
        // PathBuf just for extension parsing.
        let lower = filename.to_lowercase();

        if lower.ends_with(".safetensors") {
            ModelFormat::Safetensors
        } else if lower.ends_with(".ckpt") {
            ModelFormat::Ckpt
        } else if lower.ends_with(".pt") {
            ModelFormat::Pt
        } else if lower.ends_with(".bin") {
            ModelFormat::Bin
        } else {
            ModelFormat::Unknown
        }
    }

    /// Read the dominant `ModelDtype` from a safetensors file header.
    ///
    /// Parses only the JSON header (8-byte length prefix + header bytes).
    /// No tensor data is read. The dtype is determined by majority vote across
    /// all tensor entries — the dtype with the highest occurrence count wins.
    /// In the common case of a uniformly-typed file every entry shares the same
    /// dtype and the result is unambiguous.
    ///
    /// The `__metadata__` key is explicitly skipped — it is not a tensor entry.
    ///
    /// Safetensors dtype strings (`"BF16"`, `"F16"`, `"F8_E4M3"`, etc.) differ
    /// from the filename substrings checked by `infer_dtype` and are mapped
    /// separately here.
    ///
    /// # Errors
    ///
    /// Returns `std::io::Error` if the file cannot be opened, is too short to
    /// contain a valid 8-byte length prefix, the declared header length is out
    /// of range, or the header bytes are not valid JSON.
    async fn read_dtype_from_header(
        &self,
        path: &std::path::Path,
    ) -> Result<ModelDtype, std::io::Error> {
        let mut file = tokio::fs::File::open(path).await?;

        // Read the 8-byte little-endian u64 header length prefix.
        // This is the first field in every valid safetensors file per the spec.
        let mut len_buf = [0u8; 8];
        file.read_exact(&mut len_buf).await?;
        let header_len = u64::from_le_bytes(len_buf) as usize;

        // Guard against malformed files that claim an unreasonable header size.
        // Real headers are typically well under 1 MiB; 100 MiB is a generous
        // ceiling that no legitimate model file would exceed.
        const MAX_HEADER_BYTES: usize = 100 * 1024 * 1024;
        if header_len == 0 || header_len > MAX_HEADER_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("safetensors header length out of range: {header_len}"),
            ));
        }

        // Read the header JSON bytes immediately following the length prefix.
        let mut header_buf = vec![0u8; header_len];
        file.read_exact(&mut header_buf).await?;

        // Parse into a generic JSON object. We only need the "dtype" field
        // from each tensor entry — deserialising into a generic map avoids
        // defining a full TensorInfo struct and keeps the dependency surface
        // minimal.
        let header: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&header_buf)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        // Count occurrences of each dtype string across all tensor entries.
        // "__metadata__" is the only non-tensor key defined by the safetensors
        // spec; all other keys are tensor names.
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for (key, value) in &header {
            if key == "__metadata__" {
                // Skip the metadata block — it is not a tensor entry and its
                // fields do not follow the tensor entry schema.
                continue;
            }
            if let Some(dtype_str) = value.get("dtype").and_then(|v| v.as_str()) {
                *counts.entry(dtype_str.to_string()).or_insert(0) += 1;
            }
        }

        // Select the dtype with the highest occurrence count. On an empty file
        // (no tensor entries) `max_by_key` returns `None`, which maps to Unknown.
        // Ties are non-deterministic but vanishingly rare in practice.
        let dominant = counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(dtype_str, _)| dtype_str);

        // Map safetensors dtype strings to ModelDtype variants.
        // The format uses uppercase with underscores ("BF16", "F8_E4M3"), which
        // are distinct from the lowercase filename substrings infer_dtype checks.
        // Both F8 exponent variants (E4M3 and E5M2) map to Fp8 — ModelDtype does
        // not distinguish between the two NaN encodings.
        Ok(match dominant.as_deref() {
            Some("F32") => ModelDtype::Fp32,
            Some("F16") => ModelDtype::Fp16,
            Some("BF16") => ModelDtype::Bf16,
            Some("F8_E4M3") | Some("F8_E5M2") => ModelDtype::Fp8,
            Some("F4") => ModelDtype::Fp4,
            // No tensor entries or unrecognised dtype string — fall through
            // to Unknown, which will prompt the caller to try filename inference.
            _ => ModelDtype::Unknown,
        })
    }
}

/// Compute a deterministic model ID by hashing the first 1 MiB of the file.
///
/// Opens the file at `path`, reads at most 1 MiB (1048576 bytes), computes
/// the SHA256 digest, and returns the result as a lowercase hex string.
/// If the file is smaller than 1 MiB, the entire file is hashed.
///
/// # Arguments
///
/// * `path` — Filesystem path to the model file.
///
/// # Errors
///
/// Returns `std::io::Error` if the file cannot be opened or read.
///
/// # Rationale
///
/// We only hash the first 1 MiB rather than the full file because model files
/// can be 10+ GB. Hashing the entire file would be prohibitively slow and
/// memory-intensive. The first 1 MiB contains enough unique data (header,
/// tensor metadata) to produce a deterministic, collision-resistant identifier
/// for practical purposes.
async fn compute_id(path: &Path) -> Result<String, std::io::Error> {
    // Open the file for async reading. We use `OpenOptions` rather than
    // `tokio::fs::read` because we only need the first 1 MiB — reading the
    // entire file into memory would be wasteful for large model files.
    let file = tokio::fs::OpenOptions::new().read(true).open(path).await?;

    // Read at most 1 MiB. The `take()` combinator limits the bytes read from
    // the stream. If the file is smaller than 1 MiB, all bytes are read
    // naturally — no special handling needed.
    let mut buf = Vec::new();
    file.take(1048576).read_to_end(&mut buf).await?;

    // Compute SHA256 hex digest. The `Sha256::digest` convenience method
    // consumes the input bytes and returns a `GenericArray<u8, U32>`, which
    // we format as a lowercase hex string for storage and comparison.
    Ok(format!("{:x}", Sha256::digest(&buf)))
}
