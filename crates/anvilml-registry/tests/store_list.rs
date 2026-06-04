//! Integration tests for `ModelRegistry::list`.

use std::path::PathBuf;

use anvilml_core::{DType, ModelKind, ModelMeta};
use chrono::Utc;
use sqlx::SqlitePool;

/// Helper: open a database at the given path and return a ready pool.
async fn open_pool(path: &std::path::Path) -> SqlitePool {
    anvilml_registry::db::open(path).await.unwrap()
}

/// Upsert a `ModelMeta` for convenience in tests.
async fn upsert(registry: &anvilml_registry::ModelRegistry, meta: &ModelMeta) {
    registry.upsert(meta).await.unwrap();
}

/// Listing on an empty database returns an empty vector.
#[tokio::test]
async fn test_list_empty_returns_empty_vec() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let results = registry.list(None).await.unwrap();
    assert!(
        results.is_empty(),
        "expected empty list on a fresh database"
    );
}

/// After upserting 3 models with unsorted names, `list(None)` returns all 3
/// ordered by name ascending (Alpha, Mango, Zebra).
#[tokio::test]
async fn test_list_after_upserts_returns_ordered() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let scanned_at = Utc::now();

    upsert(
        &registry,
        &ModelMeta {
            id: "zebra-001".to_string(),
            name: "Zebra".to_string(),
            path: PathBuf::from("/models/zebra.safetensors"),
            kind: ModelKind::Diffusion,
            size_bytes: 2_000_000_000,
            dtype_hint: DType::F16,
            vram_estimate_mib: 4096,
            scanned_at,
        },
    )
    .await;

    upsert(
        &registry,
        &ModelMeta {
            id: "alpha-001".to_string(),
            name: "Alpha".to_string(),
            path: PathBuf::from("/models/alpha.safetensors"),
            kind: ModelKind::Vae,
            size_bytes: 1_000_000_000,
            dtype_hint: DType::F32,
            vram_estimate_mib: 2048,
            scanned_at,
        },
    )
    .await;

    upsert(
        &registry,
        &ModelMeta {
            id: "mango-001".to_string(),
            name: "Mango".to_string(),
            path: PathBuf::from("/models/mango.safetensors"),
            kind: ModelKind::Lora,
            size_bytes: 500_000_000,
            dtype_hint: DType::F16,
            vram_estimate_mib: 1024,
            scanned_at,
        },
    )
    .await;

    let results = registry.list(None).await.unwrap();
    assert_eq!(results.len(), 3, "expected exactly 3 models");
    assert_eq!(results[0].name, "Alpha");
    assert_eq!(results[1].name, "Mango");
    assert_eq!(results[2].name, "Zebra");
}

/// `list(Some(kind))` returns only models whose kind matches.
#[tokio::test]
async fn test_list_kind_filter() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();

    let pool = open_pool(path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let scanned_at = Utc::now();

    // Upsert 2 Diffusion models.
    upsert(
        &registry,
        &ModelMeta {
            id: "diff-001".to_string(),
            name: "DiffOne".to_string(),
            path: PathBuf::from("/models/diff1.safetensors"),
            kind: ModelKind::Diffusion,
            size_bytes: 3_000_000_000,
            dtype_hint: DType::F16,
            vram_estimate_mib: 6144,
            scanned_at,
        },
    )
    .await;

    upsert(
        &registry,
        &ModelMeta {
            id: "diff-002".to_string(),
            name: "DiffTwo".to_string(),
            path: PathBuf::from("/models/diff2.safetensors"),
            kind: ModelKind::Diffusion,
            size_bytes: 4_000_000_000,
            dtype_hint: DType::F16,
            vram_estimate_mib: 8192,
            scanned_at,
        },
    )
    .await;

    // Upsert 1 Vae model.
    upsert(
        &registry,
        &ModelMeta {
            id: "vae-001".to_string(),
            name: "VaeOne".to_string(),
            path: PathBuf::from("/models/vae1.safetensors"),
            kind: ModelKind::Vae,
            size_bytes: 1_500_000_000,
            dtype_hint: DType::F32,
            vram_estimate_mib: 3072,
            scanned_at,
        },
    )
    .await;

    let results = registry.list(Some(ModelKind::Diffusion)).await.unwrap();
    assert_eq!(results.len(), 2, "expected exactly 2 Diffusion models");
    for meta in &results {
        assert_eq!(
            meta.kind,
            ModelKind::Diffusion,
            "every result should be a Diffusion model"
        );
    }
}
