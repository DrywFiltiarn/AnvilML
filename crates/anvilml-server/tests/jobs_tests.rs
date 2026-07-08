//! Integration tests for the `POST /v1/jobs` handler.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, ServerConfig};
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use serde_json::json;
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
    let scheduler = Arc::new(JobScheduler::new(job_store, Arc::clone(&node_registry)));
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
    }
}

/// Verify that POST /v1/jobs with a valid graph and populated registry
/// returns 202 Accepted with a job_id (valid UUID) and queue_position 1.
///
/// Constructs an `AppState` with one registered node type, registers a
/// valid graph referencing that type, submits it via `POST /v1/jobs`,
/// and asserts the response status is `StatusCode::ACCEPTED` plus the
/// body contains a valid UUID `job_id` and `queue_position` of 1.
#[tokio::test]
async fn test_submit_job_valid_returns_202() {
    // Register a synthetic node type so the registry is non-empty and the
    // graph validator can resolve the node's type string.
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a valid graph referencing the registered node type.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::ACCEPTED);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // Assert job_id is a valid UUID string (36 characters, contains hyphens).
    let job_id = body["job_id"].as_str().expect("job_id must be a string");
    assert_eq!(job_id.len(), 36, "job_id must be a valid UUID v4 string");

    // Assert queue_position is 1 (first job in an empty queue).
    let queue_position = body["queue_position"]
        .as_u64()
        .expect("queue_position must be a non-negative integer");
    assert_eq!(queue_position, 1);
}

/// Verify that POST /v1/jobs with a malformed JSON body returns 400 Bad Request.
///
/// Sends a request with invalid JSON (`{not valid json}`) to the `/v1/jobs`
/// endpoint and asserts the response status is `StatusCode::BAD_REQUEST`.
/// The axum `Json` extractor returns a `Serde` error for malformed input,
/// which `AnvilError::IntoResponse` maps to HTTP 400.
#[tokio::test]
async fn test_submit_job_malformed_body_returns_400() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Send invalid JSON body.
    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from("{not valid json}"))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
}

/// Verify that POST /v1/jobs with an empty `NodeTypeRegistry` returns 503.
///
/// Constructs an `AppState` with an empty registry (no workers registered),
/// submits a structurally valid graph, and asserts the response status is
/// `StatusCode::SERVICE_UNAVAILABLE`. The scheduler's workers-available
/// guard rejects the submission before validation, returning
/// `AnvilError::WorkersUnavailable` which maps to HTTP 503.
#[tokio::test]
async fn test_submit_job_empty_registry_returns_503() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Submit a structurally valid graph, but the registry is empty so
    // the workers-available guard rejects it.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

/// Verify that POST /v1/jobs with an invalid graph (unknown node type)
/// returns 400 Bad Request.
///
/// Constructs an `AppState` with a populated registry, then submits a graph
/// that references a node type not present in the registry. The scheduler's
/// graph validator detects the unknown type and returns
/// `AnvilError::InvalidGraph`, which maps to HTTP 400.
#[tokio::test]
async fn test_submit_job_invalid_graph_returns_400() {
    // Register a node type so the registry is non-empty (avoiding the
    // workers-unavailable path) but the graph references a different,
    // unregistered type.
    let descriptor = NodeTypeDescriptor {
        type_name: "RegisteredNode".to_string(),
        display_name: "Registered Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a graph referencing "UnknownNode" which is NOT in the registry.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "UnknownNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
}
