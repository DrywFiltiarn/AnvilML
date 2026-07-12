//! Integration tests for the `GET /v1/models` and `GET /v1/models/:id` handlers.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    EnvReport, HardwareInfo, ModelDtype, ModelFormat, ModelKind, ModelMeta, NodeTypeRegistry,
    ProvisioningState, ServerConfig,
};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::{JobStore, ModelStore};
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use axum::http::StatusCode;
use chrono::Utc;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::util::ServiceExt;
use uuid::Uuid;

/// Helper to create an in-memory SQLite pool with migrations applied.
///
/// Creates a single-connection in-memory SQLite pool and runs all
/// migrations from the shared `database/migrations` directory.
async fn make_test_pool() -> sqlx::SqlitePool {
    let connect_opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&pool)
        .await
        .expect("migrations must apply to in-memory pool");

    pool
}

/// Construct a minimal `AppState` suitable for HTTP handler tests.
///
/// Creates stub values for `scheduler`, `workers`, and `db`, then
/// constructs a `ModelStore` from the same pool so that model
/// operations are backed by the in-memory database.
async fn make_test_state(node_registry: Arc<NodeTypeRegistry>) -> AppState {
    let db = make_test_pool().await;
    let job_store = JobStore::new(db.clone());

    // Construct the artifact store before the scheduler so it can be passed
    // to JobScheduler::new(). The artifact directory is a temp path and the
    // pool is in-memory, so no real files are created.
    let artifact_store = Arc::new(ArtifactStore::new(
        std::env::temp_dir().join("anvilml-test-artifacts"),
        db.clone(),
    ));

    let workers = Arc::new(
        WorkerPool::new()
            .await
            .expect("WorkerPool::new() must succeed in test"),
    );

    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        artifact_store.clone(),
        Arc::clone(&workers).transport().clone(),
    ));

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db: db.clone(),
        artifact_store,
        broadcaster: Arc::new(EventBroadcaster::new()),
        hardware: Arc::new(RwLock::new(HardwareInfo {
            host: anvilml_core::HostInfo {
                hostname: "test-host".to_string(),
                os: "Linux".to_string(),
            },
            gpus: vec![],
            inference_caps: anvilml_core::InferenceCaps::default(),
        })),
        env_report: Arc::new(RwLock::new(EnvReport {
            python_path: Some("./worker/.venv/bin/python3".to_string()),
            python_version: None,
            torch_version: None,
            provisioning: ProvisioningState::NotStarted,
            preflight_ok: false,
            reason: None,
            node_types: Vec::new(),
        })),
        model_store: Arc::new(ModelStore::new(db)),
    }
}

/// Verify that GET /v1/models with no kind filter returns all models.
///
/// Inserts two models of different kinds (diffusion and text_encoder) into
/// the model store, then calls GET /v1/models with no query parameters and
/// asserts the response status is `StatusCode::OK` with an array of length 2.
#[tokio::test]
async fn test_list_models_no_filter() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let state = make_test_state(node_registry).await;

    // Insert two models of different kinds.
    let model_store = state.model_store.clone();
    model_store
        .upsert(&ModelMeta {
            id: "diffusion-model-1".to_string(),
            name: "diffusion-v1".to_string(),
            path: PathBuf::from("/models/diffusion-v1.safetensors"),
            kind: ModelKind::Diffusion,
            dtype: ModelDtype::Fp16,
            format: ModelFormat::Safetensors,
            size_bytes: 1_000_000,
            mtime_unix: 1_700_000_000,
            scanned_at: Utc::now(),
        })
        .await
        .expect("upsert must succeed");

    model_store
        .upsert(&ModelMeta {
            id: "text-encoder-1".to_string(),
            name: "clip-vit".to_string(),
            path: PathBuf::from("/models/clip-vit.safetensors"),
            kind: ModelKind::TextEncoder,
            dtype: ModelDtype::Fp32,
            format: ModelFormat::Safetensors,
            size_bytes: 500_000,
            mtime_unix: 1_700_000_000,
            scanned_at: Utc::now(),
        })
        .await
        .expect("upsert must succeed");

    let router = build_router(state);

    // Call GET /v1/models with no query params.
    let req = Request::get("/v1/models").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let models = body.as_array().expect("response body must be a JSON array");
    assert_eq!(models.len(), 2, "should return exactly 2 models");
}

/// Verify that GET /v1/models?kind=diffusion returns only diffusion models.
///
/// Inserts three models (two diffusion, one VAE), then calls
/// GET /v1/models?kind=diffusion and asserts the response returns
/// exactly 2 diffusion models.
#[tokio::test]
async fn test_list_models_kind_filter() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let state = make_test_state(node_registry).await;

    let model_store = state.model_store.clone();

    // Insert 2 diffusion models.
    for i in 0..2 {
        model_store
            .upsert(&ModelMeta {
                id: format!("diffusion-model-{i}"),
                name: format!("diffusion-v{i}"),
                path: PathBuf::from(format!("/models/diffusion-v{i}.safetensors")),
                kind: ModelKind::Diffusion,
                dtype: ModelDtype::Fp16,
                format: ModelFormat::Safetensors,
                size_bytes: 1_000_000,
                mtime_unix: 1_700_000_000,
                scanned_at: Utc::now(),
            })
            .await
            .expect("upsert must succeed");
    }

    // Insert 1 VAE model.
    model_store
        .upsert(&ModelMeta {
            id: "vae-model-1".to_string(),
            name: "vae-v1".to_string(),
            path: PathBuf::from("/models/vae-v1.safetensors"),
            kind: ModelKind::Vae,
            dtype: ModelDtype::Fp16,
            format: ModelFormat::Safetensors,
            size_bytes: 200_000,
            mtime_unix: 1_700_000_000,
            scanned_at: Utc::now(),
        })
        .await
        .expect("upsert must succeed");

    let router = build_router(state);

    // Call GET /v1/models?kind=diffusion.
    let req = Request::get("/v1/models?kind=diffusion")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let models = body.as_array().expect("response body must be a JSON array");
    assert_eq!(models.len(), 2, "should return exactly 2 diffusion models");
}

/// Verify that GET /v1/models/:id returns the correct model for an existing ID.
///
/// Inserts one model into the store, then calls GET /v1/models/{id} and
/// asserts the response is 200 with the correct id, name, and kind.
#[tokio::test]
async fn test_get_model_existing_returns_200() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let state = make_test_state(node_registry).await;

    let model_id = "test-model-123".to_string();
    let model_store = state.model_store.clone();

    model_store
        .upsert(&ModelMeta {
            id: model_id.clone(),
            name: "test-model".to_string(),
            path: PathBuf::from("/models/test-model.safetensors"),
            kind: ModelKind::Diffusion,
            dtype: ModelDtype::Bf16,
            format: ModelFormat::Safetensors,
            size_bytes: 2_000_000,
            mtime_unix: 1_700_000_000,
            scanned_at: Utc::now(),
        })
        .await
        .expect("upsert must succeed");

    let router = build_router(state);

    let req = Request::get(&format!("/v1/models/{model_id}"))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // Assert the returned model has the correct id, name, and kind.
    assert_eq!(body["id"].as_str().expect("id must be a string"), model_id);
    assert_eq!(
        body["name"].as_str().expect("name must be a string"),
        "test-model"
    );
    assert_eq!(
        body["kind"].as_str().expect("kind must be a string"),
        "diffusion"
    );
}

/// Verify that GET /v1/models/:id returns 404 for a non-existent ID.
///
/// Calls GET /v1/models/{random-uuid} with a UUID that was never
/// inserted and asserts the response status is `StatusCode::NOT_FOUND`.
#[tokio::test]
async fn test_get_model_unknown_returns_404() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let state = make_test_state(node_registry).await;

    let router = build_router(state);

    // Use a random UUID that was never inserted.
    let unknown_id = Uuid::new_v4().to_string();
    let req = Request::get(&format!("/v1/models/{unknown_id}"))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
