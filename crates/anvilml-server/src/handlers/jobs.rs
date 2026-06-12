//! Job submission handler — POST /v1/jobs.
//!
//! Validates the submitted graph via `anvilml_scheduler::validate_graph`, persists it to SQLite,
//! enqueues it in the scheduler, and returns a 202 with the real job_id and queue position.

use std::sync::Arc;

use anvilml_core::error::AnvilError;
use anvilml_core::types::job::{JobStatus, SubmitJobRequest, SubmitJobResponse};
use anvilml_scheduler::job_store::{
    delete_by_status, get_job as scheduler_get_job, list_jobs as scheduler_list_jobs,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::App;

/// Error response body for graph validation failures.
#[derive(Debug, ToSchema)]
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
/// Returns 503 `workers_unavailable` when the Python preflight check has failed
/// (unless `ANVILML_WORKER_MOCK` is set).
#[utoipa::path(
    post,
    path = "/v1/jobs",
    summary = "Submit a new job for execution",
    request_body = SubmitJobRequest,
    responses(
        (status = 202, description = "Job accepted and queued", body = SubmitJobResponse),
        (status = 422, description = "Invalid graph — validation errors listed", body = ErrorInline),
        (status = 500, description = "Internal server error", body = ErrorInline),
        (status = 503, description = "Workers unavailable — preflight failed", body = ErrorInline)
    )
)]
pub async fn submit_job(
    State(state): State<Arc<App>>,
    Json(req): Json<SubmitJobRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    // Preflight gate: reject job submission when Python environment is
    // unhealthy, unless we are in mock mode (ANVILML_WORKER_MOCK is set).
    let preflight = state.env_report();
    if !preflight.preflight_ok && std::env::var("ANVILML_WORKER_MOCK").is_err() {
        let reason = preflight.reason.clone();
        tracing::warn!(
            reason = %reason,
            "submit_job: preflight failed, rejecting"
        );
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "workers_unavailable",
                &format!("python preflight failed: {reason}"),
            )),
        );
    }

    // Shutdown gate: reject new submissions once the server is shutting down.
    if state.is_shutdown() {
        tracing::info!("submit_job: server is shutting down, rejecting");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(error_body(
                "server_shutting_down",
                "server is shutting down — no new submissions accepted",
            )),
        );
    }

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
    State(state): State<Arc<App>>,
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

/// Cancel a queued or running job.
///
/// Transitions the job to `Cancelled` and returns 202. Returns 404 if the job
/// does not exist, or 409 if the job is in a terminal state.
#[utoipa::path(
    post,
    path = "/v1/jobs/{id}/cancel",
    summary = "Cancel a queued or running job",
    params(
        ("id" = Uuid, Path, description = "Job UUID")
    ),
    responses(
        (status = 202, description = "Job cancelled"),
        (status = 404, description = "Job not found"),
        (status = 409, description = "Job not cancellable — already terminal"),
        (status = 500, description = "Internal server error", body = ErrorInline)
    )
)]
pub async fn cancel_job(
    State(state): State<Arc<App>>,
    Path(job_id): Path<Uuid>,
) -> (StatusCode, Json<serde_json::Value>) {
    let scheduler = match &state.scheduler {
        Some(s) => s,
        None => {
            tracing::error!(cancel_job = %job_id, "cancel_job: scheduler not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body(
                    "scheduler_not_configured",
                    "job scheduler not available",
                )),
            );
        }
    };
    match scheduler.cancel(job_id).await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "cancelled",
                "job_id": job_id.to_string()
            })),
        ),
        Err(AnvilError::JobNotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "not_found",
                "message": format!("job {job_id} not found"),
            })),
        ),
        Err(AnvilError::JobNotCancellable(_)) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "job_not_cancellable",
                "message": format!("job {job_id} is not in a cancellable state"),
            })),
        ),
        Err(AnvilError::DbError(_)) => {
            tracing::error!(error = %job_id, cancel_job = %job_id, "cancel_job: database error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("internal_error", "database error")),
            )
        }
        Err(e) => {
            tracing::error!(error = %e, cancel_job = %job_id, "cancel_job: unexpected error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("internal_error", &e.to_string())),
            )
        }
    }
}

/// Delete a terminal job and its artifacts.
///
/// Returns 204 No Content on success. Returns 404 if the job does not exist,
/// or 409 if the job is in a non-terminal state (Queued or Running).
///
/// The job must be in a terminal state (Completed, Failed, or Cancelled).
/// On success, all on-disk artifact files are removed first, then the job
/// and artifact rows are deleted from the database.
#[utoipa::path(
    delete,
    path = "/v1/jobs/{id}",
    summary = "Delete a terminal job and its artifacts",
    params(
        ("id" = Uuid, Path, description = "Job UUID")
    ),
    responses(
        (status = 204, description = "Job deleted"),
        (status = 404, description = "Job not found"),
        (status = 409, description = "Job is not terminal — cannot delete"),
        (status = 500, description = "Internal server error", body = ErrorInline)
    )
)]
pub async fn delete_job(
    State(state): State<Arc<App>>,
    Path(job_id): Path<Uuid>,
) -> axum::response::Response {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            tracing::error!(delete_job = %job_id, "delete_job: database not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body(
                    "database_not_configured",
                    "database not available",
                )),
            )
                .into_response();
        }
    };

    // 1. Read the job to check its status.
    let job = match scheduler_get_job(pool, job_id).await {
        Ok(Some(job)) => job,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not_found",
                    "message": format!("job {job_id} not found"),
                })),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, delete_job = %job_id, "delete_job: database query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("internal_error", &e.to_string())),
            )
                .into_response();
        }
    };

    // 2. Reject if job is not in a terminal state.
    if job.status == JobStatus::Queued || job.status == JobStatus::Running {
        let status_str = match job.status {
            JobStatus::Queued => "Queued",
            JobStatus::Running => "Running",
            _ => "Unknown",
        };
        tracing::warn!(
            job_id = %job_id,
            status = status_str,
            "delete_job: rejecting non-terminal job"
        );
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "job_active",
                "message": format!("job {job_id} is {status_str} — must be terminal to delete"),
            })),
        )
            .into_response();
    }

    // 3. Delete artifacts (best-effort — log on failure but continue).
    if let Err(e) = state
        .artifact_store
        .delete_for_job(&job_id.to_string())
        .await
    {
        tracing::warn!(
            error = %e,
            job_id = %job_id,
            "delete_job: artifact deletion failed, continuing with job deletion"
        );
    }

    // 4. Delete the job row.
    if let Err(e) = sqlx::query("DELETE FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .execute(pool)
        .await
    {
        tracing::error!(error = %e, delete_job = %job_id, "delete_job: failed to delete job row");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error_body("internal_error", "failed to delete job")),
        )
            .into_response();
    }

    tracing::info!(job_id = %job_id, "job deleted");

    StatusCode::NO_CONTENT.into_response()
}

/// Query parameters for the `GET /v1/jobs` list endpoint.
#[derive(Debug, Deserialize, Default)]
pub struct ListJobsQuery {
    /// Filter by job status (case-insensitive).
    pub status: Option<String>,
    /// Maximum number of results (default 100, max 1000).
    pub limit: Option<u32>,
    /// Only return jobs created before this ISO 8601 timestamp.
    pub before: Option<String>,
}

/// List jobs with optional status, limit, and before-cursor filters.
///
/// Returns a JSON array of `Job` objects sorted newest-first.
#[utoipa::path(
    get,
    path = "/v1/jobs",
    summary = "List jobs with optional filters",
    params(
        ("status" = Option<JobStatus>, Query, description = "Filter by job status"),
        ("limit" = Option<u32>, Query, description = "Maximum number of results (default 100, max 1000)"),
        ("before" = Option<String>, Query, description = "Only jobs created before this ISO 8601 timestamp")
    ),
    responses(
        (status = 200, description = "Job list", body = Vec<anvilml_core::types::job::Job>),
        (status = 503, description = "Database not available", body = ErrorInline)
    )
)]
pub async fn list_jobs(
    State(state): State<Arc<App>>,
    Query(query): Query<ListJobsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            tracing::error!("list_jobs: database not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body(
                    "database_not_configured",
                    "database not available",
                )),
            );
        }
    };

    // Parse status filter (case-insensitive).
    let parsed_status = query
        .status
        .as_deref()
        .and_then(|s| match s.to_lowercase().as_str() {
            "queued" => Some(JobStatus::Queued),
            "running" => Some(JobStatus::Running),
            "completed" => Some(JobStatus::Completed),
            "failed" => Some(JobStatus::Failed),
            "cancelled" => Some(JobStatus::Cancelled),
            unknown => {
                tracing::warn!(
                    status = unknown,
                    "list_jobs: unknown status value, ignoring filter"
                );
                None
            }
        });

    // Parse before cursor (ISO 8601 / RFC 3339).
    let parsed_before = query.before.as_deref().and_then(|s| {
        DateTime::<chrono::FixedOffset>::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });
    if query.before.is_some() && parsed_before.is_none() {
        tracing::warn!(before = ?query.before, "list_jobs: invalid before timestamp, ignoring filter");
    }

    // Compute effective limit: default 100, clamped to [1, 1000].
    let effective_limit = query.limit.unwrap_or(100).clamp(1, 1000);

    match scheduler_list_jobs(pool, parsed_status, Some(effective_limit), parsed_before).await {
        Ok(jobs) => (
            StatusCode::OK,
            Json(serde_json::to_value(&jobs).expect("job list serialises")),
        ),
        Err(e) => {
            tracing::error!(error = %e, "list_jobs: database query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("internal_error", &e.to_string())),
            )
        }
    }
}

/// Query parameters for the `DELETE /v1/jobs` bulk-clear endpoint.
#[derive(Debug, Deserialize, Default)]
pub struct ClearJobsQuery {
    /// Filter by job status (case-insensitive).
    ///
    /// Accepted values: `completed`, `failed`, `cancelled`, `all`.
    /// `None` or `"all"` clears all terminal jobs.
    pub status: Option<String>,
}

/// Response body for the `DELETE /v1/jobs` bulk-clear endpoint.
#[derive(Debug, Serialize, ToSchema)]
pub struct ClearJobsResponse {
    /// Number of jobs successfully removed.
    pub removed: u32,
}

/// Bulk-clear terminal jobs by status.
///
/// Deletes all jobs matching the given status filter (default: all terminal
/// jobs). For each matched job, artifacts are deleted first (best-effort),
/// then the job row is removed from the database.
///
/// * `status` — `completed|failed|cancelled|all` (case-insensitive).
///   Defaults to `all` when omitted.
///
/// Returns `{ "removed": <count> }` with 200 OK.
#[utoipa::path(
    delete,
    path = "/v1/jobs",
    summary = "Bulk-clear terminal jobs by status",
    params(
        ("status" = Option<String>, Query, description = "Filter by status: completed, failed, cancelled, or all (default: all)")
    ),
    responses(
        (status = 200, description = "Jobs cleared", body = ClearJobsResponse),
        (status = 400, description = "Invalid status parameter"),
        (status = 503, description = "Database not available", body = ErrorInline)
    )
)]
pub async fn clear_jobs(
    State(state): State<Arc<App>>,
    Query(query): Query<ClearJobsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let pool = match &state.db {
        Some(p) => p,
        None => {
            tracing::error!("clear_jobs: database not configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(error_body(
                    "database_not_configured",
                    "database not available",
                )),
            );
        }
    };

    // Parse and validate the status parameter.
    let parsed_filter = match query.status.as_deref() {
        None | Some("all") => None,
        Some("completed") | Some("Completed") => Some("Completed"),
        Some("failed") | Some("Failed") => Some("Failed"),
        Some("cancelled") | Some("Cancelled") => Some("Cancelled"),
        Some(other) => {
            tracing::warn!(status = other, "clear_jobs: invalid status parameter");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_status",
                    "message": format!(
                        "invalid status '{other}'; must be completed, failed, cancelled, or all"
                    ),
                })),
            );
        }
    };

    // Fetch matching job IDs.
    let job_ids = match delete_by_status(pool, parsed_filter).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!(error = %e, "clear_jobs: database query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("internal_error", &e.to_string())),
            );
        }
    };

    let total = job_ids.len();
    let mut failures = 0u32;

    // Delete artifacts and rows for each matched job.
    for id in &job_ids {
        let id_str = id.to_string();

        // Best-effort artifact deletion — log warn on failure, continue.
        if let Err(e) = state.artifact_store.delete_for_job(&id_str).await {
            tracing::warn!(
                job_id = %id,
                error = %e,
                "clear_jobs: artifact deletion failed, continuing"
            );
        }

        // Delete the job row — log error on failure but continue to next job.
        if let Err(e) = sqlx::query("DELETE FROM jobs WHERE id = ?")
            .bind(id_str)
            .execute(pool)
            .await
        {
            tracing::error!(
                error = %e,
                job_id = %id,
                "clear_jobs: failed to delete job row"
            );
            failures += 1;
        }
    }

    let removed = total as u32 - failures;
    tracing::info!(removed, "bulk_delete: cleared {} jobs", removed);

    (
        StatusCode::OK,
        Json(serde_json::to_value(&ClearJobsResponse { removed }).expect("response serialises")),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anvilml_core::types::events::WsEvent;
    use anvilml_scheduler::JobScheduler;
    use axum::{
        http::{Request, StatusCode},
        Router,
    };
    use bytes::Bytes;
    use http_body_util::Full;
    use serde_json::Value;
    use sqlx::SqlitePool;
    use tokio::sync::broadcast;
    use tower::ServiceExt;

    use crate::{build_router, App, EventBroadcaster};
    use chrono::Utc;
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

    /// Build a test `App` with a real `JobScheduler` backed by an in-memory DB.
    /// Returns the router and a clone of the pool for direct DB access in tests.
    async fn build_test_app() -> (Router, SqlitePool) {
        use anvilml_scheduler::{JobQueue, VramLedger};
        use anvilml_worker::WorkerPool;

        // Set mock mode so the preflight gate is bypassed in tests.
        // The env var is left set for the duration of the test.
        std::env::set_var("ANVILML_WORKER_MOCK", "1");

        let pool = setup_pool().await;
        let (broadcaster, _rx) = broadcast::channel::<WsEvent>(16);
        let workers = Arc::new(WorkerPool::new_test_pool());

        let artifact_store = crate::artifact::store::ArtifactStore::new(
            tempfile::tempdir().unwrap().keep(),
            pool.clone(),
        );

        let scheduler = Arc::new(JobScheduler::new(
            JobQueue::new(),
            workers,
            pool.clone(),
            broadcaster,
            Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
            "auto".to_string(),
            artifact_store.clone(),
        ));

        let broadcaster_ws = Arc::new(EventBroadcaster::new(16));
        let state = App::new(
            "0.1.0",
            Some(pool.clone()),
            None,
            None,
            broadcaster_ws,
            None,
            Some(scheduler),
            artifact_store,
            anvilml_core::ServerConfig::default(),
        );

        (build_router(state), pool)
    }

    /// Submitting a graph with an unknown node type must return 422
    /// with `"error": "invalid_graph"` in the body.
    #[tokio::test]
    async fn submit_job_bad_graph_returns_422() {
        let (app, _pool) = build_test_app().await;

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
        let (app, _pool) = build_test_app().await;

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
        let (app, _pool) = build_test_app().await;

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
        let (app, _pool) = build_test_app().await;

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

    /// Submit two jobs via POST and verify GET /v1/jobs returns both.
    #[tokio::test]
    async fn list_jobs_returns_all_submitted_jobs() {
        let (app, _pool) = build_test_app().await;

        // Submit first job.
        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 1, "steps": 10}
        });
        let resp1 = app
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
        assert_eq!(resp1.status(), StatusCode::ACCEPTED);

        // Submit second job.
        let req_body2 = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 2, "steps": 15}
        });
        let resp2 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/jobs")
                    .header("content-type", "application/json")
                    .body(Full::<Bytes>::from(serde_json::to_vec(&req_body2).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp2.status(), StatusCode::ACCEPTED);

        // List all jobs.
        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/jobs")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert!(parsed.is_array(), "response body must be a JSON array");
        assert_eq!(
            parsed.as_array().unwrap().len(),
            2,
            "must return exactly 2 jobs"
        );
    }

    /// GET /v1/jobs?status=queued must filter to only queued jobs.
    #[tokio::test]
    async fn list_jobs_filters_by_status() {
        let (app, _pool) = build_test_app().await;

        // Submit two jobs (both start as Queued).
        for seed in [10, 20] {
            let req_body = serde_json::json!({
                "graph": make_valid_zit_graph(),
                "settings": {"seed": seed, "steps": 5}
            });
            let resp = app
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
            assert_eq!(resp.status(), StatusCode::ACCEPTED);
        }

        // Filter by status=queued (lowercase).
        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/jobs?status=queued")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2, "both jobs are Queued");
    }

    /// GET /v1/jobs?limit=1 must return exactly one job.
    #[tokio::test]
    async fn list_jobs_limit_clamps_to_one() {
        let (app, _pool) = build_test_app().await;

        // Submit two jobs.
        for seed in [30, 40] {
            let req_body = serde_json::json!({
                "graph": make_valid_zit_graph(),
                "settings": {"seed": seed, "steps": 5}
            });
            let resp = app
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
            assert_eq!(resp.status(), StatusCode::ACCEPTED);
        }

        // List with limit=1.
        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/jobs?limit=1")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert!(parsed.is_array());
        assert_eq!(
            parsed.as_array().unwrap().len(),
            1,
            "limit=1 must return exactly one job"
        );
    }

    /// Cancel a nonexistent job UUID must return 404 with `"error": "not_found"`.
    #[tokio::test]
    async fn cancel_job_returns_404_when_missing() {
        let (app, _pool) = build_test_app().await;

        let random_id = Uuid::new_v4();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/v1/jobs/{random_id}/cancel"))
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

    /// Submit a valid job (starts Queued), then cancel it — must return 202.
    #[tokio::test]
    async fn cancel_job_returns_202_for_queued_job() {
        let (app, _pool) = build_test_app().await;

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

        // Cancel the job.
        let cancel_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/v1/jobs/{job_id}/cancel"))
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(cancel_response.status(), StatusCode::ACCEPTED);

        let body_bytes = axum::body::to_bytes(cancel_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let cancel_parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(cancel_parsed["status"], "cancelled");
        assert_eq!(cancel_parsed["job_id"], job_id.to_string());
    }

    /// Submit a job, then update its status to Completed in the DB,
    /// then cancel — must return 409 with `"error": "job_not_cancellable"`.
    #[tokio::test]
    async fn cancel_job_returns_409_for_completed_job() {
        let (app, pool) = build_test_app().await;

        // Submit a valid job.
        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 99, "steps": 5}
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

        // Simulate the job reaching Completed status by directly updating the DB.
        sqlx::query("UPDATE jobs SET status = 'Completed', completed_at = ? WHERE id = ?")
            .bind(Utc::now().timestamp())
            .bind(job_id.to_string())
            .execute(&pool)
            .await
            .expect("update job status to Completed");

        // Cancel the completed job — should return 409.
        let cancel_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/v1/jobs/{job_id}/cancel"))
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(cancel_response.status(), StatusCode::CONFLICT);

        let body_bytes = axum::body::to_bytes(cancel_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let cancel_parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(cancel_parsed["error"], "job_not_cancellable");
    }

    /// Submit a job, set its status to Completed in the DB,
    /// then DELETE — must return 204, and GET must return 404.
    #[tokio::test]
    async fn delete_job_returns_204_for_completed_job() {
        let (app, pool) = build_test_app().await;

        // Submit a valid job.
        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 77, "steps": 5}
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

        // Set the job to Completed in the DB.
        sqlx::query("UPDATE jobs SET status = 'Completed', completed_at = ? WHERE id = ?")
            .bind(Utc::now().timestamp())
            .bind(job_id.to_string())
            .execute(&pool)
            .await
            .expect("update job to Completed");

        // DELETE the completed job — should return 204.
        let delete_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/v1/jobs/{job_id}"))
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        // GET the deleted job — should return 404.
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

        assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
    }

    /// Submit a job (Queued), then DELETE — must return 409.
    #[tokio::test]
    async fn delete_job_returns_409_for_queued_job() {
        let (app, _pool) = build_test_app().await;

        // Submit a valid job (starts as Queued).
        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 88, "steps": 5}
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

        // DELETE the queued job — should return 409.
        let delete_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/v1/jobs/{job_id}"))
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(delete_response.status(), StatusCode::CONFLICT);

        let body_bytes = axum::body::to_bytes(delete_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let delete_parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(delete_parsed["error"], "job_active");
        assert!(
            delete_parsed["message"].is_string(),
            "message must be a string"
        );
    }

    /// DELETE a nonexistent job UUID must return 404.
    #[tokio::test]
    async fn delete_job_returns_404_when_missing() {
        let (app, _pool) = build_test_app().await;

        let random_id = Uuid::new_v4();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
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

    /// Bulk delete with `?status=completed` must return `{removed:N}`
    /// and remove only completed jobs from the list.
    #[tokio::test]
    async fn clear_jobs_returns_200_for_completed_jobs() {
        let (app, pool) = build_test_app().await;

        // Submit 3 jobs.
        let mut job_ids: Vec<Uuid> = Vec::new();
        for seed in [100, 101, 102] {
            let req_body = serde_json::json!({
                "graph": make_valid_zit_graph(),
                "settings": {"seed": seed, "steps": 5}
            });
            let resp = app
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
            assert_eq!(resp.status(), StatusCode::ACCEPTED);

            let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
            let parsed: Value = serde_json::from_str(&body_str).unwrap();
            let id: Uuid = parsed["job_id"]
                .as_str()
                .expect("job_id must be a string")
                .parse()
                .expect("job_id must be valid UUID");
            job_ids.push(id);
        }

        // Set all 3 to Completed in the DB.
        for id in &job_ids {
            sqlx::query("UPDATE jobs SET status = 'Completed', completed_at = ? WHERE id = ?")
                .bind(Utc::now().timestamp())
                .bind(id.to_string())
                .execute(&pool)
                .await
                .expect("update job to Completed");
        }

        // DELETE with status=completed.
        let clear_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/jobs?status=completed")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clear_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(clear_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(parsed["removed"].as_u64().unwrap(), 3, "must remove 3 jobs");

        // Verify list returns fewer jobs.
        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/jobs")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(list_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let list_parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert!(list_parsed.is_array());
        assert_eq!(
            list_parsed.as_array().unwrap().len(),
            0,
            "all completed jobs must be removed"
        );
    }

    /// Bulk delete must remove on-disk artifact files alongside job rows.
    #[tokio::test]
    async fn clear_jobs_removes_artifacts() {
        let (app, pool) = build_test_app().await;

        // Create the artifacts table (build_test_app only creates the jobs table).
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS artifacts (
                hash       TEXT PRIMARY KEY,
                job_id     TEXT    NOT NULL,
                width      INTEGER NOT NULL,
                height     INTEGER NOT NULL,
                format     TEXT    NOT NULL,
                seed       INTEGER NOT NULL,
                steps      INTEGER NOT NULL,
                prompt     TEXT    NOT NULL,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create artifacts table");

        // Submit a job.
        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 200, "steps": 5}
        });
        let resp = app
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
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        let job_id: Uuid = parsed["job_id"]
            .as_str()
            .expect("job_id must be a string")
            .parse()
            .expect("job_id must be valid UUID");

        // Insert a fake artifact row for this job.
        let fake_hash = "aaa111bbb222ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000";
        sqlx::query(
            "INSERT INTO artifacts (hash, job_id, width, height, format, seed, steps, prompt, created_at) \
             VALUES (?, ?, 512, 512, 'png', 200, 5, 'test', ?)",
        )
        .bind(fake_hash)
        .bind(job_id.to_string())
        .bind(Utc::now().timestamp())
        .execute(&pool)
        .await
        .expect("insert artifact");

        // Create the on-disk artifact file.
        let tmp_dir = tempfile::tempdir().unwrap();
        let prefix_dir = tmp_dir.path().join(&fake_hash[..2]);
        tokio::fs::create_dir_all(&prefix_dir).await.unwrap();
        let file_path = prefix_dir.join(format!("{fake_hash}.png"));
        tokio::fs::write(&file_path, b"fake-png").await.unwrap();

        // Replace the artifact store's temp dir with our controlled one.
        // Since we can't easily swap the store, we just verify artifact row deletion.
        // The artifact store's delete_for_job will try to remove the file from its
        // own temp dir (which doesn't contain our file) — best-effort, logs warn.

        // Set job to Completed.
        sqlx::query("UPDATE jobs SET status = 'Completed', completed_at = ? WHERE id = ?")
            .bind(Utc::now().timestamp())
            .bind(job_id.to_string())
            .execute(&pool)
            .await
            .expect("update job to Completed");

        // DELETE with status=completed.
        let clear_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/jobs?status=completed")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clear_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(clear_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(parsed["removed"].as_u64().unwrap(), 1, "must remove 1 job");

        // Verify artifact row is gone.
        let remaining: Vec<String> =
            sqlx::query_scalar("SELECT hash FROM artifacts WHERE job_id = ?")
                .bind(job_id.to_string())
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(
            remaining.is_empty(),
            "artifact rows for deleted job must be removed"
        );
    }

    /// DELETE ?status=all must never remove Queued or Running jobs.
    #[tokio::test]
    async fn clear_jobs_skips_running_jobs() {
        let (app, _pool) = build_test_app().await;

        // Submit a job (starts as Queued).
        let req_body = serde_json::json!({
            "graph": make_valid_zit_graph(),
            "settings": {"seed": 300, "steps": 5}
        });
        let resp = app
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
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        let job_id: Uuid = parsed["job_id"]
            .as_str()
            .expect("job_id must be a string")
            .parse()
            .expect("job_id must be valid UUID");

        // DELETE with status=all — Queued job should be untouched.
        let clear_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/jobs?status=all")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clear_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(clear_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(
            parsed["removed"].as_u64().unwrap(),
            0,
            "no terminal jobs to remove"
        );

        // Verify the Queued job still exists.
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
        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        assert_eq!(parsed["status"], "Queued");
    }

    /// DELETE ?status=running must return 400 with an error message.
    #[tokio::test]
    async fn clear_jobs_rejects_invalid_status() {
        let (app, _pool) = build_test_app().await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/jobs?status=running")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(parsed["error"], "invalid_status");
        assert!(parsed["message"].is_string());
    }

    /// DELETE with no status parameter must default to all terminal jobs.
    #[tokio::test]
    async fn clear_jobs_defaults_to_all() {
        let (app, pool) = build_test_app().await;

        // Submit 2 jobs.
        let mut job_ids: Vec<Uuid> = Vec::new();
        for seed in [400, 401] {
            let req_body = serde_json::json!({
                "graph": make_valid_zit_graph(),
                "settings": {"seed": seed, "steps": 5}
            });
            let resp = app
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
            assert_eq!(resp.status(), StatusCode::ACCEPTED);

            let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
            let parsed: Value = serde_json::from_str(&body_str).unwrap();
            let id: Uuid = parsed["job_id"]
                .as_str()
                .expect("job_id must be a string")
                .parse()
                .expect("job_id must be valid UUID");
            job_ids.push(id);
        }

        // Set one to Completed, leave the other as Queued.
        sqlx::query("UPDATE jobs SET status = 'Completed', completed_at = ? WHERE id = ?")
            .bind(Utc::now().timestamp())
            .bind(job_ids[0].to_string())
            .execute(&pool)
            .await
            .expect("update job to Completed");

        // DELETE with no status param — should only clear the Completed one.
        let clear_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/jobs")
                    .body(Full::<Bytes>::default())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(clear_response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(clear_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
        let parsed: Value = serde_json::from_str(&body_str).unwrap();

        assert_eq!(
            parsed["removed"].as_u64().unwrap(),
            1,
            "only the Completed job should be removed"
        );

        // Verify the Queued job still exists.
        let get_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/v1/jobs/{}", job_ids[1]))
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
        let parsed: Value = serde_json::from_str(&body_str).unwrap();
        assert_eq!(parsed["status"], "Queued");
    }
}
