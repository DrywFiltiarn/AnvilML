//! Frontend serving module.
//!
//! Handles `FrontendMode::Local` by mounting a static file server with SPA fallback,
//! and `FrontendMode::Remote` by mounting a reverse-proxy catch-all.
//! `Headless` mode returns the router unchanged.

use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    Router,
};

use anvilml_core::FrontendMode;
use http_body_util::BodyExt;
use tower_http::services::ServeDir;

/// Hop-by-hop header names to strip from upstream responses.
///
/// Per RFC 7230 §6.1, these headers must not be forwarded.
const HOP_BY_HOP: [&str; 6] = [
    "connection",
    "keep-alive",
    "transfer-encoding",
    "te",
    "trailers",
    "upgrade",
];

/// Add a frontend route to the router based on the configured frontend mode.
///
/// - `Local { path }`: if the path exists, mount `ServeDir` with SPA fallback via
///   `fallback_service`. If missing, log a warning and mount an inline-HTML fallback.
/// - `Headless`: return the router unchanged.
/// - `Remote { url }`: mount a catch-all reverse-proxy handler via `nest_service`.
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
        FrontendMode::Remote { url } => {
            tracing::debug!(url = %url, "mounting remote frontend proxy");

            let client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build(hyper_util::client::legacy::connect::HttpConnector::new());

            let url = url.clone();

            let svc = tower::service_fn(move |req: Request<Body>| {
                let client = client.clone();
                let url = url.clone();
                proxy_handler(req, client, url)
            });

            router.fallback_service(svc)
        }
    }
}

/// Reverse-proxy handler: forwards the request to the upstream URL and streams
/// the response back to the client.
///
/// Returns `Infallible` as the error type because `nest_service` requires it.
/// All errors are converted to 502 Bad Gateway responses internally.
async fn proxy_handler(
    req: Request<Body>,
    client: hyper_util::client::legacy::Client<
        hyper_util::client::legacy::connect::HttpConnector,
        Body,
    >,
    url: url::Url,
) -> Result<Response<Body>, std::convert::Infallible> {
    // Build the upstream URI by joining the request path onto the base URL.
    let upstream_uri = match req.uri().path_and_query() {
        Some(pq) => {
            let path = pq.as_str();
            match url.join(path) {
                Ok(u) => match u.as_str().parse::<http::Uri>() {
                    Ok(uri) => uri,
                    Err(e) => {
                        tracing::warn!(error = %e, upstream = %url, path = %path, "failed to parse upstream URI");
                        return Ok(Response::builder()
                            .status(StatusCode::BAD_GATEWAY)
                            .body(Body::from("Bad Gateway: invalid upstream path"))
                            .unwrap());
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, upstream = %url, "failed to join upstream URI");
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::from("Bad Gateway: invalid upstream path"))
                        .unwrap());
                }
            }
        }
        None => match url.join("/") {
            Ok(u) => match u.as_str().parse::<http::Uri>() {
                Ok(uri) => uri,
                Err(e) => {
                    tracing::warn!(error = %e, upstream = %url, "failed to parse upstream URI");
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::from("Bad Gateway: invalid upstream URL"))
                        .unwrap());
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, upstream = %url, "failed to join upstream URI");
                return Ok(Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from("Bad Gateway: invalid upstream URL"))
                    .unwrap());
            }
        },
    };

    // Build the upstream request.
    let mut upstream_req = Request::builder()
        .method(req.method().clone())
        .uri(upstream_uri);

    // Copy headers, stripping hop-by-hop ones and setting Host.
    for (name, value) in req.headers() {
        if !HOP_BY_HOP.contains(&name.as_str()) {
            upstream_req = upstream_req.header(name, value);
        }
    }
    // Set Host header to upstream host.
    upstream_req = upstream_req.header("host", url.host_str().unwrap_or_default());

    let upstream_req = match upstream_req.body(req.into_body()) {
        Ok(req) => req,
        Err(e) => {
            tracing::warn!(error = %e, "failed to build upstream request");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("Bad Gateway: request build failed"))
                .unwrap());
        }
    };

    // Send the request upstream.
    let upstream_resp = match client.request(upstream_req).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::warn!(error = %e, upstream = %url, "proxy upstream request failed");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Bad Gateway: {e}")))
                .unwrap());
        }
    };

    // Build the response to forward to the client.
    let mut builder = Response::builder().status(upstream_resp.status());

    // Copy response headers, stripping hop-by-hop ones.
    for (name, value) in upstream_resp.headers() {
        if !HOP_BY_HOP.contains(&name.as_str()) {
            builder = builder.header(name, value);
        }
    }

    // Convert the upstream body to axum Body.
    // Collect the incoming body to bytes, then wrap in axum Body.
    let bytes = match upstream_resp.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::warn!(error = %e, "failed to read upstream body");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("Bad Gateway: body read failed"))
                .unwrap());
        }
    };
    let body = Body::from(bytes);

    match builder.body(body) {
        Ok(resp) => Ok(resp),
        Err(e) => {
            tracing::warn!(error = %e, "failed to build response");
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from("Bad Gateway: response build failed"))
                .unwrap())
        }
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
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

    /// Test the Remote frontend mode with a mock upstream server.
    ///
    /// Spawns a minimal TCP server that returns a static HTML response,
    /// then verifies that the reverse proxy correctly forwards requests
    /// and that API routes are not proxied.
    #[tokio::test]
    async fn test_frontend_remote() {
        // Bind a TCP listener on a random available port.
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind to random port must succeed");
        let port = listener.local_addr().unwrap().port();
        let upstream_url = format!("http://127.0.0.1:{port}");
        let upstream_url: url::Url = upstream_url.parse().unwrap();

        // Spawn a minimal mock upstream server.
        let server_task = tokio::spawn(async move {
            loop {
                let mut stream = listener.accept().await.expect("accept must succeed").0;

                // Read the request line (simplified HTTP/1.1 parser).
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).await.expect("read must succeed");
                let _request_line = String::from_utf8_lossy(&buf[..n]);

                // Build a minimal HTTP/1.1 response.
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<h1>AnvilML Remote Proxy Test</h1>"
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write response must succeed");
                break; // Only handle one request.
            }
        });

        // Build the router with Remote mode pointing to the mock server.
        let router = Router::new().route("/health", axum::routing::get(|| async { "ok" }));
        let router = add_frontend_route(router, &FrontendMode::Remote { url: upstream_url });
        let app = build_app(router);

        // Send a GET / request — should be proxied and return 200 with the body.
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

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        assert!(
            body_str.contains("AnvilML Remote Proxy Test"),
            "expected body to contain 'AnvilML Remote Proxy Test', got: {}",
            body_str
        );

        // Verify that /health still returns 200 (API routes take priority over fallback_service).
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

        // Wait for the server task to finish (it exits after one request).
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server_task).await;
    }
}
