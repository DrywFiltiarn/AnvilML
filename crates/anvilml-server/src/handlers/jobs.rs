//! Job-related HTTP handlers — submit, list, get, cancel, delete.
//!
//! Per `ANVILML_DESIGN.md §3.3`, the HTTP layer is a thin delegation layer
//! with zero business logic: handlers validate inputs, delegate to the
//! scheduler or store, and map results to HTTP responses.

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
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub(crate) struct SubmitJobRequest {
    /// The computation graph to execute, in the format expected by workers.
    pub graph: serde_json::Value,
    /// Optional execution settings (device preference, etc.).
    pub settings: JobSettings,
}

/// HTTP response body for `POST /v1/jobs` on success.
///
/// Per `ANVILML_DESIGN.md §13.5`: `202 { job_id, queue_position }`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct SubmitJobResponse {
    /// The UUID v4 assigned to the newly submitted job.
    pub job_id: Uuid,
    /// The job's 1-based position in the submission queue at the time of
    /// acceptance. This is captured atomically with the enqueue operation
    /// inside `JobScheduler::submit()`.
    pub queue_position: u32,
}

/// HTTP query parameters for `DELETE /v1/jobs` (bulk clear endpoint).
///
/// The `status` parameter selects which terminal jobs to remove:
/// - `completed` — only Completed jobs
/// - `failed` — only Failed jobs
/// - `cancelled` — only Cancelled jobs
/// - `all` — all terminal jobs (Completed + Failed + Cancelled)
///
/// Returns `400 Bad Request` for any unrecognized value.
#[derive(Debug, Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
pub(crate) struct BulkClearParams {
    /// Filter by job status. Must be one of: completed, failed, cancelled, all.
    pub status: String,
}

/// HTTP response body for `DELETE /v1/jobs` on success.
///
/// Per `ANVILML_DESIGN.md §13.4`: `200 { removed: u32 }`.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub(crate) struct RemovedCount {
    /// Number of jobs removed by the bulk clear operation.
    pub removed: u32,
}

/// Delete a single job and all its associated artifacts.
///
/// This is the core deletion logic shared by both `delete_job` (single-job)
/// and `bulk_clear_jobs` (bulk). It performs three steps:
/// 1. List all artifacts for the job via `artifact_store.list(Some(id))`.
/// 2. Delete each artifact file + DB row, logging warnings on individual
///    failures but continuing with remaining artifacts.
/// 3. Delete the job row via `job_store.delete(id)`.
///
/// Returns the number of artifacts that were deleted (0 if none existed).
/// Errors from artifact deletion are logged but do not abort the deletion;
/// the job row is always deleted regardless of artifact deletion status.
async fn delete_single_job(
    state: &AppState,
    id: Uuid,
    job_store: &JobStore,
) -> Result<u32, AnvilError> {
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

    Ok(artifact_count as u32)
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
#[utoipa::path(
    post,
    path = "/v1/jobs",
    tag = "Jobs",
    operation_id = "submit_job",
    summary = "Submit a job",
    description = "Submits a computation graph for execution. The graph is validated by the scheduler before acceptance.",
    request_body = SubmitJobRequest,
    responses(
        (status = 202, description = "Job accepted for execution", body = SubmitJobResponse),
        (status = 400, description = "Bad request — malformed JSON or invalid graph"),
        (status = 409, description = "Conflict — graph contains a cycle"),
        (status = 500, description = "Database error"),
        (status = 503, description = "Workers unavailable — no idle workers registered")
    )
)]
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
#[derive(Debug, Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
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
#[utoipa::path(
    get,
    path = "/v1/jobs",
    tag = "Jobs",
    operation_id = "list_jobs",
    summary = "List jobs",
    description = "Lists all jobs, optionally filtered by status and limited by count.",
    params(ListJobsParams),
    responses(
        (status = 200, description = "List of jobs", body = Vec<Job>),
        (status = 500, description = "Database error")
    )
)]
#[tracing::instrument(skip(state), fields(status = ?params.status, limit = ?params.limit))]
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
#[utoipa::path(
    get,
    path = "/v1/jobs/{id}",
    tag = "Jobs",
    operation_id = "get_job",
    summary = "Get a job",
    description = "Looks up a single job by its UUID.",
    params(
        ("id" = Uuid, Path, description = "Job UUID")
    ),
    responses(
        (status = 200, description = "Job details", body = Job),
        (status = 404, description = "Job not found")
    )
)]
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
#[utoipa::path(
    post,
    path = "/v1/jobs/{id}/cancel",
    tag = "Jobs",
    operation_id = "cancel_job",
    summary = "Cancel a job",
    description = "Cancels a job by its UUID. Returns 202 if accepted, 409 if already terminal, 404 if not found.",
    params(
        ("id" = Uuid, Path, description = "Job UUID")
    ),
    responses(
        (status = 202, description = "Cancellation accepted"),
        (status = 404, description = "Job not found"),
        (status = 409, description = "Job is already in a terminal state")
    )
)]
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
/// or Cancelled), then delegates to `delete_single_job()` for artifact and
/// job deletion.
#[utoipa::path(
    delete,
    path = "/v1/jobs/{id}",
    tag = "Jobs",
    operation_id = "delete_job",
    summary = "Delete a job",
    description = "Deletes a job by its UUID and all associated artifacts. Only terminal-status jobs may be deleted.",
    params(
        ("id" = Uuid, Path, description = "Job UUID")
    ),
    responses(
        (status = 204, description = "Job and artifacts deleted"),
        (status = 404, description = "Job not found"),
        (status = 409, description = "Job is not in a terminal state")
    )
)]
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

    // Delegate to the shared delete_single_job helper. This ensures both the
    // single-job and bulk clear handlers use the exact same artifact-deletion
    // and job-deletion logic — no divergence between the two code paths.
    let _artifact_count = delete_single_job(&state, id, &job_store).await?;

    tracing::info!(job_id = %id, "deleted job and artifacts");
    Ok(StatusCode::NO_CONTENT)
}

/// Bulk-clear terminal jobs matching the given status filter.
///
/// Accepts `DELETE /v1/jobs?status=<value>` where `<value>` is one of:
/// `completed`, `failed`, `cancelled`, or `all`. For each matching job,
/// delegates to `delete_single_job()` to remove the job and its artifacts.
#[utoipa::path(
    delete,
    path = "/v1/jobs",
    tag = "Jobs",
    operation_id = "bulk_clear_jobs",
    summary = "Bulk clear terminal jobs",
    description = "Bulk-clear terminal jobs matching the given status filter.",
    params(BulkClearParams),
    responses(
        (status = 200, description = "Bulk clear completed", body = RemovedCount),
        (status = 400, description = "Bad request — invalid status parameter")
    )
)]
#[tracing::instrument(skip(state), fields(status = %params.status))]
pub(crate) async fn bulk_clear_jobs(
    State(state): State<AppState>,
    Query(params): Query<BulkClearParams>,
) -> Result<Json<RemovedCount>, AnvilError> {
    // Validate the status parameter — must be one of the four allowed values.
    // "all" is a special case that matches all terminal statuses; the other
    // three map directly to JobStatus variants.
    // Using AnvilError::Serde for the 400 response — it is the closest
    // existing variant that maps to BAD_REQUEST and accepts a String.
    let job_status = match params.status.as_str() {
        "completed" => Some(JobStatus::Completed),
        "failed" => Some(JobStatus::Failed),
        "cancelled" => Some(JobStatus::Cancelled),
        "all" => None, // None = no status filter → all terminal jobs
        other => {
            tracing::warn!(
                status = %other,
                "bulk_clear rejected: unrecognized status value"
            );
            return Err(AnvilError::Serde(format!(
                "invalid status: {other}; must be completed, failed, cancelled, or all"
            )));
        }
    };

    // List all jobs matching the status filter. When status is "all" (None),
    // the list() call returns every job — but we only want terminal ones.
    // Since the API contract says bulk clear operates on terminal jobs only,
    // we filter out Queued/Running jobs in the loop below as a safety guard.
    let job_store = JobStore::new(state.db.clone());
    let jobs = job_store.list(job_status, None).await?;

    let mut removed: u32 = 0;

    for job in &jobs {
        // Skip non-terminal jobs — only Completed, Failed, Cancelled are
        // eligible for bulk deletion. Queued and Running jobs must be
        // cancelled first. This is a safety guard: if "all" returns any
        // non-terminal jobs (shouldn't happen in normal operation), we
        // skip them rather than deleting active work.
        if matches!(job.status, JobStatus::Queued | JobStatus::Running) {
            tracing::debug!(
                job_id = %job.id,
                status = ?job.status,
                "bulk_clear skipping non-terminal job"
            );
            continue;
        }

        // Delegate to the shared delete_single_job helper. This ensures
        // bulk clear uses the exact same artifact-deletion-and-job-deletion
        // logic as the single-job handler — no divergence.
        match delete_single_job(&state, job.id, &job_store).await {
            Ok(_) => removed += 1,
            Err(e) => {
                tracing::warn!(
                    job_id = %job.id,
                    error = %e,
                    "bulk_clear: failed to delete job — continuing with remaining jobs"
                );
                // Continue processing remaining jobs even if one fails.
                // The removed count reflects only successfully deleted jobs.
            }
        }
    }

    tracing::info!(removed, status = %params.status, "bulk clear completed");
    Ok(Json(RemovedCount { removed }))
}
