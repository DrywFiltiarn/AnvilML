//! Integration tests for the `POST /v1/jobs` and `GET /v1/jobs` handlers.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, ServerConfig};
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
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
    // Construct a minimal ArtifactStore for tests — the artifact
    // directory is a temp path and the pool is in-memory, so no
    // real files are created.
    let artifact_store = Arc::new(ArtifactStore::new(
        std::env::temp_dir().join("anvilml-test-artifacts"),
        db.clone(),
    ));

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db,
        artifact_store,
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

/// Verify that GET /v1/jobs with no query params returns all submitted jobs.
///
/// Submits a job via POST, then calls GET /v1/jobs with no filters and asserts
/// the response status is `StatusCode::OK` and the returned array is non-empty.
#[tokio::test]
async fn test_list_jobs_no_filter_returns_all() {
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

    // Submit a job first so the list is non-empty.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    // Now list jobs with no filters.
    let get_req = Request::get("/v1/jobs").body(Body::empty()).unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let jobs = body.as_array().expect("response body must be a JSON array");
    assert!(
        !jobs.is_empty(),
        "list_jobs with no filter should return at least the submitted job"
    );
}

/// Verify that GET /v1/jobs?status=queued returns only jobs matching the filter.
///
/// Submits two jobs, then lists with `status=queued` and asserts only the
/// matching jobs are returned.
#[tokio::test]
async fn test_list_jobs_status_filter() {
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

    // Submit two jobs.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    for _ in 0..2 {
        let req = Request::post("/v1/jobs")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);
    }

    // List with status=queued filter — both jobs are queued.
    let get_req = Request::get("/v1/jobs?status=queued")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let jobs = body.as_array().expect("response body must be a JSON array");
    assert_eq!(jobs.len(), 2, "should return exactly 2 queued jobs");
}

/// Verify that GET /v1/jobs?limit=2 returns at most 2 jobs.
///
/// Submits three jobs, then lists with `limit=2` and asserts at most 2
/// jobs are returned.
#[tokio::test]
async fn test_list_jobs_limit() {
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

    // Submit three jobs.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    for _ in 0..3 {
        let req = Request::post("/v1/jobs")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);
    }

    // List with limit=2.
    let get_req = Request::get("/v1/jobs?limit=2")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let jobs = body.as_array().expect("response body must be a JSON array");
    assert!(
        jobs.len() <= 2,
        "limit=2 should return at most 2 jobs, got {}",
        jobs.len()
    );
}

/// Verify that GET /v1/jobs/:id on an existing job returns 200 with correct data.
///
/// Submits a job, extracts its ID from the POST response, then calls
/// GET /v1/jobs/:id and asserts the response is 200 with the correct job.
#[tokio::test]
async fn test_get_job_existing_returns_200() {
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

    // Submit a job.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    let body_bytes = to_bytes(post_res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let post_body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let job_id = post_body["job_id"]
        .as_str()
        .expect("job_id must be a string");
    let parsed_id = Uuid::parse_str(job_id).expect("job_id must be a valid UUID");

    // Now GET the job by ID.
    let get_req = Request::get(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // Assert the returned job has the same ID.
    assert_eq!(
        body["id"].as_str().map(|s| Uuid::parse_str(s).unwrap()),
        Some(parsed_id)
    );
}

/// Verify that GET /v1/jobs/:id on a non-existent UUID returns 404.
///
/// Calls GET /v1/jobs/:id with a UUID that was never submitted and asserts
/// the response status is `StatusCode::NOT_FOUND`.
#[tokio::test]
async fn test_get_job_unknown_returns_404() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Use a random UUID that was never submitted.
    let unknown_id = Uuid::new_v4();
    let get_req = Request::get(&format!("/v1/jobs/{}", unknown_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Verify that GET /v1/jobs accepts the `before` query parameter (forward-compat).
///
/// Submits a job, then lists with `before=<timestamp>` query parameter.
/// The handler accepts the parameter but does not pass it to the persistence
/// layer — this test asserts that the parameter does not cause a 400 error.
#[tokio::test]
async fn test_list_jobs_before_param_accepted() {
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

    // Submit a job.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    // List with before param — should return 200 (param accepted, ignored by persistence).
    let before_str = "2026-07-08T00:00:00Z";
    let get_req = Request::get(&format!("/v1/jobs?before={}", before_str))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
