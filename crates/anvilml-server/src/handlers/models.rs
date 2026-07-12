//! HTTP handlers for model registry operations.
//!
//! Provides `list_models()`, `get_model()`, and `rescan_models()` — thin-delegation
//! handlers that read from or write to the `model_store` field of `AppState` and
//! return JSON responses. No business logic lives here; all data access is delegated
//! to `anvilml_registry::ModelStore` and `anvilml_registry::trigger_model_scan`.

use anvilml_core::{AnvilError, ModelKind, ModelMeta};
use anvilml_registry::trigger_model_scan;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::Deserialize;
use tracing::instrument;

use crate::state::AppState;

/// Query parameters for the `GET /v1/models` list endpoint.
///
/// All fields are optional — when absent, the handler returns all models.
#[derive(Debug, Deserialize)]
pub struct ListModelsParams {
    /// Filter results to only models of this kind.
    ///
    /// When `None`, all models are returned regardless of kind.
    pub kind: Option<ModelKind>,
}

/// List all discovered models, optionally filtered by kind.
///
/// Returns a JSON array of `ModelMeta` objects. When the `kind` query
/// parameter is provided, only models matching that architecture family
/// are returned; otherwise all models are returned.
///
/// # Errors
///
/// Returns HTTP 500 if the database query fails.
#[instrument(skip(state), fields(kind = ?params.kind))]
pub async fn list_models(
    State(state): State<AppState>,
    Query(params): Query<ListModelsParams>,
) -> Result<Json<Vec<ModelMeta>>, AnvilError> {
    let models = state.model_store.list(params.kind).await?;
    Ok(Json(models))
}

/// Look up a single model by its ID.
///
/// Returns the `ModelMeta` for the model whose ID matches the `{id}`
/// path parameter. If no model with that ID exists, returns HTTP 404.
///
/// # Errors
///
/// Returns HTTP 404 (`AnvilError::ModelNotFound`) if no model with the
/// given ID exists in the registry. Returns HTTP 500 if the database
/// query fails.
#[instrument(skip(state), fields(model_id = %model_id))]
pub async fn get_model(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Result<Json<ModelMeta>, AnvilError> {
    match state.model_store.get(&model_id).await? {
        Some(meta) => Ok(Json(meta)),
        None => Err(AnvilError::ModelNotFound(model_id)),
    }
}

/// Trigger a background scan of all configured model directories.
///
/// Returns HTTP 202 Accepted immediately without waiting for the scan to
/// complete. The scan runs in a spawned tokio task and writes discovered
/// models into the `ModelStore` via the shared SQLite pool.
///
/// The scan iterates over `state.config.model_dirs`, calling
/// `ModelScanner::scan_dir()` for each entry. When `recursive` is `false`,
/// the depth is set to `0` (scan only the root directory); when
/// `recursive` is `true`, the entry's `max_depth` (or the config default
/// `model_scan_depth`) is used.
#[instrument(skip(state))]
pub async fn rescan_models(State(state): State<AppState>) -> impl IntoResponse {
    tracing::info!(
        dir_count = state.config.model_dirs.len(),
        "rescan triggered, scanning {} model dir(s)",
        state.config.model_dirs.len()
    );

    // Delegate to the shared scan trigger.
    // `trigger_model_scan()` spawns a fire-and-forget tokio task internally,
    // so the handler returns 202 immediately without awaiting. Errors are
    // logged at WARN level inside the spawned task rather than propagated.
    let pool = state.db.clone();
    let model_dirs = state.config.model_dirs.clone();
    let model_scan_depth = state.config.model_scan_depth;
    trigger_model_scan(pool, model_dirs, model_scan_depth);

    StatusCode::ACCEPTED
}
