//! Integration tests for the GET /v1/nodes endpoint.
//!
//! Tests cover: 503 when no worker has reached Ready, and 200 with
//! an empty array after a mock worker reaches Ready.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::NodeTypeRegistry;
use anvilml_scheduler::scheduler::JobScheduler;
use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

/// Build a JobScheduler and ArtifactStore for tests.
async fn test_state(registry: Arc<NodeTypeRegistry>) -> (Arc<JobScheduler>, Arc<ArtifactStore>) {
    let pool = anvilml_registry::open_in_memory().await.unwrap();
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = Arc::new(ArtifactStore::new(artifact_dir, pool.clone()).await);
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(
            anvilml_scheduler::queue::JobQueue::default(),
        )),
        Arc::new(tokio::sync::Mutex::new(
            anvilml_scheduler::ledger::VramLedger::new(),
        )),
        registry.clone(),
        pool,
        Arc::new(anvilml_ipc::EventBroadcaster::new()),
        Arc::clone(&artifact_store),
        None, // cancellation requires a real worker pool
    ));
    (scheduler, artifact_store)
}

/// Verify that GET /v1/nodes returns HTTP 503 when no worker has reached
/// Ready (fresh registry that has never had `update_from_worker` called).
///
/// Builds `AppState` with a freshly constructed `NodeTypeRegistry` that
/// has never been updated, sends GET `/v1/nodes`, and asserts 503 with
/// an error body containing `"error": "workers_unavailable"`.
///
/// Preconditions: None — the server is built in-memory with no workers.
#[tokio::test]
async fn test_nodes_returns_503_when_registry_not_updated() {
    // Build AppState with a fresh, never-updated registry.
    // The registry's `updated` flag is false, so the handler should
    // return 503.
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Dispatch a GET request to /v1/nodes.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/nodes")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 503 Service Unavailable.
    assert_eq!(response.status(), 503);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the error field is "workers_unavailable".
    assert_eq!(json["error"], "workers_unavailable");

    // Assert the message contains the expected text.
    assert!(
        json["message"]
            .as_str()
            .unwrap()
            .contains("no worker has reached Ready"),
        "message should mention no worker reached Ready, got: {}",
        json["message"]
    );
}

/// Verify that GET /v1/nodes returns HTTP 200 with `[]` after a mock
/// worker reaches Ready (empty node_types list).
///
/// Builds a `NodeTypeRegistry`, calls `update_from_worker("worker-0", vec![])`
/// on it (simulating a mock worker that reached Ready but has no node types),
/// wraps it in `Arc`, passes it to `AppState::new`, sends GET `/v1/nodes`,
/// and asserts 200 with an empty array.
///
/// Preconditions: A registry that has been updated via `update_from_worker`.
#[tokio::test]
async fn test_nodes_returns_200_after_worker_ready() {
    // Build a registry and simulate a mock worker reaching Ready.
    // A mock worker's Ready event reports zero node types, but the
    // registry's updated flag is still set to true — this is the
    // distinction that 503-vs-200 logic depends on.
    let registry = NodeTypeRegistry::new().await;
    registry.update_from_worker("worker-0", vec![]).await;

    // Build AppState with the updated registry and a scheduler.
    let arc_registry = Arc::new(registry);
    let (scheduler, artifact_store) = test_state(arc_registry.clone()).await;
    let state = AppState::new("test-version", arc_registry, scheduler, artifact_store).await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Dispatch a GET request to /v1/nodes.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/nodes")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 OK.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the body is an empty JSON array.
    assert!(
        json.is_array(),
        "GET /v1/nodes must return a JSON array, got: {}",
        json
    );
    assert_eq!(
        json.as_array().unwrap().len(),
        0,
        "mock worker with zero node types should return empty array"
    );
}
