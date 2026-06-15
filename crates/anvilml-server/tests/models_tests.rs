//! Integration tests for the model metadata HTTP handlers.
//!
//! Tests cover: empty model list, kind-filtered listing, and 404 on
//! missing model ID. Each test uses an in-memory database via
//! `open_in_memory()` to ensure test isolation.

use anvilml_core::{ModelKind, ModelMeta};
use anvilml_registry::{open_in_memory, ModelStore};
use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use chrono::Utc;
use serde_json::Value;
use tower::util::ServiceExt;

/// Verify that GET /v1/models returns HTTP 200 with an empty JSON array
/// when the model registry contains zero models.
///
/// Uses `AppState::new()` which constructs an in-memory `ModelStore`
/// with no models in the database. Exercises the full `build_router`
/// pipeline including the model list handler.
#[tokio::test]
async fn test_list_models_empty() {
    // Build AppState with an empty in-memory database — the ModelStore
    // has no models, so the list endpoint should return [].
    let state = AppState::new("test-version").await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Dispatch a GET request to /v1/models.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/models")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 status.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the body is an empty JSON array.
    assert!(
        json.is_array(),
        "GET /v1/models must return a JSON array, got: {}",
        json
    );
    assert_eq!(json.as_array().unwrap().len(), 0);
}

/// Verify that GET /v1/models?kind=diffusion returns only diffusion models,
/// and that GET /v1/models?kind=vae returns an empty array when no VAE
/// models exist.
///
/// Inserts a single diffusion model into an in-memory database, then
/// exercises the kind filter on both the matching and non-matching kinds.
#[tokio::test]
async fn test_list_models_with_kind_filter() {
    // Open an in-memory database and construct a ModelStore.
    // Using a dedicated pool ensures this test doesn't share state with
    // any other test, since SQLite in-memory databases are connection-local.
    let pool = open_in_memory().await.unwrap();
    // Clone the pool before passing to ModelStore — we need the pool
    // itself to pass to AppState::new_with_hardware as well.
    let store = ModelStore::new(pool.clone()).await;

    // Insert a single diffusion model into the store.
    // This simulates what the model scanner would do after scanning a
    // directory containing a diffusion model file.
    let diffusion_model = ModelMeta {
        id: "diff-model-001".to_string(),
        name: "test-diffusion".to_string(),
        path: "/models/test-diffusion".to_string(),
        kind: ModelKind::Diffusion,
        dtype: anvilml_core::ModelDtype::Fp16,
        format: anvilml_core::ModelFormat::Safetensors,
        size_bytes: 1_000_000,
        scanned_at: Utc::now(),
    };
    store.upsert(&diffusion_model).await.unwrap();

    // Build AppState with the registry containing one diffusion model.
    // We use new_with_hardware to pass the pre-built ModelStore,
    // avoiding the sync/async boundary in the constructor.
    let hardware = std::sync::Arc::new(tokio::sync::RwLock::new(
        anvilml_core::types::HardwareInfo::default(),
    ));
    let state = AppState::new_with_hardware(
        "test-version",
        hardware,
        pool.clone(),
        std::sync::Arc::new(store),
    );

    let router = build_router(state);

    // --- Test kind=diffusion: should return the one model we inserted. ---

    // Build a GET request with ?kind=diffusion query parameter.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/models?kind=diffusion")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();

    assert_eq!(response.status(), 200);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(
        json.is_array(),
        "GET /v1/models?kind=diffusion must return a JSON array"
    );
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1, "should return exactly 1 diffusion model");
    assert_eq!(arr[0]["id"], "diff-model-001");

    // --- Test kind=vae: should return empty array since no VAE models exist. ---

    // Build a GET request with ?kind=vae query parameter.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/models?kind=vae")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), 200);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(
        json.is_array(),
        "GET /v1/models?kind=vae must return a JSON array"
    );
    assert_eq!(
        json.as_array().unwrap().len(),
        0,
        "should return empty array when no VAE models exist"
    );
}

/// Verify that GET /v1/models/:id returns HTTP 404 when the model ID
/// does not exist in the registry.
///
/// Uses an empty in-memory registry and requests a non-existent model ID.
/// The `AnvilError::ModelNotFound` variant maps to 404 via `IntoResponse`.
#[tokio::test]
async fn test_get_model_not_found() {
    let state = AppState::new("test-version").await;

    let router = build_router(state);

    // Build a GET request to a non-existent model ID.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/models/nonexistent-id")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 404 status.
    assert_eq!(response.status(), 404);

    // Read and parse the response body — it should be a JSON error object
    // with "error" set to "model_not_found".
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "model_not_found");
}
