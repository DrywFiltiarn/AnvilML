//! Worker HTTP handlers.
//!
//! Provides `list_workers` (GET /v1/workers) for querying the current state
//! of all spawned Python worker subprocesses, and `restart_worker`
//! (POST /v1/workers/:id/restart) for requesting an unconditional restart
//! of a specific worker.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use anvilml_core::{AnvilError, WorkerInfo};

use crate::state::AppState;

/// GET /v1/workers — list all workers and their current states.
///
/// Returns a JSON array of `WorkerInfo` objects, one per worker in the pool.
/// If no worker pool is configured (e.g. in test/stub mode), returns an
/// empty JSON array `[]`.
///
/// # Returns
///
/// * `200 OK` with a JSON array of `WorkerInfo` objects.
#[utoipa::path(
    get,
    path = "/v1/workers",
    summary = "List all workers",
    responses(
        (status = 200, description = "List of workers", body = Vec<WorkerInfo>)
    ),
    tag = "workers"
)]
pub async fn list_workers(State(state): State<AppState>) -> Json<Vec<WorkerInfo>> {
    // Return an empty array when no worker pool is configured.
    // This handles test/stub mode where AppState::new() is used.
    // In production, state.workers is always Some(pool).
    match state.workers {
        Some(pool) => Json(pool.get_worker_infos().await),
        None => Json(vec![]),
    }
}

/// POST /v1/workers/:id/restart — request an unconditional restart of a
/// specific worker subprocess.
///
/// Signals the named worker's `run()` loop to force-kill its subprocess (if
/// still running), then respawn it immediately — bypassing `RespawnPolicy`'s
/// attempt limit and backoff delay entirely. Valid from any current worker
/// state, including an already-`Dead` worker whose automatic respawn attempts
/// were exhausted.
///
/// Returns `202 Accepted` immediately after the restart signal is sent; the
/// respawn happens asynchronously. Callers can observe the transition through
/// `Dead` → `Respawning` → `Initializing` → `Idle` via `GET /v1/workers` or
/// the `WorkerStatusChanged` WebSocket event.
///
/// # Path parameters
///
/// * `id` — The worker's stable display identity (e.g. `"worker-0"`).
///
/// # Returns
///
/// * `202 Accepted` — restart signal delivered.
/// * `404 Not Found` — no worker with the given id exists in the pool.
/// * `500 Internal Server Error` — the worker's `run()` task has already
///   exited (e.g. it crashed unrecoverably before this request arrived).
/// * `503 Service Unavailable` — no worker pool is configured (test/stub mode).
#[utoipa::path(
    post,
    path = "/v1/workers/{id}/restart",
    summary = "Restart a worker",
    params(
        ("id" = String, Path, description = "Worker ID (e.g. \"worker-0\")")
    ),
    responses(
        (status = 202, description = "Restart signal delivered"),
        (status = 404, description = "Worker not found"),
        (status = 500, description = "Worker run loop has already exited"),
        (status = 503, description = "No worker pool configured"),
    ),
    tag = "workers"
)]
pub async fn restart_worker(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AnvilError> {
    let pool = state
        .workers
        .ok_or_else(|| AnvilError::WorkersUnavailable("no worker pool configured".to_string()))?;

    pool.restart_worker(&id).await?;

    Ok(StatusCode::ACCEPTED)
}
