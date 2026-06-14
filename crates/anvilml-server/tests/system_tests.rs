use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::Value;
use tower::util::ServiceExt;

/// Verify that the system env handler returns HTTP 200 with a JSON body
/// containing the default `EnvReport` values: `preflight_ok` is `false`
/// and `provisioning` is `"not_started"`.
///
/// Exercises the production `build_router` path rather than duplicating
/// the routing logic inline. Uses `Router::oneshot` to exercise the full
/// handler pipeline (state extraction, handler execution, response
/// serialization) without binding a live TCP listener.
#[tokio::test]
async fn test_system_env_returns_200_with_default_report() {
    let state = AppState::new("test-version");

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Build a GET request to /v1/system/env.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/system/env")
        .body(axum::body::Body::empty())
        .unwrap();

    // Dispatch the request through the router.
    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 status.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the `preflight_ok` field is false (default stub value).
    assert_eq!(json["preflight_ok"], false);

    // Assert the `provisioning` field is "not_started" (default stub value).
    assert_eq!(json["provisioning"], "not_started");
}
