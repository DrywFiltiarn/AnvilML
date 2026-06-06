mod handlers;
mod state;
pub mod ws;

pub use ws::broadcaster::EventBroadcaster;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

pub use state::AppState;

use crate::ws::handler::ws_events;

/// Build the application `Router` with all routes wired up.
pub fn build_router(state: AppState) -> Router {
    let state_arc = Arc::new(state);

    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/v1/events", get(ws_events))
        .route("/v1/models/rescan", post(handlers::models::rescan_models))
        .route("/v1/models/{id}", get(handlers::models::get_model))
        .route("/v1/models", get(handlers::models::list_models))
        .route("/v1/system/env", get(handlers::system::get_env))
        .route("/v1/system", get(handlers::system::get_system))
        .route("/v1/workers", get(handlers::workers::list_workers))
        .with_state(state_arc)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::http::{Request, StatusCode};
    use bytes::Bytes;
    use http_body_util::Full;
    use serde_json::Value;
    use tower::ServiceExt;

    use crate::{build_router, AppState, EventBroadcaster};

    #[tokio::test]
    async fn health_returns_200() {
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let state = AppState::new("0.1.0", None, None, None, broadcaster, None);
        let app = build_router(state);

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
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let state = AppState::new("0.1.0", None, None, None, broadcaster, None);
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/system/env")
                    .body(Full::<Bytes>::default())
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

    #[tokio::test]
    #[cfg(feature = "mock-hardware")]
    async fn system_returns_200_with_hardware_info() {
        // Set up mock device before detection.
        std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
        std::env::set_var("ANVILML_MOCK_VRAM_MIB", "12288");

        let pool = anvilml_registry::open_in_memory().await.unwrap();
        let hw_info =
            anvilml_hardware::detect_all_devices(&anvilml_core::ServerConfig::default(), &pool)
                .await
                .expect("detect_all_devices should succeed");

        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let state =
            AppState::new_with_hardware("0.1.0", hw_info, None, None, None, broadcaster, None);
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/system")
                    .body(Full::<Bytes>::default())
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

        // Host block must be populated.
        assert!(!parsed["host"]["os"].is_null());
        assert!(!parsed["host"]["cpu_model"].is_null());
        assert!(parsed["host"]["ram_total_mib"].is_number());
        assert!(parsed["host"]["ram_free_mib"].is_number());

        // GPUs must be non-empty.
        assert!(parsed["gpus"].is_array());
        let gpus = parsed["gpus"].as_array().unwrap();
        assert!(!gpus.is_empty(), "must have at least one GPU device");

        // First GPU should be a CUDA mock device.
        assert_eq!(gpus[0]["device_type"], "cuda");
        assert_eq!(gpus[0]["vram_total_mib"].as_u64().unwrap(), 12288);
        assert_eq!(gpus[0]["enumeration_source"], "Mock");

        // Inference caps must be present.
        assert!(parsed["inference_caps"].is_object());
    }

    #[tokio::test]
    async fn get_model_returns_404_when_missing() {
        // Use a temporary file-based database with migrations so the
        // registry actually has tables to query.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db_path = tmp.path().to_path_buf();
        let pool = anvilml_registry::db::open(&db_path)
            .await
            .expect("open db must succeed");
        let registry = std::sync::Arc::new(anvilml_registry::ModelRegistry::new(pool));
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let state = AppState::new("0.1.0", None, Some(registry), None, broadcaster, None);
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models/nonexistent-id")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        assert_eq!(parsed["error"], "not_found");
        assert_eq!(parsed["message"], "model not found");
    }

    #[tokio::test]
    async fn rescan_returns_202() {
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let state = AppState::new("0.1.0", None, None, None, broadcaster, None);
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/models/rescan")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        assert_eq!(parsed["status"], "rescan_started");
    }

    /// GET /v1/workers returns 200 with a JSON array of WorkerInfo.
    ///
    /// Constructs an AppState with a mock WorkerPool (no real Python workers)
    /// and verifies the handler returns an empty array when no workers are
    /// registered in the pool.
    #[tokio::test]
    async fn workers_endpoint_returns_200() {
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        // No WorkerPool — handler should return 503 with empty array.
        let state = AppState::new("0.1.0", None, None, None, broadcaster, None);
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/workers")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 0);
    }
}
