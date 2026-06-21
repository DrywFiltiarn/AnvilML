//! Integration tests for the model metadata HTTP handlers.
//!
//! Tests cover: empty model list, kind-filtered listing, 404 on
//! missing model ID, and the rescan endpoint (POST /v1/models/rescan).
//! Each test uses an in-memory database via `open_in_memory()` to
//! ensure test isolation.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{ModelDirConfig, ModelKind, ModelMeta, NodeTypeRegistry};
use anvilml_registry::{open_in_memory, ModelStore};
use anvilml_scheduler::ledger::VramLedger;
use anvilml_scheduler::queue::JobQueue;
use anvilml_scheduler::scheduler::JobScheduler;
use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use chrono::Utc;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tower::util::ServiceExt;

/// Build a JobScheduler and ArtifactStore for tests.
async fn test_state(registry: Arc<NodeTypeRegistry>) -> (Arc<JobScheduler>, Arc<ArtifactStore>) {
    let pool = open_in_memory().await.unwrap();
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = Arc::new(ArtifactStore::new(artifact_dir, pool.clone()).await);
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry.clone(),
        pool,
        Arc::new(anvilml_ipc::EventBroadcaster::new()),
        Arc::clone(&artifact_store),
        None, // cancellation requires a real worker pool
    ));
    (scheduler, artifact_store)
}

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
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;

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
    let node_registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(node_registry.clone()).await;
    let state = AppState::new_with_hardware_no_workers(
        "test-version",
        hardware,
        pool.clone(),
        std::sync::Arc::new(store),
        Vec::new(),
        node_registry,
        scheduler,
        artifact_store.clone(),
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
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;

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

/// Verify that POST /v1/models/rescan returns HTTP 202 with
/// `{"status": "scanning"}` body, even when model_dirs is empty.
///
/// Uses `AppState::new()` which has an empty `model_dirs` vec.
/// The rescan handler should respond 202 immediately and spawn a
/// background task that scans zero directories.
#[tokio::test]
async fn test_rescan_returns_202() {
    // Build AppState with empty model_dirs (the default from AppState::new).
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;

    let router = build_router(state);

    // Dispatch a POST request to /v1/models/rescan.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/models/rescan")
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 202 Accepted status.
    assert_eq!(response.status(), 202);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the body contains status: "scanning".
    assert_eq!(json["status"], "scanning");
}

/// Verify that after POST /v1/models/rescan with model files on disk,
/// GET /v1/models returns the scanned models.
///
/// Creates a temporary directory containing a `.safetensors` file,
/// configures `AppState` with that directory, triggers a rescan,
/// then verifies the model appears in the list.
#[tokio::test]
async fn test_rescan_populates_registry() {
    // Create a temporary directory with a model file.
    // The TempDir guard ensures cleanup even on test panic.
    let tmp_dir = tempfile::tempdir().unwrap();
    let model_file = tmp_dir.path().join("test-model.safetensors");
    // Write minimal binary content — the scanner reads the first 1 MiB
    // for hashing, so any content works.
    std::fs::write(&model_file, b"test safetensors content").unwrap();

    // Build AppState with the temp directory as a model directory.
    // Using new_with_hardware to inject a custom model_dirs vec.
    let pool = open_in_memory().await.unwrap();
    let store = ModelStore::new(pool.clone()).await;
    let hardware = std::sync::Arc::new(tokio::sync::RwLock::new(
        anvilml_core::types::HardwareInfo::default(),
    ));
    let model_dirs = vec![ModelDirConfig {
        path: PathBuf::from(tmp_dir.path()),
        recursive: false,
        max_depth: None,
    }];
    let node_registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(node_registry.clone()).await;
    let state = AppState::new_with_hardware_no_workers(
        "test-version",
        hardware,
        pool,
        std::sync::Arc::new(store),
        model_dirs,
        node_registry,
        scheduler,
        artifact_store.clone(),
    );

    let router = build_router(state);

    // Trigger the rescan via POST.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/models/rescan")
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), 202);

    // Wait for the background scan task to complete.
    // The rescan of a single small file should be nearly instantaneous,
    // but we give a small grace period for the spawned task to finish.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Query the model list — the scanned model should appear.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/models")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json.is_array());
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1, "should find exactly 1 model after rescan");
    assert_eq!(arr[0]["name"], "test-model.safetensors");
    assert_eq!(arr[0]["kind"], "unknown");
}

/// Verify that scanned models have correct `kind` (from directory name)
/// and `dtype` (from filename).
///
/// Creates two temporary directories: one named `diffusion` with a
/// `model_fp8.safetensors` file, and one named `vae` with a
/// `model.safetensors` file. After rescan, verifies that:
/// - the diffusion model has kind=diffusion, dtype=fp8
/// - the vae model has kind=vae, dtype=unknown
#[tokio::test]
async fn test_rescan_infer_kind_and_dtype() {
    // Create a temp directory containing subdirectories for different
    // model kinds. Each subdirectory is a model_dir that the scanner
    // will walk. The scanner does not recurse, so we pass each subdir
    // as a separate ModelDirConfig entry.
    let tmp_dir = tempfile::tempdir().unwrap();

    // Create a diffusion model directory with an fp8 file.
    let diffusion_dir = tmp_dir.path().join("diffusion");
    std::fs::create_dir_all(&diffusion_dir).unwrap();
    std::fs::write(
        diffusion_dir.join("model_fp8.safetensors"),
        b"diffusion fp8 content",
    )
    .unwrap();

    // Create a vae model directory with a non-dtype file.
    let vae_dir = tmp_dir.path().join("vae");
    std::fs::create_dir_all(&vae_dir).unwrap();
    std::fs::write(vae_dir.join("model.safetensors"), b"vae content").unwrap();

    // Configure AppState with two model_dir entries — one per kind
    // subdirectory. This matches how the scanner works: it walks each
    // configured directory at its top level only (no recursion).
    let pool = open_in_memory().await.unwrap();
    let store = ModelStore::new(pool.clone()).await;
    let hardware = std::sync::Arc::new(tokio::sync::RwLock::new(
        anvilml_core::types::HardwareInfo::default(),
    ));
    let model_dirs = vec![
        ModelDirConfig {
            path: PathBuf::from(diffusion_dir),
            recursive: false,
            max_depth: None,
        },
        ModelDirConfig {
            path: PathBuf::from(vae_dir),
            recursive: false,
            max_depth: None,
        },
    ];
    let node_registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(node_registry.clone()).await;
    let state = AppState::new_with_hardware_no_workers(
        "test-version",
        hardware,
        pool,
        std::sync::Arc::new(store),
        model_dirs,
        node_registry,
        scheduler,
        artifact_store.clone(),
    );

    let router = build_router(state);

    // Trigger the rescan.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/models/rescan")
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), 202);

    // Wait for the background scan task.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Query the model list.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/models")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2, "should find exactly 2 models after rescan");

    // Find the diffusion model and verify kind + dtype.
    let diffusion = arr
        .iter()
        .find(|m| m["kind"] == "diffusion")
        .expect("should find a diffusion model");
    assert_eq!(diffusion["dtype"], "fp8");
    assert_eq!(diffusion["name"], "model_fp8.safetensors");

    // Find the vae model and verify kind + dtype.
    let vae = arr
        .iter()
        .find(|m| m["kind"] == "vae")
        .expect("should find a vae model");
    assert_eq!(vae["dtype"], "unknown");
    assert_eq!(vae["name"], "model.safetensors");
}
