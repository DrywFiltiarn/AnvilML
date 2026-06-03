//! Integration tests for `anvilml_registry::store::ModelRegistry`.

use std::path::PathBuf;

use anvilml_core::{DType, ModelKind, ModelMeta};
use chrono::Utc;
use sqlx::SqlitePool;

/// Helper: open a database at the given path and return a ready pool.
async fn open_pool(path: &std::path::Path) -> SqlitePool {
    anvilml_registry::db::open(path).await.unwrap()
}

/// Upsert a `ModelMeta`, then retrieve it by ID — all fields must match.
#[tokio::test]
async fn test_upsert_then_get_returns_equal_meta() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let scanned_at = Utc::now();
    let meta = ModelMeta {
        id: "test-model-001".to_string(),
        name: "Stable Diffusion XL".to_string(),
        path: PathBuf::from("/models/sdxl/v1.safetensors"),
        kind: ModelKind::Diffusion,
        size_bytes: 6_700_000_000,
        dtype_hint: DType::F16,
        vram_estimate_mib: 8192,
        scanned_at,
    };

    // Upsert the model.
    registry.upsert(&meta).await.unwrap();

    // Get it back by ID.
    let retrieved = registry.get(&meta.id).await.unwrap().unwrap();

    assert_eq!(retrieved.id, meta.id);
    assert_eq!(retrieved.name, meta.name);
    assert_eq!(retrieved.path, meta.path);
    assert_eq!(retrieved.kind, meta.kind);
    assert_eq!(retrieved.size_bytes, meta.size_bytes);
    assert_eq!(retrieved.dtype_hint, meta.dtype_hint);
    assert_eq!(retrieved.vram_estimate_mib, meta.vram_estimate_mib);
    assert_eq!(retrieved.scanned_at, meta.scanned_at);
}

/// Get a non-existent model ID returns `None`.
#[tokio::test]
async fn test_get_missing_returns_none() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let result = registry.get("nonexistent-id").await.unwrap();

    assert!(result.is_none(), "expected None for missing model ID");
}
