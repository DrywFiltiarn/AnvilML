//! Integration tests for the AnvilML server crate.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_server::build_router;
use axum::body::Body;
use axum::http::Request;
use tower::util::ServiceExt;

/// Verify that GET /health returns 200 OK.
///
/// Constructs a `GET /health` request, sends it through the router
/// built by `build_router()`, and asserts the response status is
/// `StatusCode::OK`. This is the first end-to-end route test.
#[tokio::test]
async fn test_health_returns_200() {
    let router = build_router();
    let req = Request::get("/health").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
}
