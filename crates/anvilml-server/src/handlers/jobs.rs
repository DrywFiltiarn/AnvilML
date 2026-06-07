//! Job submission handler — POST /v1/jobs.
//!
//! Validates the submitted graph via `anvilml_scheduler::validate_graph` and returns
//! either a 422 (invalid graph) or 202 (accepted placeholder) response.
//! Enqueueing and persistence are deferred to phase 12.

use std::sync::Arc;

use anvilml_core::types::job::{SubmitJobRequest, SubmitJobResponse};
use anvilml_scheduler::validate_graph;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::state::AppState;

/// Error response body for graph validation failures.
#[derive(Debug, ToSchema)]
#[expect(dead_code)]
pub struct ErrorInline {
    /// Machine-readable error identifier.
    pub error: String,
    /// Human-readable error description (joined validation errors).
    pub message: String,
    /// Opaque request correlation ID.
    pub request_id: String,
}

/// Submit a new job for execution.
///
/// Validates the graph structure and returns a placeholder acceptance response.
#[utoipa::path(
    post,
    path = "/v1/jobs",
    summary = "Submit a new job for execution",
    request_body = SubmitJobRequest,
    responses(
        (status = 202, description = "Job accepted (placeholder; actual enqueue is phase 12)", body = SubmitJobResponse),
        (status = 422, description = "Invalid graph — validation errors listed", body = ErrorInline)
    )
)]
pub async fn submit_job(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<SubmitJobRequest>,
) -> impl IntoResponse {
    match validate_graph(&req.graph) {
        Ok(_) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "job_id": Uuid::new_v4().to_string(),
                "queue_position": 0,
            })),
        ),
        Err(errors) => {
            let message = errors.join(", ");
            tracing::warn!(errors = %message, "submit_job: graph validation failed");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": "invalid_graph",
                    "message": message,
                    "request_id": Uuid::new_v4().to_string(),
                })),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        http::{Request, StatusCode},
        Router,
    };
    use bytes::Bytes;
    use http_body_util::Full;
    use serde_json::Value;
    use tower::ServiceExt;

    use crate::{build_router, AppState, EventBroadcaster};

    fn make_valid_zit_graph() -> Value {
        serde_json::json!({
            "nodes": [
                {
                    "id": "load_pipeline",
                    "type": "ZitLoadPipeline",
                    "inputs": {"model_id": "runwayml/stable-diffusion-v1-5"}
                },
                {
                    "id": "text_encode",
                    "type": "ZitTextEncode",
                    "inputs": {"pipeline": ["load_pipeline", "pipeline"], "prompt": "a beautiful sunset"}
                },
                {
                    "id": "sampler",
                    "type": "ZitSampler",
                    "inputs": {
                        "pipeline": ["load_pipeline", "pipeline"],
                        "conditioning": ["text_encode", "conditioning"],
                        "steps": 20,
                        "seed": 42
                    }
                },
                {
                    "id": "decode",
                    "type": "ZitDecode",
                    "inputs": {"pipeline": ["load_pipeline", "pipeline"], "latents": ["sampler", "latents"]}
                },
                {
                    "id": "save",
                    "type": "SaveImage",
                    "inputs": {"image": ["decode", "image"]}
                }
            ],
            "edges": [
                ["load_pipeline", "text_encode"],
                ["load_pipeline", "sampler"],
                ["text_encode", "sampler"],
                ["sampler", "decode"],
                ["decode", "save"]
            ]
        })
    }

    fn make_invalid_graph() -> Value {
        serde_json::json!({
            "nodes": [
                {"id": "n0", "type": "NopeNode", "inputs": {}}
            ],
            "edges": []
        })
    }

    fn build_test_app() -> Router {
        let broadcaster = Arc::new(EventBroadcaster::new(16));
        let state = AppState::new("0.1.0", None, None, None, broadcaster, None);
        build_router(state)
    }

    /// Submitting a graph with an unknown node type must return 422
    /// with `"error": "invalid_graph"` in the body.
    #[tokio::test]
    async fn submit_job_bad_graph_returns_422() {
        let app = build_test_app();

        let req_body = serde_json::json!({
            "graph": make_invalid_graph(),
            "settings": {}
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/jobs")
                    .header("content-type", "application/json")
                    .body(Full::<Bytes>::from(serde_json::to_vec(&req_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(parsed["error"], "invalid_graph");
        assert!(
            parsed["message"].is_string(),
            "message must be a string, got: {}",
            parsed["message"]
        );
        assert!(
            !parsed["request_id"].is_null(),
            "request_id must be present"
        );
    }

    /// Submitting a valid ZiT 5-node graph must return 202 with
    /// `job_id` (UUID) and `queue_position: 0`.
    #[tokio::test]
    async fn submit_job_valid_zit_graph_returns_202() {
        let app = build_test_app();

        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {
                "seed": 42,
                "steps": 20,
                "guidance_scale": 7.5,
                "width": 1024,
                "height": 1024
            }
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/jobs")
                    .header("content-type", "application/json")
                    .body(Full::<Bytes>::from(serde_json::to_vec(&req_body).unwrap()))
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

        assert!(!parsed["job_id"].is_null(), "job_id must be present");
        assert_eq!(parsed["queue_position"], 0);
    }
}
