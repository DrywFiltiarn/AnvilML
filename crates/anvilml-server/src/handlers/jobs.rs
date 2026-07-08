//! `POST /v1/jobs` handler — submit a computation graph for execution.
//!
//! Accepts a JSON body containing a graph definition and job settings,
//! delegates entirely to `JobScheduler::submit()`, and returns `202 Accepted`
//! with the job ID and queue position. Zero business logic — per
//! `ANVILML_DESIGN.md §3.3`, the HTTP layer is a thin delegation layer.

use anvilml_core::AnvilError;
use anvilml_core::JobSettings;
use axum::Json;
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
