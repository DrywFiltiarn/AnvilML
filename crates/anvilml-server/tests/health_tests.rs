//! Integration tests for the AnvilML server crate.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_server::build_router;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use serde_json::Value;
use tower::util::ServiceExt;

/// Verify that GET /health returns 200 OK with a JSON body containing
/// `status="ok"`, a string `version`, and a non-negative integer `uptime_s`.
///
/// Constructs a `GET /health` request, sends it through the router
/// built by `build_router()` with a captured `Instant`, and asserts
/// the response status is `StatusCode::OK` plus all three JSON fields
/// match the `ANVILML_DESIGN.md §13.4` contract.
#[tokio::test]
async fn test_health_returns_200() {
    let start = std::time::Instant::now();
    let router = build_router(start);
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
