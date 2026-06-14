use anvilml_server::AppState;
use axum::body::to_bytes;
use axum::http::{Method, Request};
use axum::routing::get;
use axum::Router;
use serde_json::Value;
use tower::util::ServiceExt;

/// Verify that the health handler returns HTTP 200 with a JSON body
/// containing a `status` key set to `"ok"`.
///
/// Uses `Router::oneshot` to exercise the full handler pipeline
/// (state extraction, handler execution, response serialization)
/// without binding a live TCP listener.
#[tokio::test]
async fn test_health_returns_200_with_status_key() {
    let state = AppState::new("test-version");

    // Build a router with the health handler and shared state.
    let router = Router::new()
        // Import health via the crate's public re-export from lib.rs.
        .route("/health", get(anvilml_server::health))
        .with_state(state);

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
