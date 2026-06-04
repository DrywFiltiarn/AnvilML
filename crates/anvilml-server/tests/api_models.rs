use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use anvilml_core::config::ModelDirConfig;
use anvilml_core::ModelKind;
use anvilml_registry::ModelRegistry;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use anvilml_server::{build_router, AppState, EventBroadcaster};

/// Create a unique temporary directory structure for testing model scanning.
///
/// Each call creates its own `TempDir` (OS-managed, under `/tmp`) so that
/// parallel tests never share files. Returns `(temp_dir_guard, diffusion_dir_path, db_file_path)`.
fn setup_test_env() -> (TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::TempDir::new().expect("create temp dir");
    let diffusion_dir = tmp.path().join("diffusion");
    let db_path = tmp.path().join("test.db");

    fs::create_dir_all(&diffusion_dir).expect("create test dir");
    fs::File::create(diffusion_dir.join("model-fp16.safetensors")).expect("create model file");

    // Pre-create the database file — `anvilml_registry::open` requires it.
    fs::File::create(&db_path).expect("pre-create db file");
    (tmp, diffusion_dir, db_path)
}

/// Build an `AppState` with a fresh registry backed by a file-based SQLite
/// database that has been initialized with migrations, and the given model
/// directory configured for rescan.
async fn build_test_app_state(model_dir: PathBuf, db_path: PathBuf) -> AppState {
    let pool = anvilml_registry::open(&db_path)
        .await
        .expect("open db must succeed");
    let registry = Arc::new(ModelRegistry::new(pool.clone()));

    let dirs = vec![ModelDirConfig {
        path: model_dir,
        kind: Some(ModelKind::Diffusion),
    }];
    registry.rescan(&dirs).await.expect("rescan must succeed");

    let broadcaster = Arc::new(EventBroadcaster::new(16));
    AppState::new("0.1.0", Some(pool), Some(registry), Some(dirs), broadcaster)
}

#[tokio::test]
async fn list_models_returns_scanned_models() {
    let (_tmp, model_dir, db_path) = setup_test_env();
    let state = build_test_app_state(model_dir, db_path).await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();

    assert!(parsed.is_array(), "response body must be a JSON array");
    let models = parsed.as_array().unwrap();
    assert_eq!(models.len(), 1, "must have exactly one scanned model");

    let model = &models[0];
    assert_eq!(model["name"], "model-fp16");
    assert_eq!(model["kind"], "diffusion");
    assert_eq!(model["dtype_hint"], "f16");
}

#[tokio::test]
async fn list_models_kind_filter_diffusion() {
    let (_tmp, model_dir, db_path) = setup_test_env();
    let state = build_test_app_state(model_dir, db_path).await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models?kind=diffusion")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();

    assert!(parsed.is_array());
    let models = parsed.as_array().unwrap();
    assert_eq!(
        models.len(),
        1,
        "kind=diffusion must return one diffusion model"
    );

    let model = &models[0];
    assert_eq!(model["name"], "model-fp16");
    assert_eq!(model["kind"], "diffusion");
}

#[tokio::test]
async fn list_models_kind_filter_no_match() {
    let (_tmp, model_dir, db_path) = setup_test_env();
    let state = build_test_app_state(model_dir, db_path).await;
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models?kind=vae")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();

    assert!(parsed.is_array());
    let models = parsed.as_array().unwrap();
    assert!(
        models.is_empty(),
        "kind=vae must return empty array when no VAE models exist"
    );
}
