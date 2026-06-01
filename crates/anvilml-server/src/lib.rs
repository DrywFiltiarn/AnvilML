mod handlers;
mod state;

use std::sync::Arc;

use axum::{routing::get, Router};

pub use state::AppState;

/// Build the application `Router` with all routes wired up.
pub fn build_router(state: AppState) -> Router {
    let state_arc = Arc::new(state);

    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/v1/system/env", get(handlers::system::get_env))
        .with_state(state_arc)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use tower::ServiceExt;

    use crate::{build_router, AppState};

    #[tokio::test]
    async fn health_returns_200() {
        let state = AppState::new("0.1.0");
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["version"], "0.1.0");
        assert!(parsed["uptime_s"].is_u64());
    }

    #[tokio::test]
    async fn env_returns_200_with_stub_report() {
        let state = AppState::new("0.1.0");
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/system/env")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        assert_eq!(parsed["python_path"], "");
        assert_eq!(parsed["python_version"], "");
        assert_eq!(parsed["torch_version"], "");
        assert_eq!(parsed["preflight_ok"], false);
        assert_eq!(parsed["reason"], "not_checked");
    }
}
