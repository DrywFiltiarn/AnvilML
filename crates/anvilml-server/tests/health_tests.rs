//! Integration tests for the AnvilML server crate.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{NodeTypeRegistry, ServerConfig};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
use tower::util::ServiceExt;

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
/// fields are not used by the `/health` or `/v1/nodes` handlers, so
/// minimal construction is sufficient. The `node_registry` is shared
/// with the scheduler for consistency.
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

    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        artifact_store.clone(),
    ));
    let workers = Arc::new(
        WorkerPool::new()
            .await
            .expect("WorkerPool::new() must succeed in test"),
    );

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db,
        artifact_store,
        broadcaster: Arc::new(EventBroadcaster::new()),
    }
}

/// Verify that GET /health returns 200 OK with a JSON body containing
/// `status="ok"`, a string `version`, and a non-negative integer `uptime_s`.
///
/// Constructs a `GET /health` request, sends it through the router
/// built by `build_router()` with a captured `Instant`, and asserts
/// the response status is `StatusCode::OK` plus all three JSON fields
/// match the `ANVILML_DESIGN.md §13.4` contract.
#[tokio::test]
async fn test_health_returns_200() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/health").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    // Parse response body and assert on all three JSON fields.
    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
    let uptime = body["uptime_s"]
        .as_u64()
        .expect("uptime_s must be a non-negative integer");
    // u64 is always >= 0; the .as_u64() parse above confirms it's a valid integer.
    let _ = uptime;
}
