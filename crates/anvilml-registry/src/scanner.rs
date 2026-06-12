//! Model directory scanner — walks configured model directories and discovers weight files.
//!
//! Discovers `.safetensors`, `.ckpt`, `.pt`, `.bin` files, computes deterministic IDs
//! via SHA-256 of file content (4 MiB head + 4 MiB tail), and infers kind/dtype heuristics.

use std::collections::HashMap;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use anvilml_core::config::ModelDirConfig;
use anvilml_core::{DType, ModelKind, ModelMeta};
use sha2::{Digest, Sha256};

/// Allowed model file extensions (without the dot).
const ALLOWED_EXTENSIONS: &[&str] = &["safetensors", "ckpt", "pt", "bin"];

/// Maximum safetensors header size in bytes (100 MiB).
const MAX_SAFETENSORS_HEADER: u64 = 100 * 1024 * 1024;

/// Map a safetensors dtype string to an `DType`.
fn map_dtype_str(s: &str) -> DType {
    match s {
        "F32" => DType::F32,
        "F16" => DType::F16,
        "BF16" => DType::BF16,
        "F8_E4M3" => DType::F8E4M3,
        "F8_E5M2" => DType::F8E5M2,
        "I8" => DType::Q8,
        "I4" => DType::Q4,
        _ => DType::Unknown,
    }
}

/// Read a safetensors file header and infer dtype from the most-frequent key dtype string.
///
/// Safetensors files store a JSON header at the start containing per-tensor dtype
/// annotations. This function reads that header, counts dtype occurrences across all
/// tensor keys (excluding `__metadata__`), and returns the most-frequent dtype.
///
/// Returns `None` on any read, parse, or decode error — the caller should fall back
/// to filename-based inference.
pub fn read_safetensors_dtype(path: &Path) -> Option<DType> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf).ok()?;
    let header_len = u64::from_le_bytes(len_buf);

    // Guard against excessively large headers.
    if header_len > MAX_SAFETENSORS_HEADER {
        return None;
    }

    let mut header_bytes = vec![0u8; header_len as usize];
    file.read_exact(&mut header_bytes).ok()?;

    let header_str = std::str::from_utf8(&header_bytes).ok()?;
    let json_value: serde_json::Value = serde_json::from_str(header_str).ok()?;

    let object = json_value.as_object()?;

    // Count dtype strings across all keys (skip __metadata__).
    let mut dtype_counts: HashMap<String, u32> = HashMap::new();
    for (key, value) in object {
        if key == "__metadata__" {
            continue;
        }
        if let Some(dtype_str) = value.as_str() {
            *dtype_counts.entry(dtype_str.to_string()).or_insert(0) += 1;
        }
    }

    // Find the key with the maximum count.
    let most_frequent = dtype_counts.into_iter().max_by_key(|&(_, count)| count)?;
    Some(map_dtype_str(&most_frequent.0))
}

/// Infer the model kind from a parent directory name (case-insensitive exact match).
pub fn infer_kind(parent_dir: &str) -> ModelKind {
    let lower = parent_dir.to_lowercase();
    match lower.as_str() {
        "diffusion" => ModelKind::Diffusion,
        "vae" => ModelKind::Vae,
        "lora" => ModelKind::Lora,
        "clip" => ModelKind::Clip,
        "controlnet" => ModelKind::ControlNet,
        "unet" => ModelKind::Unet,
        "upscale" => ModelKind::Upscale,
        _ => ModelKind::default(),
    }
}

/// Infer the data type from a file-stem suffix (case-insensitive).
pub fn infer_dtype(filename: &str) -> DType {
    let lower = filename.to_lowercase();
    if lower.ends_with("f32") {
        DType::F32
    } else if lower.ends_with("fp8e4m3") || lower.ends_with("f8e4m3") {
        DType::F8E4M3
    } else if lower.ends_with("fp8e5m2") || lower.ends_with("f8e5m2") {
        DType::F8E5M2
    } else if lower.ends_with("fp8") || lower.ends_with("f8") {
        DType::F8E4M3
    } else if lower.ends_with("bf16") {
        // Must be checked before f16, since "bf16" ends with "f16".
        DType::BF16
    } else if lower.ends_with("fp16") || lower.ends_with("f16") {
        DType::F16
    } else if lower.ends_with("q8") {
        DType::Q8
    } else if lower.ends_with("q4") {
        DType::Q4
    } else {
        DType::Unknown
    }
}

/// Estimate VRAM consumption in MiB from file size and data type.
///
/// Converts bytes to MiB, applies a per-dtype memory factor, and enforces a minimum of 1 MiB.
pub fn vram_estimate_mib(size_bytes: u64, dtype: DType) -> u32 {
    let size_mib = size_bytes / (1024 * 1024);
    let factor = match dtype {
        DType::F32 => 2.0,
        DType::F16 | DType::BF16 => 1.0,
        DType::F8E4M3 | DType::F8E5M2 | DType::Q8 => 0.5,
        DType::Q4 => 0.25,
        DType::Unknown => 1.0,
    };
    let estimate = (size_mib as f64) * factor;
    estimate.max(1.0) as u32
}

/// Compute a model identity hash from file content.
///
/// Reads up to `CHUNK` bytes from the head and `CHUNK` bytes from the tail,
/// feeds both into SHA-256, and returns the first 16 hex characters.
/// Files smaller than `2 × CHUNK` are read in full.
fn content_hash_id(path: &Path) -> io::Result<String> {
    const CHUNK: u64 = 4 * 1024 * 1024; // 4 MiB

    let mut file = std::fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut hasher = Sha256::new();

    if file_len <= CHUNK * 2 {
        let mut buf = Vec::with_capacity(file_len as usize);
        file.read_to_end(&mut buf)?;
        hasher.update(&buf);
    } else {
        let mut head = vec![0u8; CHUNK as usize];
        file.read_exact(&mut head)?;
        hasher.update(&head);

        file.seek(SeekFrom::End(-(CHUNK as i64)))?;
        let mut tail = vec![0u8; CHUNK as usize];
        file.read_exact(&mut tail)?;
        hasher.update(&tail);
    }

    let full_hex = hex::encode(hasher.finalize());
    Ok(full_hex.chars().take(16).collect())
}

/// Scan configured model directories and return discovered model metadata.
///
/// Every accepted file is resolved to a **canonical absolute path** via
/// [`std::fs::canonicalize`] before any further processing. This guarantees:
///
/// - The same physical file always produces the same path string regardless of
///   the process working directory at scan time.
/// - Paths stored in the DB via [`crate::store::ModelRegistry::upsert`] are
///   absolute, so the stale-detection pass in `rescan` works correctly on both
///   Windows and Linux without any CWD dependency.
/// - On Windows, `canonicalize` returns `\\?\`-prefixed UNC paths; the store's
///   `norm_path` helper strips that prefix and normalises separators to `/`
///   before writing to SQLite, keeping stored paths platform-neutral.
pub async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta> {
    let mut results = Vec::new();

    for dir_config in dirs {
        for entry in walkdir::WalkDir::new(&dir_config.path)
            .follow_links(false)
            .into_iter()
        {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    let path = dir_config.path.clone();
                    if e.io_error().map(|inner| inner.kind()) == Some(io::ErrorKind::NotFound) {
                        tracing::warn!(path = %path.display(), "scanner: skipping missing path");
                    } else {
                        tracing::warn!(path = %path.display(), error = %e, "scanner: skipping unreadable entry");
                    }
                    continue;
                }
            };

            // Only process files.
            if !entry.file_type().is_file() {
                tracing::debug!(path = %entry.path().display(), reason = "not a file", "scanner: skipped");
                continue;
            }

            // Check allowed extension.
            let ext = match entry.path().extension().and_then(|e| e.to_str()) {
                Some(ext) => ext,
                None => {
                    tracing::debug!(path = %entry.path().display(), reason = "extension not matched", "scanner: skipped");
                    continue;
                }
            };
            if !ALLOWED_EXTENSIONS.contains(&ext) {
                tracing::debug!(path = %entry.path().display(), reason = "extension not matched", "scanner: skipped");
                continue;
            }

            // Get file size.
            let size_bytes = match entry.metadata() {
                Ok(m) => m.len(),
                Err(e) => {
                    if e.io_error().map(|inner| inner.kind()) == Some(io::ErrorKind::NotFound) {
                        tracing::warn!(path = %entry.path().display(), "scanner: skipping missing path");
                    } else {
                        tracing::warn!(path = %entry.path().display(), error = %e, "scanner: skipping file with unreadable metadata");
                    }
                    continue;
                }
            };

            // Resolve to a canonical absolute path. This is the single point where
            // relative paths produced by WalkDir (e.g. `.\models\diffusion\file.sft`)
            // become stable absolute paths. All downstream consumers — ID hashing,
            // DB storage, stale detection — operate on this canonical form.
            let canonical_path = match std::fs::canonicalize(entry.path()) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(
                        path = %entry.path().display(),
                        error = %e,
                        "scanner: canonicalize failed, skipping file"
                    );
                    continue;
                }
            };

            // Extract name from file stem of the canonical path.
            let name = canonical_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();

            // Compute ID from SHA-256 of file content (4 MiB head + 4 MiB tail).
            let id = match content_hash_id(&canonical_path) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(path = %canonical_path.display(), error = %e, "scanner: skipping unreadable file");
                    continue;
                }
            };
            let id_clone = id.clone();

            // Infer kind: explicit config or from parent directory name.
            let parent_dir_name = canonical_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let kind = dir_config
                .kind
                .unwrap_or_else(|| infer_kind(parent_dir_name));

            // Infer dtype: try safetensors header first, fall back to filename.
            let stem = canonical_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let dtype = if ext == "safetensors" {
                match read_safetensors_dtype(&canonical_path) {
                    Some(DType::Unknown) | None => infer_dtype(stem),
                    Some(dtype) => dtype,
                }
            } else {
                infer_dtype(stem)
            };

            // Estimate VRAM.
            let vram = vram_estimate_mib(size_bytes, dtype);

            results.push(ModelMeta {
                id,
                name,
                path: canonical_path.clone(),
                kind,
                size_bytes,
                dtype_hint: dtype,
                vram_estimate_mib: vram,
                ..ModelMeta::default()
            });

            tracing::debug!(path = %canonical_path.display(), id = %id_clone, "scanner: accepted");
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_kind_matches() {
        assert_eq!(infer_kind("diffusion"), ModelKind::Diffusion);
        assert_eq!(infer_kind("vae"), ModelKind::Vae);
        assert_eq!(infer_kind("lora"), ModelKind::Lora);
        assert_eq!(infer_kind("clip"), ModelKind::Clip);
        assert_eq!(infer_kind("controlnet"), ModelKind::ControlNet);
        assert_eq!(infer_kind("unet"), ModelKind::Unet);
        assert_eq!(infer_kind("upscale"), ModelKind::Upscale);
    }

    #[test]
    fn test_infer_kind_case_insensitive() {
        assert_eq!(infer_kind("Diffusion"), ModelKind::Diffusion);
        assert_eq!(infer_kind("VAE"), ModelKind::Vae);
        assert_eq!(infer_kind("LoRA"), ModelKind::Lora);
    }

    #[test]
    fn test_infer_kind_fallback() {
        assert_eq!(infer_kind("unknown_dir"), ModelKind::default());
        assert_eq!(infer_kind("foobar"), ModelKind::default());
    }

    #[test]
    fn test_infer_dtype_matches() {
        assert_eq!(infer_dtype("model-f32"), DType::F32);
        assert_eq!(infer_dtype("model-fp16"), DType::F16);
        assert_eq!(infer_dtype("model-f16"), DType::F16);
        assert_eq!(infer_dtype("model-bf16"), DType::BF16);
        assert_eq!(infer_dtype("model-q8"), DType::Q8);
        assert_eq!(infer_dtype("model-q4"), DType::Q4);
    }

    #[test]
    fn test_infer_dtype_case_insensitive() {
        assert_eq!(infer_dtype("MODEL-FP16"), DType::F16);
        assert_eq!(infer_dtype("model-BF16"), DType::BF16);
    }

    #[test]
    fn test_infer_dtype_unknown() {
        assert_eq!(infer_dtype("model"), DType::Unknown);
        assert_eq!(infer_dtype("weights"), DType::Unknown);
    }

    #[test]
    fn test_infer_dtype_fp8_suffixes() {
        assert_eq!(infer_dtype("model-fp8"), DType::F8E4M3);
        assert_eq!(infer_dtype("model-f8"), DType::F8E4M3);
        assert_eq!(infer_dtype("MODEL-FP8"), DType::F8E4M3);
        assert_eq!(infer_dtype("model-fp8e4m3"), DType::F8E4M3);
        assert_eq!(infer_dtype("model-fp8e5m2"), DType::F8E5M2);
        assert_eq!(infer_dtype("model-f8e4m3"), DType::F8E4M3);
        assert_eq!(infer_dtype("model-f8e5m2"), DType::F8E5M2);
    }

    #[test]
    fn test_vram_estimate_mib() {
        // 1 MiB file (1048576 bytes) with F32 -> 2.0 MiB
        assert_eq!(vram_estimate_mib(1048576, DType::F32), 2);
        // 1 MiB file with F16 -> 1.0 MiB
        assert_eq!(vram_estimate_mib(1048576, DType::F16), 1);
        // 1 MiB file with Q4 -> 0.25 MiB -> clamped to 1
        assert_eq!(vram_estimate_mib(1048576, DType::Q4), 1);
        // 1 MiB file with F8E4M3 -> 0.5 MiB -> clamped to 1
        assert_eq!(vram_estimate_mib(1048576, DType::F8E4M3), 1);
        // Small file (e.g. 100 bytes) -> 0 MiB -> clamped to 1
        assert_eq!(vram_estimate_mib(100, DType::Unknown), 1);
    }

    #[test]
    fn test_content_hash_id_small_file() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("small.safetensors");
        std::fs::write(&path, b"hello world").expect("write file");

        let id = content_hash_id(&path).expect("hash small file");
        assert_eq!(id.len(), 16, "id must be 16 hex chars");
        // Must be stable: same content → same id.
        let id2 = content_hash_id(&path).expect("hash again");
        assert_eq!(id, id2, "content hash must be deterministic");
    }

    #[test]
    fn test_content_hash_id_different_content() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path_a = tmp.path().join("a.safetensors");
        let path_b = tmp.path().join("b.safetensors");
        std::fs::write(&path_a, b"content_a").expect("write a");
        std::fs::write(&path_b, b"content_b").expect("write b");

        let id_a = content_hash_id(&path_a).expect("hash a");
        let id_b = content_hash_id(&path_b).expect("hash b");
        assert_ne!(id_a, id_b, "different content must produce different ids");
    }

    #[test]
    fn test_map_dtype_str() {
        assert_eq!(map_dtype_str("F32"), DType::F32);
        assert_eq!(map_dtype_str("F16"), DType::F16);
        assert_eq!(map_dtype_str("BF16"), DType::BF16);
        assert_eq!(map_dtype_str("F8_E4M3"), DType::F8E4M3);
        assert_eq!(map_dtype_str("F8_E5M2"), DType::F8E5M2);
        assert_eq!(map_dtype_str("I8"), DType::Q8);
        assert_eq!(map_dtype_str("I4"), DType::Q4);
        assert_eq!(map_dtype_str("UNKNOWN"), DType::Unknown);
    }

    #[test]
    fn test_read_safetensors_dtype_header_wins() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("model-f32.safetensors");

        // Build a safetensors header: mostly F16 keys, one F32 key.
        let header = serde_json::json!({
            "tensor_a": "F16",
            "tensor_b": "F16",
            "tensor_c": "F16",
            "tensor_d": "F32",
        });
        let header_bytes = serde_json::to_vec(&header).expect("serialize header");
        let header_len = (header_bytes.len() as u64).to_le_bytes();

        let mut data = Vec::with_capacity(8 + header_bytes.len());
        data.extend_from_slice(&header_len);
        data.extend_from_slice(&header_bytes);
        std::fs::write(&path, &data).expect("write safetensors file");

        let dtype = read_safetensors_dtype(&path);
        assert_eq!(
            dtype,
            Some(DType::F16),
            "header F16 should win over filename f32"
        );
    }

    #[test]
    fn test_read_safetensors_dtype_fallback_malformed() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("weights-q8.safetensors");

        // Write invalid binary data (not a valid safetensors header).
        std::fs::write(&path, b"\x00\x00\x00\x00\x00\x00\x00\x00invalid")
            .expect("write malformed file");

        let dtype = read_safetensors_dtype(&path);
        assert!(dtype.is_none(), "malformed header should return None");

        // Scanner should fall back to infer_dtype from filename.
        assert_eq!(infer_dtype("weights-q8"), DType::Q8);
    }

    #[test]
    fn test_read_safetensors_dtype_fp8_header() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("model-fp8.safetensors");

        // Build a safetensors header with F8_E4M3 keys.
        let header = serde_json::json!({
            "layers.0.weight": "F8_E4M3",
            "layers.1.weight": "F8_E4M3",
            "layers.2.weight": "F8_E4M3",
            "layers.3.bias": "F8_E4M3",
        });
        let header_bytes = serde_json::to_vec(&header).expect("serialize header");
        let header_len = (header_bytes.len() as u64).to_le_bytes();

        let mut data = Vec::with_capacity(8 + header_bytes.len());
        data.extend_from_slice(&header_len);
        data.extend_from_slice(&header_bytes);
        std::fs::write(&path, &data).expect("write safetensors file");

        let dtype = read_safetensors_dtype(&path);
        assert_eq!(
            dtype,
            Some(DType::F8E4M3),
            "header F8_E4M3 should map to F8E4M3"
        );
    }

    #[test]
    fn test_read_safetensors_dtype_too_large_header() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("huge.safetensors");

        // Write a header length that exceeds the guard (100 MiB + 1 byte).
        let oversized_len = (MAX_SAFETENSORS_HEADER + 1).to_le_bytes();
        std::fs::write(&path, &oversized_len).expect("write oversized header file");

        let dtype = read_safetensors_dtype(&path);
        assert!(dtype.is_none(), "oversized header should return None");
    }

    #[test]
    fn test_read_safetensors_dtype_nonexistent() {
        let dtype = read_safetensors_dtype(Path::new("/nonexistent/path/file.safetensors"));
        assert!(dtype.is_none(), "nonexistent file should return None");
    }

    #[test]
    fn test_read_safetensors_dtype_empty_header() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("empty.safetensors");

        // Write a header length of 0 (empty JSON object).
        let len_bytes = (0u64).to_le_bytes();
        std::fs::write(&path, &len_bytes).expect("write empty header file");

        let dtype = read_safetensors_dtype(&path);
        assert!(
            dtype.is_none(),
            "empty header with no tensor keys should return None"
        );
    }

    #[test]
    fn test_read_safetensors_dtype_metadata_only() {
        let tmp = tempfile::tempdir().expect("create tempdir");
        let path = tmp.path().join("metadata-only.safetensors");

        // Header with only __metadata__ (no tensor keys).
        let header = serde_json::json!({
            "__metadata__": {
                "format": "pt"
            }
        });
        let header_bytes = serde_json::to_vec(&header).expect("serialize header");
        let header_len = (header_bytes.len() as u64).to_le_bytes();

        let mut data = Vec::with_capacity(8 + header_bytes.len());
        data.extend_from_slice(&header_len);
        data.extend_from_slice(&header_bytes);
        std::fs::write(&path, &data).expect("write safetensors file");

        let dtype = read_safetensors_dtype(&path);
        assert!(
            dtype.is_none(),
            "header with only __metadata__ should return None"
        );
    }
}
