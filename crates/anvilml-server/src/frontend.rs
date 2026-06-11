//! Frontend serving module.
//!
//! Handles `FrontendMode::Local` by mounting a static file server with SPA fallback,
//! and returns the router unchanged for `Headless` and `Remote` modes.

use axum::{
    body::Body,
    http::{Request, Response},
    Router,
};

use anvilml_core::FrontendMode;
use tower_http::services::ServeDir;

/// Add a frontend route to the router based on the configured frontend mode.
///
/// - `Local { path }`: if the path exists, mount `ServeDir` with SPA fallback via
///   `fallback_service`. If missing, log a warning and mount an inline-HTML fallback.
/// - `Headless` / `Remote`: return the router unchanged.
pub fn add_frontend_route<S>(router: Router<S>, mode: &FrontendMode) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    match mode {
        FrontendMode::Local { path } => {
            if path.is_dir() {
                tracing::debug!(path = %path.display(), "mounting local frontend");
                let svc = ServeDir::new(path).fallback(tower_http::services::ServeFile::new(
                    path.join("index.html"),
                ));
                router.fallback_service(svc)
            } else {
                tracing::warn!(
                    path = %path.display(),
                    "frontend path {:?} not found, serving inline fallback",
                    path
                );
                let svc = tower::service_fn(|_req: Request<Body>| async {
                    Ok::<Response<Body>, std::convert::Infallible>(Response::new(Body::from(
                        "<h1>AnvilML</h1><p>Frontend not found. API at /v1/.</p>",
                    )))
                });
                router.fallback_service(svc)
            }
        }
        FrontendMode::Headless => router,
        FrontendMode::Remote { .. } => router,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use axum::{
        body::to_bytes,
        http::{Request, StatusCode},
    };
    use bytes::Bytes;
    use http_body_util::Full;
    use tower::ServiceExt;

    use super::*;

    /// Resolve the repo root from CARGO_MANIFEST_DIR (parent×2).
    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    /// Build an in-process axum app from a router with no state.
    fn build_app(router: Router<()>) -> Router {
        router.with_state(())
    }

    #[tokio::test]
    async fn test_frontend_local_serves_fixture() {
        let fixture_path = repo_root().join("test-frontend");
        assert!(
            fixture_path.is_dir(),
            "test fixture directory must exist at {:?}",
            fixture_path
        );

        let router = Router::new().route("/health", axum::routing::get(|| async { "ok" }));
        let router = add_frontend_route(
            router,
            &FrontendMode::Local {
                path: fixture_path.clone(),
            },
        );
        let app = build_app(router);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        assert!(
            body_str.contains("AnvilML Test Frontend"),
            "expected body to contain 'AnvilML Test Frontend', got: {}",
            body_str
        );
    }

    #[tokio::test]
    async fn test_frontend_local_missing_path() {
        let missing_path = PathBuf::from("/nonexistent/frontend/path");

        let router = Router::new().route("/health", axum::routing::get(|| async { "ok" }));
        let router = add_frontend_route(router, &FrontendMode::Local { path: missing_path });
        let app = build_app(router);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        assert!(
            body_str.contains("Frontend not found"),
            "expected body to contain 'Frontend not found', got: {}",
            body_str
        );
    }

    #[tokio::test]
    async fn test_frontend_headless() {
        let router = Router::new().route("/health", axum::routing::get(|| async { "ok" }));
        let router = add_frontend_route(router, &FrontendMode::Headless);
        let app = build_app(router);

        // Headless mode: no catch-all, so GET / returns 404.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        // Health endpoint still works.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        assert_eq!(body_str, "ok");
    }
}
