//! `GET /v1/workers` handler — list all Python worker subprocesses.
//!
//! Returns the current state of every worker in the pool as a JSON array of
//! `WorkerInfo` objects. Delegates to `WorkerPool::list()` which zips each
//! `WorkerHandle` with its corresponding `GpuDevice` to construct the info
//! structs.
//!
//! Per `ANVILML_DESIGN.md §13.4`: `GET /v1/workers → 200 Vec<WorkerInfo>`.
//!
//! Also `POST /v1/workers/{id}/restart` (P18-D3) — see `restart_worker()`'s
//! own doc comment.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use anvilml_core::AnvilError;
use anvilml_core::WorkerInfo;
use anvilml_worker::RestartOutcome;

use crate::AppState;

/// List all registered workers and their current lifecycle states.
///
/// Returns `200 OK` with a JSON array of `WorkerInfo` objects, one per
/// worker in the pool. The array is empty (not `null`) when no workers
/// have been spawned yet.
#[utoipa::path(
    get,
    path = "/v1/workers",
    tag = "Workers",
    operation_id = "list_workers",
    summary = "List workers",
    description = "Lists all registered Python workers and their current lifecycle states.",
    responses(
        (status = 200, description = "List of workers", body = Vec<WorkerInfo>)
    )
)]
pub(crate) async fn list_workers(State(state): State<AppState>) -> Json<Vec<WorkerInfo>> {
    // Delegate to WorkerPool::list() which acquires the internal lock on
    // each WorkerHandle's status, zips handles with their GpuDevice
    // metadata, and constructs a Vec<WorkerInfo>. Returns an empty vector
    // when the pool has no workers.
    Json(state.workers.list().await)
}

/// Restart the worker identified by `id`: request its graceful shutdown,
/// wait for it to exit, then spawn a fresh replacement into the same
/// device slot (P18-D3).
///
/// This is **not** just `request_shutdown()` — an earlier premise (that
/// the pool's crash-respawn machinery would restart a gracefully-shut-down
/// worker on its own) was audited and found false: `request_shutdown()`
/// drives the worker into a terminal exit, not a respawn. See
/// `WorkerPool::restart_worker()`'s own doc comment for the full audit
/// finding and why this handler's real work happens there, not here.
#[utoipa::path(
    post,
    path = "/v1/workers/{id}/restart",
    tag = "Workers",
    operation_id = "restart_worker",
    summary = "Restart a worker",
    description = "Restarts the worker identified by ID: graceful shutdown followed by respawn into the same device slot.",
    params(
        ("id" = String, Path, description = "Worker ID")
    ),
    responses(
        (status = 202, description = "Restart accepted"),
        (status = 404, description = "Worker not found"),
        (status = 409, description = "Worker already shutting down")
    )
)]
#[tracing::instrument(skip(state), fields(worker_id = %id))]
pub(crate) async fn restart_worker(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AnvilError> {
    match state.workers.restart_worker(&id).await? {
        RestartOutcome::Accepted(handle) => {
            tracing::info!(worker_id = %handle.worker_id, "restart accepted");
            Ok(StatusCode::ACCEPTED)
        }
        RestartOutcome::NotFound => {
            tracing::debug!(worker_id = %id, "restart: worker not found");
            Err(AnvilError::WorkerNotFound(id))
        }
        RestartOutcome::Conflict => {
            tracing::debug!(worker_id = %id, "restart rejected: already shutting down");
            Ok(StatusCode::CONFLICT)
        }
    }
}
