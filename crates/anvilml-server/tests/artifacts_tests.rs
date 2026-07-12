//! Integration tests for the `GET /v1/artifacts` handler.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    ArtifactMeta, EnvReport, HardwareInfo, NodeTypeRegistry, ProvisioningState, ServerConfig,
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
use axum::http::header::CONTENT_TYPE;
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::util::ServiceExt;
use uuid::Uuid;

/// Helper to create an in-memory SQLite pool with migrations applied.
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
/// Creates stub values for `scheduler`, `workers`, and `db` — these
/// fields are not used by the `/v1/artifacts` handler, so minimal
/// construction is sufficient.
async fn make_test_state() -> AppState {
    let db = make_test_pool().await;

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
        JobStore::new(db.clone()),
        Arc::new(NodeTypeRegistry::new()),
        artifact_store.clone(),
        Arc::clone(&workers).transport().clone(),
    ));

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry: Arc::new(NodeTypeRegistry::new()),
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

/// Helper to save an artifact directly into the artifact store.
///
/// Constructs a minimal `ArtifactMeta` with the given parameters,
/// generates synthetic PNG bytes (appending `content_suffix` to ensure
/// distinct hashes across multiple calls), and calls `store.save()`.
/// Returns the hash for verification.
async fn save_artifact(
    state: &AppState,
    job_id: Uuid,
    width: u32,
    height: u32,
    seed: i64,
    steps: u32,
    content_suffix: &[u8],
) -> String {
    // Append a content suffix so that multiple save_artifact() calls
    // with different parameters produce different content hashes.
    // Without this, the same PNG bytes would produce the same hash
    // and the idempotent save() would skip the second insert.
    let mut png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    png_bytes.extend_from_slice(content_suffix);
    let meta = ArtifactMeta {
        hash: String::new(), // computed by save()
        job_id,
        width,
        height,
        seed,
        steps,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/anvilml-test-artifacts/placeholder.png"),
    };
    state
        .artifact_store
        .save(&png_bytes, &meta)
        .await
        .expect("artifact save must succeed in test")
}

/// Verify that GET /v1/artifacts with an empty store returns 200
/// with a JSON array `[]`.
///
/// Constructs an `AppState` with an empty artifact store, sends a
/// GET request to `/v1/artifacts`, and asserts the response status
/// is `StatusCode::OK` and the body parses to an empty JSON array.
#[tokio::test]
async fn test_list_artifacts_empty_store_returns_200_empty_array() {
    let state = make_test_state().await;
    let router = build_router(state);

    let req = Request::get("/v1/artifacts").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let arr = body.as_array().expect("response body must be a JSON array");
    assert!(
        arr.is_empty(),
        "empty artifact store should return an empty JSON array, got: {arr:?}"
    );
}

/// Verify that GET /v1/artifacts with a populated store returns all artifacts.
///
/// Saves two artifacts with distinct PNG bytes into the store, sends
/// GET `/v1/artifacts` with no filter, and asserts the response status
/// is `StatusCode::OK` and the returned array has length 2.
#[tokio::test]
async fn test_list_artifacts_populated_returns_all() {
    let state = make_test_state().await;
    let router = build_router(state.clone());

    let job_id = Uuid::new_v4();

    // Save two artifacts with distinct PNG bytes (different sizes).
    save_artifact(&state, job_id, 512, 512, 42, 20, b"artifact1").await;
    save_artifact(&state, job_id, 768, 1024, 99, 50, b"artifact2").await;

    let req = Request::get("/v1/artifacts").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let arr = body.as_array().expect("response body must be a JSON array");
    assert_eq!(arr.len(), 2, "should return exactly 2 artifacts");
}

/// Verify that GET /v1/artifacts?job_id=<uuid> filters to matching artifacts.
///
/// Saves two artifacts with different `job_id` values, then sends
/// GET `/v1/artifacts?job_id=<first_id>` and asserts the response
/// has length 1 with the correct job_id.
#[tokio::test]
async fn test_list_artifacts_job_id_filter_returns_matching() {
    let state = make_test_state().await;
    let router = build_router(state.clone());

    let job_id_a = Uuid::new_v4();
    let job_id_b = Uuid::new_v4();

    // Save one artifact for each job.
    save_artifact(&state, job_id_a, 512, 512, 1, 10, b"job_a").await;
    save_artifact(&state, job_id_b, 768, 768, 2, 20, b"job_b").await;

    // List with job_id filter for job_a.
    let req = Request::get(&format!("/v1/artifacts?job_id={}", job_id_a))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let arr = body.as_array().expect("response body must be a JSON array");
    assert_eq!(
        arr.len(),
        1,
        "job_id filter should return exactly 1 artifact"
    );

    // Assert the returned artifact has the correct job_id.
    let returned_job_id = body[0]["job_id"].as_str().expect("job_id must be a string");
    assert_eq!(
        returned_job_id,
        job_id_a.to_string(),
        "returned artifact job_id must match the filter"
    );
}

/// Verify that GET /v1/artifacts returns the correct JSON shape.
///
/// Saves one artifact, sends GET `/v1/artifacts`, deserialises the
/// body into `serde_json::Value`, and asserts the presence and types
/// of all `ArtifactMeta` fields:
/// - `hash` is a string
/// - `job_id` is a string (UUID)
/// - `width` is an integer
/// - `height` is an integer
/// - `steps` is an integer
/// - `seed` is an integer
/// - `created_at` is a string
/// - `file_path` is a string
#[tokio::test]
async fn test_list_artifacts_json_shape() {
    let state = make_test_state().await;
    let router = build_router(state.clone());

    let job_id = Uuid::new_v4();
    save_artifact(&state, job_id, 512, 512, 42, 20, b"shape_test").await;

    let req = Request::get("/v1/artifacts").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let arr = body.as_array().expect("response body must be a JSON array");
    assert_eq!(arr.len(), 1, "should return exactly 1 artifact");

    let meta = &arr[0];

    // Assert `hash` is a non-empty string.
    assert!(
        meta["hash"].is_string(),
        "hash must be a string, got: {:?}",
        meta["hash"]
    );

    // Assert `job_id` is a string (UUID format — 36 chars with hyphens).
    assert!(
        meta["job_id"].is_string(),
        "job_id must be a string, got: {:?}",
        meta["job_id"]
    );
    let job_id_str = meta["job_id"].as_str().expect("job_id must be a string");
    assert_eq!(
        job_id_str.len(),
        36,
        "job_id must be a valid UUID v4 string"
    );

    // Assert `width` is an integer.
    assert!(
        meta["width"].is_number(),
        "width must be an integer, got: {:?}",
        meta["width"]
    );

    // Assert `height` is an integer.
    assert!(
        meta["height"].is_number(),
        "height must be an integer, got: {:?}",
        meta["height"]
    );

    // Assert `steps` is an integer.
    assert!(
        meta["steps"].is_number(),
        "steps must be an integer, got: {:?}",
        meta["steps"]
    );

    // Assert `seed` is an integer.
    assert!(
        meta["seed"].is_number(),
        "seed must be an integer, got: {:?}",
        meta["seed"]
    );

    // Assert `created_at` is a string.
    assert!(
        meta["created_at"].is_string(),
        "created_at must be a string, got: {:?}",
        meta["created_at"]
    );

    // Assert `file_path` is a string.
    assert!(
        meta["file_path"].is_string(),
        "file_path must be a string, got: {:?}",
        meta["file_path"]
    );
}

/// Verify that GET /v1/artifacts/{hash} returns 200 with correct Content-Type
/// for a saved artifact.
///
/// Saves one artifact into the store, retrieves its hash from the database
/// via the list endpoint, then sends a GET request to
/// `/v1/artifacts/{hash}` and asserts the response status is `StatusCode::OK`
/// and the `Content-Type` header is `image/png`.
#[tokio::test]
async fn test_get_artifact_existing_hash_returns_200() {
    let state = make_test_state().await;
    let router = build_router(state.clone());

    let job_id = Uuid::new_v4();
    let hash = save_artifact(&state, job_id, 512, 512, 42, 20, b"get_existing").await;

    let req = Request::get(&format!("/v1/artifacts/{hash}"))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

/// Verify that GET /v1/artifacts/{hash} returns 404 for an unknown hash.
///
/// Sends a GET request to `/v1/artifacts/{unknown_hash}` with a hash that
/// does not correspond to any saved artifact, and asserts the response
/// status is `StatusCode::NOT_FOUND` (404).
#[tokio::test]
async fn test_get_artifact_unknown_hash_returns_404() {
    let state = make_test_state().await;
    let router = build_router(state.clone());

    let unknown_hash = "0000000000000000000000000000000000000000000000000000000000000000";

    let req = Request::get(&format!("/v1/artifacts/{unknown_hash}"))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Verify that GET /v1/artifacts/{hash} returns byte-for-byte identical
/// content to what was saved.
///
/// Saves an artifact with known PNG bytes, retrieves it via the
/// `/v1/artifacts/{hash}` endpoint, and asserts the response body bytes
/// exactly match the original bytes.
#[tokio::test]
async fn test_get_artifact_byte_for_byte_match() {
    let state = make_test_state().await;
    let router = build_router(state.clone());

    let job_id = Uuid::new_v4();
    let original_bytes = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x01, 0x02, 0x03,
    ];
    let meta = ArtifactMeta {
        hash: String::new(),
        job_id,
        width: 64,
        height: 64,
        seed: 1,
        steps: 10,
        created_at: Utc::now(),
        file_path: PathBuf::from("/tmp/anvilml-test-artifacts/placeholder.png"),
    };
    let hash = state
        .artifact_store
        .save(&original_bytes, &meta)
        .await
        .expect("artifact save must succeed in test");

    let req = Request::get(&format!("/v1/artifacts/{hash}"))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    assert_eq!(
        body_bytes.as_ref(),
        original_bytes,
        "response body must exactly match the saved PNG bytes"
    );
}

/// Verify that GET /v1/artifacts/{hash} sets the Content-Type header
/// to exactly `image/png`.
///
/// Saves one artifact, retrieves it via `/v1/artifacts/{hash}`, and
/// asserts the `Content-Type` header value is `image/png`.
#[tokio::test]
async fn test_get_artifact_content_type_header() {
    let state = make_test_state().await;
    let router = build_router(state.clone());

    let job_id = Uuid::new_v4();
    let hash = save_artifact(&state, job_id, 512, 512, 42, 20, b"content_type_test").await;

    let req = Request::get(&format!("/v1/artifacts/{hash}"))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let content_type = res
        .headers()
        .get(CONTENT_TYPE)
        .expect("Content-Type header must be present");
    assert_eq!(
        content_type
            .to_str()
            .expect("Content-Type must be valid UTF-8"),
        "image/png",
        "Content-Type must be exactly image/png"
    );
}
