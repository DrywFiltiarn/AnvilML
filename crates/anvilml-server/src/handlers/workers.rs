//! `GET /v1/workers` handler — list all Python worker subprocesses.
//!
//! Returns the current state of every worker in the pool as a JSON array of
//! `WorkerInfo` objects. Delegates to `WorkerPool::list()` which zips each
//! `WorkerHandle` with its corresponding `GpuDevice` to construct the info
//! structs.
//!
//! Per `ANVILML_DESIGN.md §13.4`: `GET /v1/workers → 200 Vec<WorkerInfo>`.

use axum::Json;
use axum::extract::State;

use anvilml_core::types::worker::WorkerInfo;

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
