//! Job handlers for `POST /v1/jobs`, `GET /v1/jobs`, and `GET /v1/jobs/:id`.
//!
//! These handlers delegate all job operations to the `JobScheduler` which
//! owns the job queue, VRAM ledger, and SQLite persistence.

use crate::state::AppState;
use anvilml_core::types::{Job, JobStatus, SubmitJobRequest, SubmitJobResponse};
use anvilml_core::AnvilError;
use axum::extract::{Json, Path, Query, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

/// Query parameters for the `list_jobs` endpoint.
///
/// All fields are optional — the handler builds a dynamic SQL query with
/// only the provided filters.
#[derive(Debug, Deserialize)]
pub struct ListJobsQuery {
    /// Filter by job status. Valid values: `"queued"`, `"running"`,
    /// `"completed"`, `"failed"`, `"cancelled"`.
    pub status: Option<String>,
    /// Maximum number of jobs to return (1-based limit).
    pub limit: Option<u32>,
    /// Only return jobs created strictly before this RFC 3339 timestamp.
    pub before: Option<String>,
}

/// Submit a new job for execution.
///
/// Delegates to the `JobScheduler::submit()` method which validates the
/// computation graph against the node type registry, persists the job to
/// SQLite, enqueues it for dispatch, and broadcasts a `JobQueued` WebSocket
/// event.
///
/// # Arguments
///
/// * `state` — Shared application state containing the job scheduler.
/// * `req` — The job submission request containing the graph JSON and
///   optional settings (device preference, etc.).
///
/// # Returns
///
/// * `202 Accepted` — graph is valid, job persisted and queued.
/// * `422 Unprocessable Entity` — graph validation failed (unknown node
///   types, duplicate IDs, invalid edges, cycles, slot mismatches).
/// * `500 Internal Server Error` — database or serialization error.
#[tracing::instrument(skip(state, req), fields(graph_nodes = ?req.graph.get("nodes").and_then(|n| n.get("len").map(|l| l.as_u64()))))]
pub async fn submit_job(
    State(state): State<AppState>,
    Json(req): Json<SubmitJobRequest>,
) -> Result<(StatusCode, Json<SubmitJobResponse>), AnvilError> {
    // Delegate to the scheduler — it handles validation, persistence,
    // queueing, and event broadcasting. The scheduler owns the job
    // lifecycle; the handler is purely a translation layer between
    // HTTP and the scheduler API.
    let response = state.scheduler.submit(req).await?;

    // Return 202 Accepted with the scheduler's response containing the
    // real job ID and queue position.
    Ok((StatusCode::ACCEPTED, Json(response)))
}

/// List jobs with optional filters.
///
/// Queries the job database for jobs matching the provided filters
/// (status, limit, before timestamp). Returns results ordered by
/// `created_at` descending (most recent first).
///
/// # Arguments
///
/// * `state` — Shared application state containing the job scheduler.
/// * `params` — Optional filters: `status` (string enum), `limit` (u32),
///   `before` (RFC 3339 timestamp string).
///
/// # Returns
///
/// * `200 OK` with a JSON array of matching jobs.
/// * `400 Bad Request` if `status` or `before` cannot be parsed.
#[tracing::instrument(skip(state, params))]
pub async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<ListJobsQuery>,
) -> Result<Json<Vec<Job>>, AnvilError> {
    // Parse the optional status filter. The scheduler expects a
    // JobStatus enum; we convert from the query string here so the
    // handler can return a 400 for invalid status values.
    let status = match params.status.as_deref() {
        None => None,
        Some("queued") => Some(JobStatus::Queued),
        Some("running") => Some(JobStatus::Running),
        Some("completed") => Some(JobStatus::Completed),
        Some("failed") => Some(JobStatus::Failed),
        Some("cancelled") => Some(JobStatus::Cancelled),
        Some(other) => {
            // Invalid status string — the client sent a value that is
            // not a recognised job status. Return 400 so the client
            // knows to fix the query parameter.
            return Err(AnvilError::Internal(format!(
                "invalid status filter: {other}"
            )));
        }
    };

    // Parse the optional before timestamp. The scheduler expects a
    // DateTime<Utc>; we parse from the RFC 3339 query string here so
    // the handler can return a 400 for invalid timestamps.
    let before = match params.before {
        None => None,
        Some(ref ts) => Some(
            DateTime::parse_from_rfc3339(ts)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    // Invalid RFC 3339 timestamp — the client sent a value
                    // that cannot be parsed. Return 400 with the parse error
                    // so the client can diagnose the format issue.
                    AnvilError::Internal(format!("invalid before timestamp: {e}"))
                })?,
        ),
    };

    // Delegate to the scheduler which builds the dynamic SQL query with
    // the provided filters. The scheduler returns results ordered by
    // created_at DESC (most recent first).
    let jobs = state
        .scheduler
        .list_jobs(status, params.limit, before)
        .await?;

    Ok(Json(jobs))
}

/// Get a single job by its UUID.
///
/// Queries the job database for a job with the given ID. Returns 404
/// if no matching job exists.
///
/// # Arguments
///
/// * `state` — Shared application state containing the job scheduler.
/// * `id` — The UUID of the job to look up.
///
/// # Returns
///
/// * `200 OK` with the job JSON body.
/// * `404 Not Found` if no job with the given ID exists.
#[tracing::instrument(skip(state), fields(job_id = %id))]
pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, AnvilError> {
    // Delegate to the scheduler which queries the database. The scheduler
    // returns None for missing jobs; the handler translates that to a 404
    // response via AnvilError::JobNotFound.
    match state.scheduler.get_job(id).await? {
        Some(job) => Ok(Json(job)),
        None => Err(AnvilError::JobNotFound(id.to_string())),
    }
}
