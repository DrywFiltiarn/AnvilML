//! Integration tests for the `/v1/nodes` handler.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, ServerConfig};
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
/// minimal construction is sufficient.
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

/// Verify that GET /v1/nodes returns 200 OK with an empty JSON array
/// when the `NodeTypeRegistry` has no registered node types.
///
/// Constructs an `AppState` with an empty registry, builds the router,
/// sends a `GET /v1/nodes` request, and asserts the response status is
/// `StatusCode::OK` and the body is an empty JSON array `[]`.
#[tokio::test]
async fn test_nodes_empty_registry_returns_200_empty_array() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/v1/nodes").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    assert!(body.is_array(), "response body must be a JSON array");
    assert_eq!(body.as_array().unwrap().len(), 0);
}

/// Verify that GET /v1/nodes returns 200 OK with a JSON array containing
/// the correct `NodeTypeDescriptor` fields when the registry is populated.
///
/// Constructs an `AppState` with one registered `NodeTypeDescriptor`,
/// builds the router, sends a `GET /v1/nodes` request, and asserts
/// the response status is `StatusCode::OK` and the body contains the
/// registered descriptor with all expected fields.
#[tokio::test]
async fn test_nodes_populated_registry_returns_correct_shape() {
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
    let req = Request::get("/v1/nodes").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    assert!(body.is_array());
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["type_name"], "TestNode");
    assert_eq!(item["display_name"], "Test Node");
    assert_eq!(item["category"], "test");
    assert_eq!(item["description"], "A synthetic test node.");
    assert!(item["inputs"].is_array());
    assert!(item["outputs"].is_array());
}

/// Verify that the GET /v1/nodes response body is a JSON array (type check),
/// not an object or null.
///
/// Constructs an `AppState` with an empty registry, sends a `GET /v1/nodes`
/// request, and asserts that `serde_json::Value::is_array()` returns `true`.
/// This catches regressions where the handler accidentally returns an object
/// or null instead of an array.
#[tokio::test]
async fn test_nodes_response_is_array_not_object() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/v1/nodes").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // Assert the response is a JSON array, not an object or null.
    assert!(
        body.is_array(),
        "response must be a JSON array, got {:?}",
        body
    );
    assert!(!body.is_object(), "response must not be a JSON object");
    assert!(!body.is_null(), "response must not be null");
}

/// Verify that the health endpoint continues to work after `build_router()`
/// was refactored to accept `AppState` instead of `HealthState`.
///
/// Constructs an `AppState` with `start_time`, builds the router, and
/// sends a `GET /health` request. Asserts the response status is 200 and
/// the body contains `status="ok"`. This ensures the health handler
/// migration to `State<AppState>` did not break.
#[tokio::test]
async fn test_nodes_health_handler_still_works() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/health").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
    let _uptime = body["uptime_s"]
        .as_u64()
        .expect("uptime_s must be a non-negative integer");
}

/// Verify that multiple registered node descriptors are all returned
/// in the `/v1/nodes` response.
///
/// Constructs an `AppState` with three registered `NodeTypeDescriptor`
/// values, sends a `GET /v1/nodes` request, and asserts the response
/// array has length 3 with all three type names present.
#[tokio::test]
async fn test_nodes_multiple_descriptors_preserved() {
    let descriptors = vec![
        NodeTypeDescriptor {
            type_name: "LoadModel".to_string(),
            display_name: "Load Model".to_string(),
            category: "loaders".to_string(),
            description: "Loads a model checkpoint.".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        },
        NodeTypeDescriptor {
            type_name: "EncodePrompt".to_string(),
            display_name: "Encode Prompt".to_string(),
            category: "conditioning".to_string(),
            description: "Encodes a text prompt.".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        },
        NodeTypeDescriptor {
            type_name: "VAEDecode".to_string(),
            display_name: "VAE Decode".to_string(),
            category: "latent".to_string(),
            description: "Decodes a latent tensor to an image.".to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        },
    ];

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(descriptors);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);
    let req = Request::get("/v1/nodes").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let items = body.as_array().expect("response must be a JSON array");
    assert_eq!(items.len(), 3);

    // Collect all type names from the response and verify all three are present.
    let type_names: Vec<&str> = items
        .iter()
        .map(|item| item["type_name"].as_str().unwrap())
        .collect();
    assert!(type_names.contains(&"LoadModel"));
    assert!(type_names.contains(&"EncodePrompt"));
    assert!(type_names.contains(&"VAEDecode"));
}
