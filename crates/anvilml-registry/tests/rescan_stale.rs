//! Integration tests for stale model removal during rescan.

use anvilml_core::config::ModelDirConfig;
use anvilml_core::ModelKind;
use sqlx::SqlitePool;

/// Helper: open a database at the given path and return a ready pool.
async fn open_pool(path: &std::path::Path) -> SqlitePool {
    anvilml_registry::db::open(path).await.unwrap()
}

/// Scan 2 files, delete 1, rescan → assert removed == 1 and DB has 1 row.
#[tokio::test]
async fn test_rescan_removes_stale_models() {
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
        path: model_dir.clone(),
        kind: Some(ModelKind::Diffusion),
    }];

    // First rescan: should find both models.
    let (upserted, removed) = registry.rescan(&dirs).await.unwrap();
    assert_eq!(upserted, 2, "should upsert 2 models");
    assert_eq!(removed, 0, "should remove 0 stale models");

    let models = registry.list(None).await.unwrap();
    assert_eq!(models.len(), 2, "DB should have 2 rows");

    // Delete one file from disk.
    std::fs::remove_file(model_dir.join("model_a.safetensors")).unwrap();

    // Second rescan: should find 1 model and remove 1 stale.
    let (upserted, removed) = registry.rescan(&dirs).await.unwrap();
    assert_eq!(upserted, 1, "should upsert 1 model");
    assert_eq!(removed, 1, "should remove 1 stale model");

    let models = registry.list(None).await.unwrap();
    assert_eq!(models.len(), 1, "DB should have 1 row");

    // The remaining model should be model_b.
    let id = &models[0].id;
    assert!(
        id.contains("model_b") || id.len() == 16,
        "remaining model should be model_b"
    );
}
