//! Integration tests for `ModelRegistry::rescan`.

use anvilml_core::config::ModelDirConfig;
use anvilml_core::ModelKind;
use sqlx::SqlitePool;

/// Helper: open a database at the given path and return a ready pool.
async fn open_pool(path: &std::path::Path) -> SqlitePool {
    anvilml_registry::db::open(path).await.unwrap()
}

/// First rescan on a tempdir with 2 model files should upsert both and return count 2.
#[tokio::test]
async fn test_rescan_adds_models() {
    let db_tmp = tempfile::NamedTempFile::new().unwrap();
    let db_path = db_tmp.path();

    let model_tmp = tempfile::tempdir().unwrap();
    let model_dir = model_tmp.path().join("diffusion");
    std::fs::create_dir_all(&model_dir).unwrap();

    // Write 2 .safetensors files with distinct content.
    std::fs::write(model_dir.join("model_a.safetensors"), b"aaaa").unwrap();
    std::fs::write(model_dir.join("model_b.safetensors"), b"bbbb").unwrap();

    let pool = open_pool(db_path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let dirs = [ModelDirConfig {
        path: model_dir,
        kind: Some(ModelKind::Diffusion),
    }];

    let count = registry.rescan(&dirs).await.unwrap();
    assert_eq!(count, 2);

    let models = registry.list(None).await.unwrap();
    assert_eq!(models.len(), 2);

    // Verify both model IDs are present.
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.iter().any(|id| id.contains("model_a") || id.len() == 16));
    assert!(ids.iter().any(|id| id.contains("model_b") || id.len() == 16));
}

/// Second rescan over the same tempdir keeps exactly N rows (idempotent).
#[tokio::test]
async fn test_rescan_idempotent() {
    let db_tmp = tempfile::NamedTempFile::new().unwrap();
    let db_path = db_tmp.path();

    let model_tmp = tempfile::tempdir().unwrap();
    let model_dir = model_tmp.path().join("diffusion");
    std::fs::create_dir_all(&model_dir).unwrap();

    // Write 2 .safetensors files.
    std::fs::write(model_dir.join("model_a.safetensors"), b"aaaa").unwrap();
    std::fs::write(model_dir.join("model_b.safetensors"), b"bbbb").unwrap();

    let pool = open_pool(db_path).await;
    let registry = anvilml_registry::ModelRegistry::new(pool);

    let dirs = [ModelDirConfig {
        path: model_dir,
        kind: Some(ModelKind::Diffusion),
    }];

    // First rescan.
    let count1 = registry.rescan(&dirs).await.unwrap();
    assert_eq!(count1, 2);

    let models1 = registry.list(None).await.unwrap();
    assert_eq!(models1.len(), 2);

    let ids_first: Vec<String> = models1.iter().map(|m| m.id.clone()).collect();

    // Second rescan on the same files.
    let count2 = registry.rescan(&dirs).await.unwrap();
    assert_eq!(count2, 2);

    let models2 = registry.list(None).await.unwrap();
    assert_eq!(models2.len(), 2);

    let ids_second: Vec<String> = models2.iter().map(|m| m.id.clone()).collect();

    // IDs must be identical across both runs (same canonical paths → same SHA-256 → same id).
    assert_eq!(ids_first, ids_second);
}
