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
use anvilml_core::types::worker::WorkerInfo;
use anvilml_worker::RestartOutcome;

use crate::AppState;

/// List all registered workers and their current lifecycle states.
///
/// Returns `200 OK` with a JSON array of `WorkerInfo` objects, one per
/// worker in the pool. The array is empty (not `null`) when no workers
/// have been spawned yet.
///
/// Per `ANVILML_DESIGN.md §13.4`: `GET /v1/workers → 200 [WorkerInfo, ...]`.
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `WorkerPool` through `state.workers`.
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
///
/// # Responses
///
/// * `202 Accepted` — the old generation exited (or was bounded-timed-out)
///   and a replacement has been spawned. Matches `cancel_job()`'s own
///   `202`-not-`200` framing (`ANVILML_DESIGN.md §13.5`): the new worker is
///   spawned, not necessarily `Ready`/`Idle` yet.
/// * `404 Not Found` — no worker with the given `id` exists in the pool.
/// * `409 Conflict` — that worker is already `Dying` (a shutdown, from
///   this same restart's own retry or from `shutdown_all()`, is already in
///   flight for it).
///
/// State is injected via `axum::extract::State<AppState>`, which provides
/// access to the `WorkerPool` through `state.workers` — `Arc<WorkerPool>`,
/// shared with the dispatch loop and event loop, so this handler never has
/// exclusive `&mut` access; `WorkerPool::restart_worker()` takes `&self`
/// for exactly this reason.
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
