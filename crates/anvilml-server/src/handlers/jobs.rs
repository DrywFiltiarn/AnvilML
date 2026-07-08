//! `POST /v1/jobs` handler — submit a computation graph for execution.
//!
//! Accepts a JSON body containing a graph definition and job settings,
//! delegates entirely to `JobScheduler::submit()`, and returns `202 Accepted`
//! with the job ID and queue position. Zero business logic — per
//! `ANVILML_DESIGN.md §3.3`, the HTTP layer is a thin delegation layer.

use anvilml_core::AnvilError;
use anvilml_core::Job;
use anvilml_core::JobSettings;
use anvilml_core::JobStatus;
use axum::Json;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::state::AppState;

/// HTTP request body for `POST /v1/jobs`.
///
/// Contains the computation graph (`serde_json::Value`) and execution
/// settings (`JobSettings`). The graph is validated by the scheduler
/// before acceptance.
#[derive(Debug, Deserialize)]
pub(crate) struct SubmitJobRequest {
    /// The computation graph to execute, in the format expected by workers.
    pub graph: serde_json::Value,
    /// Optional execution settings (device preference, etc.).
    pub settings: JobSettings,
}

/// HTTP response body for `POST /v1/jobs` on success.
///
/// Per `ANVILML_DESIGN.md §13.5`: `202 { job_id, queue_position }`.
#[derive(Debug, Serialize)]
pub(crate) struct SubmitJobResponse {
    /// The UUID v4 assigned to the newly submitted job.
    pub job_id: Uuid,
    /// The job's 1-based position in the submission queue at the time of
    /// acceptance. This is captured atomically with the enqueue operation
    /// inside `JobScheduler::submit()`.
    pub queue_position: u32,
}

/// Submit a job graph for execution.
///
/// Accepts a `SubmitJobRequest`, delegates to `JobScheduler::submit()`,
/// and returns `202 Accepted` with the job ID and queue position.
///
/// On error, `AnvilError`'s `IntoResponse` impl maps the error to the
/// appropriate HTTP status code:
/// - `WorkersUnavailable` → 503 (no workers registered)
/// - `InvalidGraph` / `CycleDetected` → 400 (graph validation failed)
/// - `Db` → 500 (database error)
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `JobScheduler` through `state.scheduler`.
pub(crate) async fn submit_job(
    State(state): State<AppState>,
    Json(body): Json<SubmitJobRequest>,
) -> Result<(StatusCode, Json<SubmitJobResponse>), AnvilError> {
    // Delegate to the scheduler's submit method. It validates the graph,
    // persists the job, enqueues it, and returns (job_id, queue_position).
    // The HTTP handler adds no business logic — per ANVILML_DESIGN.md §3.3.
    let (job_id, queue_position) = state.scheduler.submit(body.graph, body.settings).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(SubmitJobResponse {
            job_id,
            queue_position,
        }),
    ))
}

/// Query parameters for `GET /v1/jobs` (list endpoint).
///
/// All fields are optional:
/// - `status` filters jobs by their lifecycle status.
/// - `limit` caps the number of returned jobs.
/// - `before` is a cursor for future pagination support; the persistence
///   layer does not yet use it, so it is accepted at the HTTP layer but
///   silently ignored (forward-compatibility per `ANVILML_DESIGN.md §13.4`).
#[derive(Debug, Deserialize)]
pub(crate) struct ListJobsParams {
    /// Optional status filter — only jobs matching this status are returned.
    pub status: Option<JobStatus>,
    /// Optional maximum number of jobs to return.
    pub limit: Option<u32>,
    /// Cursor for future pagination — accepted but not yet passed to the
    /// persistence layer (forward-compatibility per `ANVILML_DESIGN.md §13.4`).
    /// This field is intentionally unused by the handler — the struct field
    /// exists so the HTTP API accepts the query parameter without returning
    /// a 400 error, but the handler does not forward it to `list_jobs()`.
    /// Stored as String because serde_urlencoded cannot deserialize
    /// DateTime<Utc> from query strings.
    #[allow(dead_code)] // Intentionally unused — accepted at HTTP layer for forward-compat.
    #[serde(default)]
    pub before: Option<String>,
}

/// List jobs, optionally filtered by status.
///
/// Accepts optional `status` and `limit` query parameters. Delegates to
/// `JobScheduler::list_jobs()` which queries the database and returns all
/// matching jobs.
///
/// The `before` query parameter is accepted for forward-compatibility but
/// is not passed to the persistence layer — `JobStore::list()` does not
/// support a before-cursor parameter (per `ANVILML_DESIGN.md §13.4`).
///
/// # Response
///
/// Returns `200 OK` with a JSON array of `Job` objects.
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `JobScheduler` through `state.scheduler`.
#[tracing::instrument(skip(state), fields(status, limit))]
pub(crate) async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<ListJobsParams>,
) -> Result<Json<Vec<Job>>, AnvilError> {
    // Delegate to the scheduler's list_jobs method. It queries the database
    // and returns all matching jobs. The `before` field is accepted at the
    // HTTP layer for forward-compatibility but is not passed to list_jobs()
    // because JobStore::list() does not support a before-cursor.
    let jobs = state
        .scheduler
        .list_jobs(params.status, params.limit)
        .await?;
    tracing::info!(count = jobs.len(), "listed jobs");
    Ok(Json(jobs))
}

/// Look up a single job by its ID.
///
/// Accepts a job UUID as a path parameter (`/v1/jobs/:id`). Delegates to
/// `JobScheduler::get_job()` which queries the database.
///
/// # Response
///
/// Returns `200 OK` with the `Job` object, or `404 Not Found` if the job
/// does not exist in the database.
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `JobScheduler` through `state.scheduler`.
#[tracing::instrument(skip(state), fields(job_id))]
pub(crate) async fn get_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Job>, AnvilError> {
    // Delegate to the scheduler's get_job method. If the job is not found
    // (None), return AnvilError::JobNotFound which IntoResponse maps to 404.
    let job = state.scheduler.get_job(job_id).await?;
    job.ok_or_else(|| AnvilError::JobNotFound(job_id.to_string()))
        .map(Json)
}
