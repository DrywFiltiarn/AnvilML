use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anvilml_core::config::ModelDirConfig;
use anvilml_core::ModelKind;
use anvilml_registry::ModelRegistry;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use anvilml_server::{build_router, AppState};

/// Create a unique temporary directory structure for testing model scanning.
///
/// Creates a unique subdirectory under the system temp dir to avoid races
/// between concurrent tests. Returns `(diffusion_dir_path, db_file_path)`.
fn setup_test_env() -> (PathBuf, PathBuf) {
    let id = std::process::id();
    let temp_base = std::env::temp_dir().join(format!("anvilml_test_models_{id}"));
    let _ = fs::remove_dir_all(&temp_base); // clean up from previous runs
    fs::create_dir_all(temp_base.join("diffusion")).expect("create test dir");
    fs::File::create(temp_base.join("diffusion/model-fp16.safetensors"))
        .expect("create model file");

    let db_path = temp_base.join("test.db");
    // Pre-create the database file — `anvilml_registry::open` requires it.
    fs::File::create(&db_path).expect("pre-create db file");
    (temp_base.join("diffusion"), db_path)
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

    AppState::new("0.1.0", Some(pool), Some(registry), Some(dirs))
}

#[tokio::test]
async fn list_models_returns_scanned_models() {
    let (model_dir, db_path) = setup_test_env();
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
    let (model_dir, db_path) = setup_test_env();
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
    let (model_dir, db_path) = setup_test_env();
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
