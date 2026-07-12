//! HTTP handlers for model registry operations.
//!
//! Provides `list_models()` and `get_model()` — thin-delegation handlers that
//! read from the `model_store` field of `AppState` and return JSON responses.
//! No business logic lives here; all data access is delegated to
//! `anvilml_registry::ModelStore`.

use anvilml_core::{AnvilError, ModelKind, ModelMeta};
use axum::extract::{Path, Query, State};
use axum::response::Json;
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
