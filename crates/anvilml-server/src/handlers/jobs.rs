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
use anvilml_registry::JobStore;
use anvilml_scheduler::CancelOutcome;
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

/// Cancel a job by its ID.
///
/// Accepts a job UUID as a path parameter (`/v1/jobs/{id}/cancel`). Delegates
/// entirely to `JobScheduler::cancel()`, which returns a `CancelOutcome`
/// indicating whether the cancellation was accepted, the job was already
/// terminal, or the job does not exist.
///
/// # Response
///
/// - `202 Accepted` — the job was in a cancellable state (Queued or Running)
///   and cancellation was accepted.
/// - `409 Conflict` — the job exists but is already in a terminal state
///   (Completed/Failed/Cancelled). Cancelling a finished job is a no-op, not
///   an error, per the idempotent-cancel principle.
/// - `404 Not Found` — no job with the given ID exists in the database.
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `JobScheduler` through `state.scheduler`.
#[tracing::instrument(skip(state), fields(job_id = %id))]
pub(crate) async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AnvilError> {
    // Delegate to the scheduler's cancel method. The CancelOutcome enum
    // distinguishes three cases so we can return the correct HTTP status:
    // - Accepted → 202 (queued or running job accepted for cancellation)
    // - AlreadyTerminal → 409 (job is already finished — no-op, not an error)
    // - NotFound → 404 (job ID does not exist in the database)
    // AnvilError is returned via ? for DB-level failures (mapped to 500).
    match state.scheduler.cancel(id).await? {
        CancelOutcome::Accepted => {
            tracing::info!(job_id = %id, "cancel accepted");
            Ok(StatusCode::ACCEPTED)
        }
        CancelOutcome::AlreadyTerminal => {
            tracing::debug!(job_id = %id, "cancel rejected: already terminal");
            Ok(StatusCode::CONFLICT)
        }
        CancelOutcome::NotFound => {
            tracing::debug!(job_id = %id, "cancel: job not found");
            Ok(StatusCode::NOT_FOUND)
        }
    }
}

/// Delete a job by its ID, along with all associated artifacts.
///
/// Accepts a job UUID as a path parameter (`/v1/jobs/{id}`). The handler
/// looks up the job, verifies it is in a terminal state (Completed, Failed,
/// or Cancelled), deletes all associated artifacts, then deletes the job row.
///
/// # Response
///
/// - `204 No Content` — the job was terminal and has been deleted along with
///   all associated artifacts.
/// - `409 Conflict` — the job exists but is in a non-terminal state (Queued
///   or Running). Deleting an active job is not allowed — use cancel first.
/// - `404 Not Found` — no job with the given ID exists in the database.
///
/// # State Access
///
/// Reads from `state.db` via `JobStore` and from `state.artifact_store` for
/// artifact listing and deletion.
///
/// # Note
///
/// Per `ANVILML_DESIGN.md §13.4`, only terminal-status jobs may be deleted.
/// Non-terminal jobs must be cancelled first (POST /v1/jobs/{id}/cancel).
/// Bulk delete is deferred to P18-E2.
#[tracing::instrument(skip(state), fields(job_id = %id))]
pub(crate) async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AnvilError> {
    // Look up the job by ID. If it does not exist, return 404.
    // We create a JobStore from the shared db pool — this is the same
    // pattern used by all other job-related handlers.
    let job_store = JobStore::new(state.db.clone());
    let job = job_store.get(id).await?;

    let job = job.ok_or_else(|| AnvilError::JobNotFound(id.to_string()))?;

    // Check the job status — only terminal jobs (Completed, Failed, Cancelled)
    // may be deleted. Queued and Running jobs must be cancelled first.
    // This mirrors the cancel handler's 409 pattern for non-terminal states.
    if matches!(job.status, JobStatus::Queued | JobStatus::Running) {
        // Log the status as a string via serde_json serialization — JobStatus
        // does not implement Display, so we serialize it to its snake_case form.
        let status_str = serde_json::to_string(&job.status)
            .map(|s| s.trim_matches('"').to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        tracing::debug!(
            job_id = %id,
            status = %status_str,
            "delete rejected: job is not in a terminal state"
        );
        return Ok(StatusCode::CONFLICT);
    }

    // List all artifacts associated with this job, then delete each one.
    // Each delete removes both the file from disk and the DB row.
    // If no artifacts exist, list() returns an empty vec and the loop is
    // a no-op — this is correct behavior.
    let artifacts = state.artifact_store.list(Some(id)).await?;
    let artifact_count = artifacts.len();

    for artifact in &artifacts {
        // Delete each artifact file and DB row. Errors are logged but do
        // not abort the deletion — we attempt to remove all artifacts
        // before deleting the job, so the caller sees partial cleanup
        // rather than a complete failure.
        if let Err(e) = state.artifact_store.delete(&artifact.hash).await {
            tracing::warn!(
                job_id = %id,
                artifact_hash = %artifact.hash,
                error = %e,
                "failed to delete artifact — continuing with remaining artifacts"
            );
        }
    }

    if artifact_count > 0 {
        tracing::debug!(job_id = %id, artifact_count, "deleted artifacts");
    }

    // Delete the job row. This is the final cleanup step — after all
    // artifacts are removed, the job itself is removed from the database.
    job_store.delete(id).await?;

    tracing::info!(job_id = %id, "deleted job and artifacts");
    Ok(StatusCode::NO_CONTENT)
}
