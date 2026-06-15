//! Model metadata HTTP handlers.
//!
//! Provides `list_models` (GET /v1/models) and `get_model` (GET /v1/models/:id)
//! for querying the model registry via HTTP.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use anvilml_core::AnvilError;
use anvilml_core::ModelKind;
use anvilml_core::ModelMeta;

use crate::state::AppState;

/// Query parameter filter for listing models.
///
/// Accepts an optional `kind` query parameter that filters results to
/// models of the specified kind. Uses `snake_case` deserialization to
/// match the `ModelKind` Display/FromStr convention (e.g. `?kind=diffusion`).
#[derive(Deserialize)]
pub(crate) struct ModelsFilter {
    /// Filter models by kind. If `None`, all models are returned.
    kind: Option<ModelKind>,
}

/// GET /v1/models — list all models, optionally filtered by kind.
///
/// Extracts `State<AppState>` and an optional `?kind=` query parameter.
/// Calls `ModelStore::list()` with the optional kind filter and returns
/// the result as a JSON array.
///
/// # Query parameters
///
/// * `kind` — Optional. Filters results to models of the given kind.
///   Accepts snake_case values: `diffusion`, `text_encoder`, `vae`,
///   `lora`, `controlnet`, `upscale`, `unknown`.
///
/// # Returns
///
/// * `200 OK` with a JSON array of `ModelMeta` objects.
pub(crate) async fn list_models(
    State(state): State<AppState>,
    Query(filter): Query<ModelsFilter>,
) -> Result<Json<Vec<ModelMeta>>, AnvilError> {
    // Delegate to the model store's list method. The optional kind filter
    // is passed through; when None the store returns all models.
    let models = state.registry.list(filter.kind).await?;

    Ok(Json(models))
}

/// GET /v1/models/:id — retrieve a single model by its ID.
///
/// Extracts `State<AppState>` and the model ID from the URL path.
/// Calls `ModelStore::get()` and returns the model as JSON if found,
/// or a 404 error if the model does not exist.
///
/// # Path parameters
///
/// * `id` — The model's unique identifier (SHA256 hex digest).
///
/// # Returns
///
/// * `200 OK` with a JSON `ModelMeta` object.
/// * `404 Not Found` with an `AnvilError::ModelNotFound` body if the
///   model ID does not exist in the registry.
pub(crate) async fn get_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ModelMeta>, AnvilError> {
    // Delegate to the model store's get method. On Ok(Some(meta)) we
    // return the model as JSON; on Ok(None) we return a 404 error
    // using the ModelNotFound variant which maps to 404 via IntoResponse.
    match state.registry.get(&id).await? {
        Some(meta) => Ok(Json(meta)),
        None => Err(AnvilError::ModelNotFound(id)),
    }
}
