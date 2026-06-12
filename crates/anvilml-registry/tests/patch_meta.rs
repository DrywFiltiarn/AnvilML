//! Integration tests for `anvilml_registry::store::ModelRegistry::patch_meta`.

use std::path::PathBuf;

use anvilml_core::{DType, ModelKind, ModelMeta, ModelMetaPatch};
use chrono::Utc;
use sqlx::SqlitePool;

/// Helper: open a database at the given path and return a ready pool.
async fn open_pool(path: &std::path::Path) -> SqlitePool {
    anvilml_registry::db::open(path).await.unwrap()
}

/// Patching dtype_hint from F16 to F32 should update the dtype and recompute
/// vram_estimate_mib (2.0x factor for F32 vs 1.0x for F16).
#[tokio::test]
async fn patch_meta_updates_dtype_recomputes_vram() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let scanned_at = Utc::now();
    let meta = ModelMeta {
        id: "patch-test-001".to_string(),
        name: "Patch Test Model".to_string(),
        path: PathBuf::from("/models/patch-test.safetensors"),
        kind: ModelKind::Diffusion,
        size_bytes: 6_700_000_000,
        dtype_hint: DType::F16,
        vram_estimate_mib: 6_700,
        scanned_at,
    };

    registry.upsert(&meta).await.unwrap();

    // Patch dtype from F16 to F32.
    let patch = ModelMetaPatch {
        dtype_hint: Some(DType::F32),
        kind: None,
    };

    let result = registry.patch_meta(&meta.id, patch).await.unwrap();

    let updated = result.expect("patch_meta should return Some for existing model");

    assert_eq!(updated.id, meta.id);
    assert_eq!(updated.dtype_hint, DType::F32);
    // 6_700_000_000 bytes = 6391 MiB; F32 factor 2.0 -> 12782 MiB
    assert_eq!(updated.vram_estimate_mib, 12_778);
    // kind should be unchanged
    assert_eq!(updated.kind, ModelKind::Diffusion);
}

/// Patching kind should update kind and recompute vram (vram unchanged when
/// dtype stays the same).
#[tokio::test]
async fn patch_meta_updates_kind_keeps_dtype() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let scanned_at = Utc::now();
    let meta = ModelMeta {
        id: "patch-test-002".to_string(),
        name: "Patch Test Model 2".to_string(),
        path: PathBuf::from("/models/patch-test-2.safetensors"),
        kind: ModelKind::Diffusion,
        size_bytes: 6_700_000_000,
        dtype_hint: DType::F32,
        vram_estimate_mib: 12_782,
        scanned_at,
    };

    registry.upsert(&meta).await.unwrap();

    let patch = ModelMetaPatch {
        dtype_hint: None,
        kind: Some(ModelKind::Vae),
    };

    let result = registry.patch_meta(&meta.id, patch).await.unwrap();

    let updated = result.expect("patch_meta should return Some for existing model");

    assert_eq!(updated.id, meta.id);
    assert_eq!(updated.kind, ModelKind::Vae);
    // dtype unchanged, so vram stays the same
    assert_eq!(updated.dtype_hint, DType::F32);
    assert_eq!(updated.vram_estimate_mib, 12_778);
}

/// Patching a non-existent model ID returns None.
#[tokio::test]
async fn patch_meta_missing_returns_none() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let patch = ModelMetaPatch {
        dtype_hint: Some(DType::F32),
        kind: None,
    };

    let result = registry.patch_meta("nonexistent", patch).await.unwrap();

    assert!(result.is_none(), "expected None for missing model ID");
}

/// Patching with all None fields is a no-op (returns unchanged model).
#[tokio::test]
async fn patch_meta_all_none_is_noop() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let scanned_at = Utc::now();
    let meta = ModelMeta {
        id: "patch-test-003".to_string(),
        name: "Patch Test Model 3".to_string(),
        path: PathBuf::from("/models/patch-test-3.safetensors"),
        kind: ModelKind::Lora,
        size_bytes: 500_000_000,
        dtype_hint: DType::Q8,
        vram_estimate_mib: 238,
        scanned_at,
    };

    registry.upsert(&meta).await.unwrap();

    let patch = ModelMetaPatch {
        dtype_hint: None,
        kind: None,
    };

    let result = registry.patch_meta(&meta.id, patch).await.unwrap();

    let updated = result.expect("patch_meta should return Some for existing model");

    assert_eq!(updated.id, meta.id);
    assert_eq!(updated.dtype_hint, DType::Q8);
    assert_eq!(updated.kind, ModelKind::Lora);
}
