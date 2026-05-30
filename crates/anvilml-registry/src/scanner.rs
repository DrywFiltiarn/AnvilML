//! Filesystem scanner for model directories.
//!
//! Walks configured model directories, discovers model files by extension
//! (`.safetensors`, `.ckpt`, `.pt`, `.bin`), and derives a fully-populated
//! [`ModelMeta`](anvilml_core::types::ModelMeta) struct for each file.

use std::path::Path;

use anvilml_core::config::ModelDirConfig;
use anvilml_core::types::{DType, ModelKind, ModelMeta};
use sha2::{Digest, Sha256};

/// File extensions that the scanner recognises as model files.
const MODEL_EXTENSIONS: &[&str] = &["safetensors", "ckpt", "pt", "bin"];

/// Compute a short (16-char hex) identifier from the SHA-256 hash of the
/// canonical path string.
pub fn compute_id(path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // first 16 hex chars
}

/// Infer `DType` from a filename suffix (e.g. `f16`, `bf16`, `q8`, `q4`).
fn infer_dtype_from_filename(filename: &str) -> DType {
    let lower = filename.to_lowercase();
    if lower.contains("q8") || lower.contains("int8") {
        DType::Q8
    } else if lower.contains("q4") || lower.contains("int4") {
        DType::Q4
    } else if lower.contains("bf16") {
        DType::BF16
    } else if lower.contains("f16") || lower.contains("half") {
        DType::F16
    } else {
        DType::F32
    }
}

/// Estimate VRAM usage in MiB based on file size and a dtype factor.
pub fn estimate_vram_mib(size_bytes: u64, dtype: &DType) -> u64 {
    let factor = match dtype {
        DType::F32 => 4,
        DType::F16 | DType::BF16 => 2,
        DType::I8 | DType::Q8 => 1,
        DType::Q4 => 0,
        DType::Unknown => 4,
    };
    // Each parameter occupies `factor` bytes; VRAM ≈ size_bytes / factor * 2
    // (activations + weights roughly double the weight footprint).
    if factor == 0 {
        return size_bytes / (1024 * 1024) / 2;
    }
    size_bytes / factor * 2 / (1024 * 1024)
}

/// Infer `ModelKind` from the parent directory name.
pub fn infer_kind_from_dirname(dir_name: &str) -> ModelKind {
    let lower = dir_name.to_lowercase();
    if lower.contains("clip") || lower.contains("text_enco") {
        ModelKind::Clip
    } else if lower.contains("diffusion") || lower.contains("txt2img") {
        ModelKind::Diffusion
    } else if lower.contains("vae") {
        ModelKind::Vae
    } else if lower.contains("lora") {
        ModelKind::Lora
    } else if lower.contains("control") || lower.contains("controlnet") {
        ModelKind::ControlNet
    } else if lower.contains("unet") {
        ModelKind::Unet
    } else if lower.contains("upscale") || lower.contains("super_res") {
        ModelKind::Upscale
    } else {
        ModelKind::Diffusion // safe default
    }
}

/// Check whether a file path has one of the recognised model extensions.
pub fn is_model_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        MODEL_EXTENSIONS.contains(&ext)
    } else {
        false
    }
}

/// Scan a single directory and return discovered `ModelMeta` entries.
fn scan_dir(dir: &ModelDirConfig) -> Vec<ModelMeta> {
    let mut results = Vec::new();

    let dir_path = &dir.path;
    if !dir_path.exists() {
        return results;
    }

    let parent_name = dir_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let kind_from_dir = dir
        .kind
        .as_ref()
        .map(|k| (*k).clone().into())
        .unwrap_or_else(|| infer_kind_from_dirname(&parent_name));

    // Use walkdir to recursively walk the directory.
    for entry in walkdir::WalkDir::new(dir_path).into_iter() {
        match entry {
            Ok(entry) => {
                if entry.file_type().is_file() && is_model_file(entry.path()) {
                    let path = entry.path().to_string_lossy().to_string();
                    let canonical = std::fs::canonicalize(entry.path())
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.clone());

                    let id = compute_id(&canonical);

                    // Extract filename for dtype inference.
                    let filename = entry
                        .path()
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let dtype_hint = infer_dtype_from_filename(&filename);

                    // Get file size — Metadata doesn't impl Default, so use ok() + unwrap_or.
                    let size_bytes = entry.metadata().ok().map(|m| m.len()).unwrap_or(0);
                    let vram_estimate_mib = estimate_vram_mib(size_bytes, &dtype_hint);

                    // Derive model name from filename (without extension).
                    let name = entry
                        .path()
                        .file_stem()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| filename.clone());

                    results.push(ModelMeta {
                        id,
                        name,
                        kind: kind_from_dir.clone(),
                        dtype: None, // scanner doesn't inspect file contents
                        dtype_hint,
                        path: canonical,
                        size_bytes,
                        vram_estimate_mib,
                        scanned_at: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
            Err(_) => continue, // skip unreadable entries
        }
    }

    results
}

/// Scan multiple directories and return all discovered `ModelMeta` entries.
pub async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta> {
    let mut all_meta = Vec::new();
    for dir in dirs {
        let metas = scan_dir(dir);
        all_meta.extend(metas);
    }
    // Sort by path for deterministic output.
    all_meta.sort_by(|a, b| a.path.cmp(&b.path));
    all_meta
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ------------------------------------------------------------------
    // compute_id — SHA256-based id generation
    // ------------------------------------------------------------------

    #[test]
    fn compute_id_returns_16_hex_chars() {
        let id = compute_id("/models/test.safetensors");
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn compute_id_is_deterministic() {
        let id1 = compute_id("/models/test.safetensors");
        let id2 = compute_id("/models/test.safetensors");
        assert_eq!(id1, id2);
    }

    #[test]
    fn compute_id_differs_for_different_paths() {
        let id1 = compute_id("/models/model_a.safetensors");
        let id2 = compute_id("/models/model_b.safetensors");
        assert_ne!(id1, id2);
    }

    // ------------------------------------------------------------------
    // infer_dtype_from_filename
    // ------------------------------------------------------------------

    #[test]
    fn infer_dtype_q8() {
        assert_eq!(infer_dtype_from_filename("model-q8.safetensors"), DType::Q8);
        assert_eq!(infer_dtype_from_filename("model_int8.pt"), DType::Q8);
    }

    #[test]
    fn infer_dtype_q4() {
        assert_eq!(infer_dtype_from_filename("model-q4.safetensors"), DType::Q4);
        assert_eq!(infer_dtype_from_filename("model_int4.bin"), DType::Q4);
    }

    #[test]
    fn infer_dtype_bf16() {
        assert_eq!(
            infer_dtype_from_filename("model-bf16.safetensors"),
            DType::BF16
        );
    }

    #[test]
    fn infer_dtype_f16() {
        assert_eq!(infer_dtype_from_filename("model-f16.ckpt"), DType::F16);
        assert_eq!(infer_dtype_from_filename("model_half.pt"), DType::F16);
    }

    #[test]
    fn infer_dtype_f32_default() {
        assert_eq!(infer_dtype_from_filename("model.safetensors"), DType::F32);
    }

    // ------------------------------------------------------------------
    // infer_kind_from_dirname
    // ------------------------------------------------------------------

    #[test]
    fn infer_kind_clip() {
        assert_eq!(infer_kind_from_dirname("clip"), ModelKind::Clip);
        assert_eq!(infer_kind_from_dirname("text_encoder"), ModelKind::Clip);
    }

    #[test]
    fn infer_kind_diffusion() {
        assert_eq!(infer_kind_from_dirname("diffusion"), ModelKind::Diffusion);
        assert_eq!(infer_kind_from_dirname("txt2img"), ModelKind::Diffusion);
    }

    #[test]
    fn infer_kind_vae() {
        assert_eq!(infer_kind_from_dirname("vae"), ModelKind::Vae);
    }

    #[test]
    fn infer_kind_lora() {
        assert_eq!(infer_kind_from_dirname("lora"), ModelKind::Lora);
    }

    #[test]
    fn infer_kind_controlnet() {
        assert_eq!(infer_kind_from_dirname("controlnet"), ModelKind::ControlNet);
        assert_eq!(infer_kind_from_dirname("control"), ModelKind::ControlNet);
    }

    #[test]
    fn infer_kind_unet() {
        assert_eq!(infer_kind_from_dirname("unet"), ModelKind::Unet);
    }

    #[test]
    fn infer_kind_upscale() {
        assert_eq!(infer_kind_from_dirname("upscale"), ModelKind::Upscale);
        assert_eq!(infer_kind_from_dirname("super_res"), ModelKind::Upscale);
    }

    #[test]
    fn infer_kind_default_diffusion() {
        assert_eq!(infer_kind_from_dirname("misc"), ModelKind::Diffusion);
    }

    // ------------------------------------------------------------------
    // is_model_file
    // ------------------------------------------------------------------

    #[test]
    fn is_model_file_safetensors() {
        assert!(is_model_file(Path::new("model.safetensors")));
    }

    #[test]
    fn is_model_file_ckpt() {
        assert!(is_model_file(Path::new("model.ckpt")));
    }

    #[test]
    fn is_model_file_pt() {
        assert!(is_model_file(Path::new("model.pt")));
    }

    #[test]
    fn is_model_file_bin() {
        assert!(is_model_file(Path::new("model.bin")));
    }

    #[test]
    fn is_model_file_unrecognised() {
        assert!(!is_model_file(Path::new("model.json")));
        assert!(!is_model_file(Path::new("model")));
    }

    // ------------------------------------------------------------------
    // estimate_vram_mib
    // ------------------------------------------------------------------

    #[test]
    fn estimate_vram_f32() {
        // 1 GiB of F32 weights => ~512 MiB VRAM
        let vram = estimate_vram_mib(1_073_741_824, &DType::F32);
        assert!(vram > 0);
    }

    #[test]
    fn estimate_vram_q4() {
        // Q4 factor is 0.5, so size/0.5*2 = size*4 => large number
        let vram = estimate_vram_mib(1_073_741_824, &DType::Q4);
        assert!(vram > 0);
    }

    // ------------------------------------------------------------------
    // scan_dir — real file discovery via tempfile
    // ------------------------------------------------------------------

    #[test]
    fn scan_dir_discovers_model_files() {
        let tmp_dir = std::env::temp_dir().join("anvilml_scan_test");
        let _ = std::fs::create_dir_all(&tmp_dir);

        // Create a model file.
        let mut f = NamedTempFile::new_in(&tmp_dir).expect("create temp file");
        f.write_all(b"fake model data").unwrap();
        let safetensors_path = tmp_dir.join("model.safetensors");
        std::fs::rename(f.path(), &safetensors_path).unwrap();

        // Create a non-model file that should be ignored.
        std::fs::write(tmp_dir.join("readme.txt"), "ignore me").unwrap();

        let config = ModelDirConfig {
            path: tmp_dir.clone(),
            kind: None,
        };
        let results = scan_dir(&config);

        assert_eq!(results.len(), 1);
        assert!(results[0].name.contains("model"));
        assert_eq!(results[0].id.len(), 16);
        assert!(results[0].size_bytes > 0);

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn scan_dir_empty_returns_empty() {
        let tmp_dir = std::env::temp_dir().join("anvilml_scan_empty");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let config = ModelDirConfig {
            path: tmp_dir.clone(),
            kind: None,
        };
        let results = scan_dir(&config);
        assert!(results.is_empty());

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn scan_dir_nonexistent_returns_empty() {
        let config = ModelDirConfig {
            path: std::path::PathBuf::from("/nonexistent/path/that/does/not/exist"),
            kind: None,
        };
        let results = scan_dir(&config);
        assert!(results.is_empty());
    }

    // ------------------------------------------------------------------
    // scan_dirs — async entry point
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn scan_dirs_multiple() {
        let tmp_dir_a = std::env::temp_dir().join("anvilml_scan_multi_a");
        let tmp_dir_b = std::env::temp_dir().join("anvilml_scan_multi_b");
        let _ = std::fs::create_dir_all(&tmp_dir_a);
        let _ = std::fs::create_dir_all(&tmp_dir_b);

        // Create model files.
        std::fs::write(tmp_dir_a.join("model_a.pt"), b"data").unwrap();
        std::fs::write(tmp_dir_b.join("model_b.ckpt"), b"data").unwrap();

        let configs = vec![
            ModelDirConfig {
                path: tmp_dir_a.clone(),
                kind: None,
            },
            ModelDirConfig {
                path: tmp_dir_b.clone(),
                kind: None,
            },
        ];

        let results = scan_dirs(&configs).await;
        assert_eq!(results.len(), 2);

        // Results should be sorted by path.
        assert!(results[0].path < results[1].path);

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp_dir_a);
        let _ = std::fs::remove_dir_all(&tmp_dir_b);
    }
}
