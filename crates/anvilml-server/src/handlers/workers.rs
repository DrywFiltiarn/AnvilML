use axum::{extract::State, http::StatusCode, response::Json};
use std::sync::Arc;

use anvilml_core::WorkerInfo;

/// GET /v1/workers handler.
///
/// Returns a JSON array of `WorkerInfo` objects, one per worker in the pool.
pub async fn list_workers(
    State(state): State<Arc<crate::App>>,
) -> (StatusCode, Json<Vec<WorkerInfo>>) {
    match &state.workers {
        Some(pool) => {
            let infos = pool.list().await;
            (StatusCode::OK, Json(infos))
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(vec![])),
    }
}
