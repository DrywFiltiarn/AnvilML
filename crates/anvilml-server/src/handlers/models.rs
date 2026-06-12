use axum::{extract::Path, extract::Query, extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use anvilml_core::ModelKind;

use crate::App;

/// Query parameters for the models list endpoint.
#[derive(Debug, Deserialize)]
pub struct ModelsListQuery {
    /// Optional kind filter — only return models of this kind.
    pub kind: Option<ModelKind>,
}

/// GET /v1/models handler.
///
/// Returns a JSON array of all scanned model metadata, optionally filtered
/// by `kind`. Delegates to `registry.list(kind)` from the application state.
#[utoipa::path(
    get,
    path = "/v1/models",
    summary = "List scanned models",
    params(
        ("kind" = Option<ModelKind>, Query, description = "Filter by model kind")
    ),
    responses(
        (status = 200, description = "Model list", body = Vec<anvilml_core::ModelMeta>)
    )
)]
pub async fn list_models(
    State(state): State<Arc<App>>,
    Query(query): Query<ModelsListQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.registry.list(query.kind).await {
        Ok(models) => (StatusCode::OK, Json(serde_json::to_value(&models).unwrap())),
        Err(e) => {
            tracing::error!(error = %e, "list_models: registry query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "internal_error",
                    "message": e.to_string()
                })),
            )
        }
    }
}

/// GET /v1/models/:id handler.
///
/// Returns the model metadata for a single model identified by its ID.
/// Returns 200 with the model on success, or 404 with an error JSON body
/// when no model with the given ID exists.
#[utoipa::path(
    get,
    path = "/v1/models/{id}",
    summary = "Get a model by ID",
    params(
        ("id" = String, Path, description = "Model ID")
    ),
    responses(
        (status = 200, description = "Model found", body = anvilml_core::ModelMeta),
        (status = 404, description = "Model not found")
    )
)]
pub async fn get_model(
    State(state): State<Arc<App>>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.registry.get(&id).await {
        Ok(Some(meta)) => (StatusCode::OK, Json(serde_json::to_value(&meta).unwrap())),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "message": "model not found"
            })),
        ),
        Err(e) => {
            tracing::error!(error = %e, "get_model: registry query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "internal_error",
                    "message": e.to_string()
                })),
            )
        }
    }
}

/// POST /v1/models/rescan handler.
///
/// Triggers a background model directory rescan. The handler returns 202
/// Accepted immediately; the actual scanning work is performed by a spawned
/// tokio task.
#[utoipa::path(
    post,
    path = "/v1/models/rescan",
    summary = "Trigger model directory rescan",
    responses(
        (status = 202, description = "Rescan started", body = RescanResponse)
    )
)]
pub async fn rescan_models(State(state): State<Arc<App>>) -> (StatusCode, Json<RescanResponse>) {
    let dirs = state.model_dirs.clone();
    let registry = Arc::clone(&state.registry);

    tokio::spawn(async move {
        match registry.rescan(&dirs).await {
            Ok(count) => tracing::info!(models_scanned = count, "background rescan complete"),
            Err(e) => tracing::warn!("background rescan failed: {e}"),
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(RescanResponse {
            status: "rescan_started",
        }),
    )
}

/// Response body for POST /v1/models/rescan.
#[derive(Debug, Serialize, ToSchema)]
pub struct RescanResponse {
    /// Rescan status — always "rescan_started".
    pub status: &'static str,
}
