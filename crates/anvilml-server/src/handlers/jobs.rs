//! Job submission handler — POST /v1/jobs.
//!
//! Validates the submitted graph via `anvilml_scheduler::validate_graph`, persists it to SQLite,
//! enqueues it in the scheduler, and returns a 202 with the real job_id and queue position.

use std::sync::Arc;

use anvilml_core::error::AnvilError;
use anvilml_core::types::job::{SubmitJobRequest, SubmitJobResponse};
use anvilml_scheduler::job_store::get_job as scheduler_get_job;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
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

/// Build a standard error JSON body.
fn error_body(code: &str, message: &str) -> serde_json::Value {
    json!({
        "error": code,
        "message": message,
        "request_id": Uuid::new_v4().to_string(),
    })
}

/// Submit a new job for execution.
///
/// Validates the graph structure via the scheduler, persists the job to SQLite,
/// enqueues it, and returns 202 with the real job_id and queue position.
#[utoipa::path(
    post,
    path = "/v1/jobs",
    summary = "Submit a new job for execution",
    request_body = SubmitJobRequest,
    responses(
        (status = 202, description = "Job accepted and queued", body = SubmitJobResponse),
        (status = 422, description = "Invalid graph — validation errors listed", body = ErrorInline),
        (status = 500, description = "Internal server error", body = ErrorInline)
    )
)]
pub async fn submit_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitJobRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let scheduler = match &state.scheduler {
        Some(s) => s,
        None => {
            tracing::error!("submit_job: scheduler not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body(
                    "scheduler_not_configured",
                    "job scheduler not available",
                )),
            );
        }
    };
    match scheduler.submit(req).await {
        Ok(resp) => (
            StatusCode::ACCEPTED,
            Json(serde_json::to_value(&resp).expect("submit response serialises")),
        ),
        Err(AnvilError::InvalidGraph(msg)) => {
            tracing::warn!(error = %msg, "submit_job: graph validation failed");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(error_body("invalid_graph", &msg)),
            )
        }
        Err(e) => {
            tracing::error!(error = %e, "submit_job: unexpected error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("internal_error", &e.to_string())),
            )
        }
    }
}

/// Retrieve a job by its UUID.
///
/// Returns 200 with the full `Job` record if found, or 404 if not found.
#[utoipa::path(
    get,
    path = "/v1/jobs/{id}",
    summary = "Get a job by ID",
    params(
        ("id" = Uuid, Path, description = "Job UUID")
    ),
    responses(
        (status = 200, description = "Job found", body = anvilml_core::types::job::Job),
        (status = 404, description = "Job not found"),
        (status = 500, description = "Internal server error", body = ErrorInline)
    )
)]
pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
) -> (StatusCode, Json<serde_json::Value>) {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            tracing::error!(get_job = %job_id, "get_job: database not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body(
                    "database_not_configured",
                    "database not available",
                )),
            );
        }
    };
    match scheduler_get_job(pool, job_id).await {
        Ok(Some(job)) => (
            StatusCode::OK,
            Json(serde_json::to_value(&job).expect("job serialises")),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "not_found",
                "message": format!("job {job_id} not found"),
            })),
        ),
        Err(e) => {
            tracing::error!(error = %e, get_job = %job_id, "get_job: database query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("internal_error", &e.to_string())),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anvilml_core::types::events::WsEvent;
    use anvilml_scheduler::{JobQueue, JobScheduler};
    use axum::{
        http::{Request, StatusCode},
        Router,
    };
    use bytes::Bytes;
    use http_body_util::Full;
    use serde_json::Value;
    use sqlx::SqlitePool;
    use tokio::sync::broadcast;
    use tokio::sync::Notify;
    use tower::ServiceExt;

    use crate::{build_router, AppState, EventBroadcaster};
    use uuid::Uuid;

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

    /// Create an in-memory SQLite pool with the `jobs` table.
    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("connect in-memory SQLite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jobs (
                id         TEXT PRIMARY KEY,
                status     TEXT    NOT NULL DEFAULT 'Queued',
                graph      TEXT    NOT NULL,
                settings   TEXT    NOT NULL,
                device_index INTEGER          DEFAULT -1,
                created_at INTEGER   NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                worker_id  TEXT,
                artifact_count INTEGER DEFAULT 0,
                error      TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create jobs table");

        pool
    }

    /// Build a test `AppState` with a real `JobScheduler` backed by an in-memory DB.
    async fn build_test_app() -> Router {
        let pool = setup_pool().await;
        let (broadcaster, _rx) = broadcast::channel::<WsEvent>(16);
        let notify = Arc::new(Notify::new());
        let workers: Arc<Vec<anvilml_core::types::worker::WorkerInfo>> = Arc::new(vec![]);

        let scheduler = Arc::new(JobScheduler::new(
            JobQueue::new(),
            workers,
            pool.clone(),
            broadcaster,
            notify,
        ));

        let broadcaster_ws = Arc::new(EventBroadcaster::new(16));
        let state = AppState::new(
            "0.1.0",
            Some(pool),
            None,
            None,
            broadcaster_ws,
            None,
            Some(scheduler),
        );
        build_router(state)
    }

    /// Submitting a graph with an unknown node type must return 422
    /// with `"error": "invalid_graph"` in the body.
    #[tokio::test]
    async fn submit_job_bad_graph_returns_422() {
        let app = build_test_app().await;

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
    /// `job_id` (non-nil UUID) and `queue_position >= 1`.
    #[tokio::test]
    async fn submit_job_valid_zit_graph_returns_202() {
        let app = build_test_app().await;

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
        let _job_id: Uuid = parsed["job_id"]
            .as_str()
            .expect("job_id must be a string")
            .parse()
            .expect("job_id must be valid UUID");
        assert!(
            parsed["queue_position"].as_u64().unwrap() >= 1,
            "queue_position must be >= 1"
        );
    }

    /// Submit a valid job and then GET it — must return 200 with matching job_id
    /// and status "Queued".
    #[tokio::test]
    async fn get_job_returns_200_with_queued_job() {
        let app = build_test_app().await;

        // Submit a valid job.
        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 42, "steps": 20}
        });

        let submit_response = app
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

        assert_eq!(submit_response.status(), StatusCode::ACCEPTED);

        let body_bytes = axum::body::to_bytes(submit_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let submit_parsed: Value = serde_json::from_str(&body_str).unwrap();
        let job_id: Uuid = submit_parsed["job_id"]
            .as_str()
            .expect("job_id must be a string")
            .parse()
            .expect("job_id must be valid UUID");

        // Now GET the job.
        let get_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/v1/jobs/{job_id}"))
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(get_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(get_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let job_parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(job_parsed["id"], job_id.to_string());
        assert_eq!(job_parsed["status"], "Queued");
    }

    /// GET a nonexistent job UUID must return 404 with `"error": "not_found"`.
    #[tokio::test]
    async fn get_job_returns_404_when_missing() {
        let app = build_test_app().await;

        let random_id = Uuid::new_v4();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/v1/jobs/{random_id}"))
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
    }
}
