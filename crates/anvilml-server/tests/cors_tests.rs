//! Integration tests for the CORS middleware layer.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket, mirroring
//! the pattern established in `health_tests.rs`.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{EnvReport, HardwareInfo, NodeTypeRegistry, ProvisioningState, ServerConfig};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::util::ServiceExt;

/// Helper to create an in-memory SQLite pool with migrations applied.
///
/// Duplicated from `health_tests.rs` rather than shared, per this crate's
/// existing test-file convention of self-contained integration test files.
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
async fn make_test_state(node_registry: Arc<NodeTypeRegistry>) -> AppState {
    let db = make_test_pool().await;
    let job_store = JobStore::new(db.clone());

    // Construct the artifact store before the scheduler so it can be passed
    // to JobScheduler::new().
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
        db,
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
    }
}

/// A simple cross-origin GET request (e.g. from AnvilML-TestUI on a
/// different port) receives an `access-control-allow-origin` header on
/// the actual response, not just on a preflight.
///
/// This is the exact failure mode reported against the TestUI: a plain
/// `GET /health` with an `Origin` header must come back with a CORS
/// header, or the browser blocks the response from reaching JS even
/// though the underlying HTTP request succeeded (status 200).
#[tokio::test]
async fn test_cors_header_present_on_simple_get_request() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    let req = Request::get("/health")
        .header(header::ORIGIN, "http://localhost:8848")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "response to a cross-origin GET must carry access-control-allow-origin"
    );
}

/// A CORS preflight `OPTIONS` request against a real route (`/v1/jobs`,
/// which only has GET/POST handlers registered — no explicit OPTIONS
/// handler exists anywhere in the router) is answered directly by the
/// CORS layer with a 2xx status and the three standard preflight
/// response headers, proving `CorsLayer` intercepts preflights ahead of
/// route matching rather than needing a registered OPTIONS handler.
#[tokio::test]
async fn test_cors_preflight_options_request_succeeds_without_route_handler() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    let req = Request::builder()
        .method("OPTIONS")
        .uri("/v1/jobs")
        .header(header::ORIGIN, "http://localhost:8848")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();

    assert!(
        res.status().is_success(),
        "preflight OPTIONS must succeed, got {}",
        res.status()
    );
    assert!(
        res.headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "preflight response must carry access-control-allow-origin"
    );
    assert!(
        res.headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_METHODS),
        "preflight response must carry access-control-allow-methods"
    );
}
