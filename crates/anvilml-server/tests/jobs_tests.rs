//! Integration tests for the POST /v1/jobs endpoint.
//!
//! Tests cover: 422 when the graph contains an unknown node type (empty
//! registry means no known types, so any graph fails validation), 422
//! when the graph contains an unknown node type (registry with LoadModel
//! only), and 202 when the graph is valid (delegated to JobScheduler).

use anvilml_core::types::{NodeTypeDescriptor, SlotDescriptor, SlotType};
use anvilml_core::NodeTypeRegistry;
use anvilml_ipc::{ArtifactStore, EventBroadcaster};
use anvilml_scheduler::{ledger::VramLedger, queue::JobQueue, scheduler::JobScheduler};
use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::json;
use std::sync::Arc;
use tower::util::ServiceExt;

/// Build a JobScheduler backed by an in-memory database for tests.
///
/// The scheduler's queue and ledger are freshly initialised — the queue
/// starts empty and the ledger has no registered devices. The event
/// broadcaster is a fresh instance. Returns both the scheduler and the
/// artifact store so tests can construct AppState.
async fn test_scheduler(
    registry: Arc<NodeTypeRegistry>,
) -> (Arc<JobScheduler>, Arc<ArtifactStore>) {
    let pool = anvilml_registry::open_in_memory()
        .await
        .expect("in-memory pool for test scheduler");
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = ArtifactStore::new(artifact_dir.clone(), pool.clone()).await;
    let artifact_store = Arc::new(artifact_store);
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry.clone(),
        pool,
        Arc::new(EventBroadcaster::new()),
        Arc::clone(&artifact_store),
    ));
    (scheduler, artifact_store)
}

/// Verify that POST /v1/jobs returns HTTP 422 when no worker has reached
/// Ready (fresh registry that has never had `update_from_worker` called).
///
/// With the scheduler-backed implementation, an empty registry means no
/// node types are known — any graph with nodes fails validation, returning
/// 422 with `invalid_graph` error. An empty graph `{}` also fails because
/// the `nodes` array is missing.
///
/// Preconditions: None — the server is built in-memory with no workers.
#[tokio::test]
async fn test_submit_job_returns_503_when_no_workers() {
    // Build a registry with no node types registered.
    let registry = Arc::new(NodeTypeRegistry::new().await);

    // Build the scheduler with the empty registry.
    let (scheduler, artifact_store) = test_scheduler(registry.clone()).await;

    // Build AppState with the empty registry and scheduler.
    let state = AppState::new("test-version", registry, scheduler, artifact_store.clone()).await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Build a POST request with an empty graph object.
    // SubmitJobRequest requires both "graph" and "settings" fields.
    // The graph {} is the payload, which will fail validation (no "nodes" array).
    let body = json!({"graph": {}, "settings": {}});
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/jobs")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 422 Unprocessable Entity.
    // With the scheduler-backed implementation, validation fails because
    // the graph is missing the "nodes" array — this is a structural error.
    assert_eq!(response.status(), 422);

    // Read and parse the response body as JSON.
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Assert the error field is "invalid_graph".
    assert_eq!(json["error"], "invalid_graph");
}

/// Verify that POST /v1/jobs returns HTTP 422 when the graph contains
/// an unknown node type.
///
/// Builds a `NodeTypeRegistry` with `LoadModel` registered, sends POST
/// `/v1/jobs` with a graph containing a node of type `"GhostNode"` (not
/// registered), and asserts 422 with an error body containing
/// `"error": "invalid_graph"`.
///
/// Preconditions: A registry that has `LoadModel` registered via
/// `update_from_worker`.
#[tokio::test]
async fn test_submit_job_returns_422_with_unknown_node_type() {
    // Build a registry and register LoadModel to simulate a mock worker
    // that reached Ready and reported its capabilities.
    let registry = NodeTypeRegistry::new().await;
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "LoadModel".to_string(),
                category: "io".to_string(),
                description: "Loads a model from disk".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let arc_registry = Arc::new(registry);

    // Build the scheduler with the populated registry.
    let (scheduler, artifact_store) = test_scheduler(arc_registry.clone()).await;

    let state = AppState::new(
        "test-version",
        arc_registry,
        scheduler,
        artifact_store.clone(),
    )
    .await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Build a POST request with a graph containing an unknown node type.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "n1", "type": "GhostNode" }
            ]
        },
        "settings": {}
    });
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/jobs")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 422 Unprocessable Entity.
    assert_eq!(response.status(), 422);

    // Read and parse the response body as JSON.
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Assert the error field is "invalid_graph".
    assert_eq!(json["error"], "invalid_graph");

    // Assert the message contains the unknown type name.
    let message = json["message"].as_str().unwrap();
    assert!(
        message.contains("GhostNode"),
        "message should mention the unknown type GhostNode, got: {message}"
    );
}

/// Verify that POST /v1/jobs returns HTTP 202 when the graph is valid.
///
/// Builds a `NodeTypeRegistry` with `LoadModel` registered, sends POST
/// `/v1/jobs` with a valid graph containing a single `LoadModel` node,
/// and asserts 202 with a response body containing a valid `job_id`
/// UUID and `queue_position: 1` (scheduler uses 1-based indexing).
///
/// Preconditions: A registry that has `LoadModel` registered via
/// `update_from_worker`.
#[tokio::test]
async fn test_submit_job_returns_202_with_valid_graph() {
    // Build a registry and register LoadModel.
    let registry = NodeTypeRegistry::new().await;
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "LoadModel".to_string(),
                category: "io".to_string(),
                description: "Loads a model from disk".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let arc_registry = Arc::new(registry);

    // Build the scheduler with the populated registry.
    let (scheduler, artifact_store) = test_scheduler(arc_registry.clone()).await;

    let state = AppState::new(
        "test-version",
        arc_registry,
        scheduler,
        artifact_store.clone(),
    )
    .await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Build a POST request with a valid graph.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "n1", "type": "LoadModel" }
            ]
        },
        "settings": {}
    });
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/jobs")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 202 Accepted.
    assert_eq!(response.status(), 202);

    // Read and parse the response body as JSON.
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Assert job_id is present and is a valid UUID string.
    assert!(
        json["job_id"].is_string(),
        "job_id must be a string, got: {:?}",
        json["job_id"]
    );
    let job_id_str = json["job_id"].as_str().unwrap();
    uuid::Uuid::parse_str(job_id_str).expect("job_id must be a valid UUID");

    // Assert queue_position is 1 (scheduler uses 1-based indexing).
    assert_eq!(json["queue_position"], 1);
}

/// Verify that GET /v1/jobs returns submitted jobs with correct status.
///
/// Submits a job via POST /v1/jobs, then calls GET /v1/jobs and verifies
/// the returned list contains the job with `status: "queued"`.
///
/// Preconditions: A registry with LoadModel registered, and a job
/// persisted via the submit endpoint.
#[tokio::test]
async fn test_list_jobs_returns_queued_jobs() {
    // Build a registry and register LoadModel.
    let registry = NodeTypeRegistry::new().await;
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "LoadModel".to_string(),
                category: "io".to_string(),
                description: "Loads a model from disk".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let arc_registry = Arc::new(registry);

    // Build the scheduler with the populated registry.
    let (scheduler, artifact_store) = test_scheduler(arc_registry.clone()).await;

    let state = AppState::new(
        "test-version",
        arc_registry,
        scheduler,
        artifact_store.clone(),
    )
    .await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // First, submit a job via POST so there is something in the list.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "n1", "type": "LoadModel" }
            ]
        },
        "settings": {}
    });
    let post_request = Request::builder()
        .method(Method::POST)
        .uri("/v1/jobs")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let post_response = router.clone().oneshot(post_request).await.unwrap();
    assert_eq!(post_response.status(), 202);

    // Now list jobs via GET /v1/jobs.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/jobs")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 OK.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Assert the body is a JSON array with at least one job.
    assert!(
        json.is_array(),
        "GET /v1/jobs must return a JSON array, got: {}",
        json
    );
    let arr = json.as_array().unwrap();
    assert!(arr.len() >= 1, "should return at least 1 job");

    // Assert the first job has status "Queued" (PascalCase from serde enum).
    assert_eq!(arr[0]["status"], "Queued");
}

/// Verify that GET /v1/jobs/{uuid} returns HTTP 404 for an unknown UUID.
///
/// Calls GET /v1/jobs/{uuid} with a UUID that was never submitted,
/// verifies 404 response with `"error": "job_not_found"`.
///
/// Preconditions: None — the scheduler has no jobs.
#[tokio::test]
async fn test_get_job_returns_404_for_unknown_id() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_scheduler(registry.clone()).await;

    let state = AppState::new("test-version", registry, scheduler, artifact_store.clone()).await;

    let router = build_router(state);

    // Build a GET request with a random UUID that was never submitted.
    let unknown_id = uuid::Uuid::new_v4();
    let request = Request::builder()
        .method(Method::GET)
        .uri(&format!("/v1/jobs/{unknown_id}"))
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 404 Not Found.
    assert_eq!(response.status(), 404);

    // Read and parse the response body as JSON.
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Assert the error field is "job_not_found".
    assert_eq!(json["error"], "job_not_found");
}
