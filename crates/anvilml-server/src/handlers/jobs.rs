//! Job handlers for `POST /v1/jobs`, `GET /v1/jobs`, `GET /v1/jobs/:id`,
//! `POST /v1/jobs/:id/cancel`, `DELETE /v1/jobs/:id`, and `DELETE /v1/jobs`.
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

/// Query parameters for the `bulk_clear` endpoint.
///
/// All fields are optional — the handler defaults to `"all"` when no
/// status filter is provided.
#[derive(Debug, Deserialize)]
pub struct BulkClearQuery {
    /// Filter by job status. Valid values: `"completed"`, `"failed"`,
    /// `"cancelled"`, or `"all"`. Defaults to `"all"` if omitted.
    pub status: Option<String>,
}

/// Cancel a job by its UUID.
///
/// Delegates to the `JobScheduler::cancel_job()` method which handles
/// cancellation differently based on the job's current status:
/// - Queued: immediately removes from queue and marks Cancelled.
/// - Running: sends a CancelJob IPC message to the owning worker.
/// - Terminal: returns 409 Conflict.
///
/// # Arguments
///
/// * `state` — Shared application state containing the job scheduler.
/// * `id` — The UUID of the job to cancel.
///
/// # Returns
///
/// * `202 Accepted` — cancellation accepted (queued or running job).
/// * `404 Not Found` — no job with the given ID exists.
/// * `409 Conflict` — job is in a terminal state.
#[tracing::instrument(skip(state), fields(job_id = %id))]
pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AnvilError> {
    // Delegate to the scheduler — it handles the full cancellation logic
    // including queue removal, IPC messaging, and status updates.
    // The scheduler maps terminal-state jobs to InvalidOperation (409)
    // and missing jobs to JobNotFound (404).
    state.scheduler.cancel_job(id).await?;

    Ok(StatusCode::ACCEPTED)
}

/// Delete a terminal job and its artifacts.
///
/// Only allows deletion of jobs in terminal states (Completed, Failed,
/// Cancelled). Deletes all associated artifact files from disk and the
/// job record from the database.
///
/// # Arguments
///
/// * `state` — Shared application state containing the job scheduler
///   and artifact store.
/// * `id` — The UUID of the job to delete.
///
/// # Returns
///
/// * `204 No Content` — job and artifacts deleted successfully.
/// * `404 Not Found` — no job with the given ID exists.
/// * `409 Conflict` — job is not in a terminal state.
#[tracing::instrument(skip(state), fields(job_id = %id))]
pub async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AnvilError> {
    // Look up the job to determine its status. Only terminal jobs can
    // be deleted — we don't want to accidentally remove active work.
    let job = state
        .scheduler
        .get_job(id)
        .await?
        .ok_or_else(|| AnvilError::JobNotFound(id.to_string()))?;

    // Check if the job is in a terminal state. Only Completed, Failed,
    // and Cancelled jobs can be deleted. Running or Queued jobs must
    // be cancelled first (via the cancel_job handler) before deletion.
    // This prevents accidental deletion of in-progress work.
    match job.status {
        JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
            // Terminal state — proceed with deletion.
        }
        other => {
            // Non-terminal state — cannot delete. The client should cancel
            // the job first if they want to remove it.
            return Err(AnvilError::InvalidOperation(format!(
                "cannot delete job in {:?} state; job must be terminal (Completed, Failed, or Cancelled)",
                other
            )));
        }
    }

    // Delete all artifacts associated with this job from disk.
    // We list artifacts by job ID, then delete each one. This ensures
    // no orphaned artifact files remain after the job is deleted.
    let artifacts = state.artifact_store.list(Some(id)).await?;
    for artifact in &artifacts {
        // Delete each artifact file from disk and remove its DB row.
        // If an artifact was already deleted (orphan), delete() will
        // return ArtifactNotFound which we map to a warning.
        if let Err(e) = state.artifact_store.delete(&artifact.hash).await {
            tracing::warn!(
                job_id = %id,
                hash = %artifact.hash,
                error = %e,
                "failed to delete artifact during job deletion"
            );
        }
    }

    // Delete the job row from the database. This is the final step —
    // after this, the job no longer exists and cannot be queried.
    sqlx::query("DELETE FROM jobs WHERE id = ?")
        .bind(id.to_string())
        .execute(&state.scheduler.db())
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// Bulk clear terminal jobs and their artifacts.
///
/// Deletes all jobs matching the given status filter (must be a terminal
/// status: completed, failed, cancelled, or all) along with their
/// artifact files from disk.
///
/// # Arguments
///
/// * `state` — Shared application state.
/// * `params` — Optional status filter.
///
/// # Returns
///
/// * `200 OK` with `{ "removed": u32 }` body.
/// * `400 Bad Request` — invalid status value.
#[tracing::instrument(skip(state, params))]
pub async fn bulk_clear(
    State(state): State<AppState>,
    Query(params): Query<BulkClearQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), AnvilError> {
    // Determine the status filter. If omitted, default to "all" which
    // clears all terminal jobs.
    let status = params.status.as_deref().unwrap_or("all");

    // Validate the status is one of the allowed values. We use a match
    // instead of a contains check to ensure exhaustive handling — any
    // unrecognised value is caught at compile time if we add new statuses.
    match status {
        "completed" | "failed" | "cancelled" | "all" => {
            // Valid status — proceed.
        }
        other => {
            // Invalid status — the client sent a value that is not a
            // recognised terminal status or "all". Return 400 so the
            // client knows to fix the query parameter.
            return Err(AnvilError::Internal(format!(
                "invalid status filter for bulk clear: {other}"
            )));
        }
    }

    // Fetch the IDs of jobs that will be deleted. We need these IDs
    // to delete their associated artifacts before removing the jobs.
    // We query for terminal jobs matching the status filter.
    let jobs_to_delete = match status {
        "all" => {
            // Fetch all terminal jobs — we need to delete artifacts for each.
            let mut all = Vec::new();
            for s in [
                JobStatus::Completed,
                JobStatus::Failed,
                JobStatus::Cancelled,
            ] {
                let jobs = state.scheduler.list_jobs(Some(s), None, None).await?;
                all.extend(jobs);
            }
            all
        }
        s => {
            // Fetch jobs for a single terminal status.
            let status_enum = match s {
                "completed" => JobStatus::Completed,
                "failed" => JobStatus::Failed,
                "cancelled" => JobStatus::Cancelled,
                _ => unreachable!("status already validated above"),
            };
            state
                .scheduler
                .list_jobs(Some(status_enum), None, None)
                .await?
        }
    };

    // Delete artifacts for each job. This must happen before the DB
    // deletion to ensure artifacts are cleaned up even if the DB
    // deletion fails (we can always re-run cleanup).
    for job in &jobs_to_delete {
        let artifacts = state.artifact_store.list(Some(job.id)).await?;
        for artifact in &artifacts {
            if let Err(e) = state.artifact_store.delete(&artifact.hash).await {
                tracing::warn!(
                    job_id = %job.id,
                    hash = %artifact.hash,
                    error = %e,
                    "failed to delete artifact during bulk clear"
                );
            }
        }
    }

    // Delete the jobs from the database. The scheduler handles the
    // actual SQL deletion and returns the count of affected rows.
    let count = state.scheduler.delete_jobs_by_status(status).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "removed": count })),
    ))
}
