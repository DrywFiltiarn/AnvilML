//! Worker listing HTTP handler.
//!
//! Provides `list_workers` (GET /v1/workers) for querying the current state
//! of all spawned Python worker subprocesses.

use axum::extract::State;
use axum::Json;

use anvilml_core::WorkerInfo;

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
