use axum::{extract::Path, extract::State, http::StatusCode, response::Json};
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

/// POST /v1/workers/{id}/restart handler.
///
/// Sends a shutdown to the specified worker, force-kills it, and respawns it.
pub async fn restart_worker(
    State(state): State<Arc<crate::App>>,
    Path(worker_id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    let workers = match &state.workers {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "workers_not_configured",
                    "message": "worker pool not available",
                })),
            );
        }
    };

    match workers.restart(&worker_id, &state.config).await {
        Ok(()) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "status": "restarting",
                "worker_id": worker_id,
            })),
        ),
        Err(anvilml_core::AnvilError::WorkerDead(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "message": format!("worker {} not found", worker_id),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "restart_failed",
                "message": e.to_string(),
            })),
        ),
    }
}
