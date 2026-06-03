//! Model directory scanner — walks configured model directories and discovers weight files.
//!
//! Discovers `.safetensors`, `.ckpt`, `.pt`, `.bin` files, computes deterministic IDs
//! via SHA-256 of canonical paths, and infers kind/dtype heuristics.

use anvilml_core::config::ModelDirConfig;
use anvilml_core::{DType, ModelKind, ModelMeta};
use sha2::{Digest, Sha256};

/// Allowed model file extensions (without the dot).
const ALLOWED_EXTENSIONS: &[&str] = &["safetensors", "ckpt", "pt", "bin"];

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
        DType::Q8 => 0.5,
        DType::Q4 => 0.25,
        DType::Unknown => 1.0,
    };
    let estimate = (size_mib as f64) * factor;
    estimate.max(1.0) as u32
}

/// Compute a SHA-256 hex digest of the input string.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Scan configured model directories and return discovered model metadata.
pub async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta> {
    let mut results = Vec::new();

    for dir_config in dirs {
        for entry in walkdir::WalkDir::new(&dir_config.path)
            .follow_links(false)
            .into_iter()
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Only process files.
            if !entry.file_type().is_file() {
                continue;
            }

            // Check allowed extension.
            let ext = match entry.path().extension().and_then(|e| e.to_str()) {
                Some(ext) => ext,
                None => continue,
            };
            if !ALLOWED_EXTENSIONS.contains(&ext) {
                continue;
            }

            // Get file size.
            let size_bytes = match entry.metadata() {
                Ok(m) => m.len(),
                Err(_) => continue,
            };

            // Extract name from file stem.
            let name = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();

            // Compute ID from SHA-256 of canonical path string.
            let canonical_path = entry
                .path()
                .canonicalize()
                .unwrap_or_else(|_| entry.path().to_path_buf());
            let canonical_str = canonical_path.to_string_lossy().to_string();
            let full_hash = sha256_hex(&canonical_str);
            let id: String = full_hash.chars().take(16).collect();

            // Infer kind: explicit config or from parent directory name.
            let parent_dir_name = canonical_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let kind = dir_config
                .kind
                .unwrap_or_else(|| infer_kind(parent_dir_name));

            // Infer dtype from file stem suffix.
            let stem = entry
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let dtype = infer_dtype(stem);

            // Estimate VRAM.
            let vram = vram_estimate_mib(size_bytes, dtype);

            results.push(ModelMeta {
                id,
                name,
                path: canonical_path,
                kind,
                size_bytes,
                dtype_hint: dtype,
                vram_estimate_mib: vram,
                ..ModelMeta::default()
            });
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
    fn test_vram_estimate_mib() {
        // 1 MiB file (1048576 bytes) with F32 → 2.0 MiB
        assert_eq!(vram_estimate_mib(1048576, DType::F32), 2);
        // 1 MiB file with F16 → 1.0 MiB
        assert_eq!(vram_estimate_mib(1048576, DType::F16), 1);
        // 1 MiB file with Q4 → 0.25 MiB → clamped to 1
        assert_eq!(vram_estimate_mib(1048576, DType::Q4), 1);
        // Small file (e.g. 100 bytes) → 0 MiB → clamped to 1
        assert_eq!(vram_estimate_mib(100, DType::Unknown), 1);
    }

    #[test]
    fn test_sha256_hex() {
        let hash = sha256_hex("hello world");
        assert_eq!(hash.len(), 64);
        // Known SHA-256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
