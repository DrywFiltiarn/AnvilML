use anvilml_core::NodeTypeRegistry;
use anvilml_ipc::EventBroadcaster;
use anvilml_scheduler::{ledger::VramLedger, queue::JobQueue, scheduler::JobScheduler};
use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

/// Verify that the health handler returns HTTP 200 with a JSON body
/// containing a `status` key set to `"ok"`.
///
/// Exercises the production `build_router` path rather than duplicating
/// the routing logic inline. Uses `Router::oneshot` to exercise the full
/// handler pipeline (state extraction, handler execution, response
/// serialization) without binding a live TCP listener.
#[tokio::test]
async fn test_health_returns_200_with_status_key() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry.clone(),
        anvilml_registry::open_in_memory().await.unwrap(),
        Arc::new(EventBroadcaster::new()),
    ));
    let state = AppState::new("test-version", registry, scheduler).await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Build a GET request to /health.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(axum::body::Body::empty())
        .unwrap();

    // Dispatch the request through the router.
    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 status.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the `status` field is "ok".
    assert_eq!(json["status"], "ok");
}
