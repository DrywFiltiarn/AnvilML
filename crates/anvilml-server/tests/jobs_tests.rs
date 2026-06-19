//! Integration tests for the POST /v1/jobs endpoint.
//!
//! Tests cover: 503 when no workers have reached Ready, 422 when the
//! graph contains an unknown node type, and 202 when the graph is valid.

use anvilml_core::types::{NodeTypeDescriptor, SlotDescriptor, SlotType};
use anvilml_core::NodeTypeRegistry;
use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::json;
use std::sync::Arc;
use tower::util::ServiceExt;

/// Verify that POST /v1/jobs returns HTTP 503 when no worker has reached
/// Ready (fresh registry that has never had `update_from_worker` called).
///
/// Builds `AppState` with a freshly constructed `NodeTypeRegistry` that
/// has never been updated, sends POST `/v1/jobs` with an empty graph body,
/// and asserts 503 with an error body containing `"error": "workers_unavailable"`.
///
/// Preconditions: None — the server is built in-memory with no workers.
#[tokio::test]
async fn test_submit_job_returns_503_when_no_workers() {
    // Build AppState with a fresh, never-updated registry.
    // The registry's `updated` flag is false and types map is empty,
    // so `is_empty()` returns true and the handler returns 503.
    let state = AppState::new("test-version", Arc::new(NodeTypeRegistry::new().await)).await;

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

    // Assert HTTP 503 Service Unavailable.
    assert_eq!(response.status(), 503);

    // Read and parse the response body as JSON.
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Assert the error field is "workers_unavailable".
    assert_eq!(json["error"], "workers_unavailable");
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

    let state = AppState::new("test-version", Arc::new(registry)).await;

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
/// UUID and `queue_position: 0`.
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

    let state = AppState::new("test-version", Arc::new(registry)).await;

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

    // Assert queue_position is 0 (placeholder value).
    assert_eq!(json["queue_position"], 0);
}
