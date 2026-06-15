//! Model metadata HTTP handlers.
//!
//! Provides `list_models` (GET /v1/models) and `get_model` (GET /v1/models/:id)
//! for querying the model registry via HTTP.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

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

/// POST /v1/models/rescan — trigger a model directory rescan.
///
/// Responds with HTTP 202 Accepted immediately, then spawns a background
/// task that scans all configured model directories (from `AppState::model_dirs`)
/// and upserts discovered models into the registry. The HTTP thread is not
/// blocked during the (potentially slow) directory scan.
///
/// The scanner logs completion at INFO with `count=` and `dir=` fields.
/// Errors during the background scan are logged at ERROR.
///
/// # Returns
///
/// * `202 Accepted` with `{"status": "scanning"}` body.
pub(crate) async fn rescan_models(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    // Clone model_dirs for the background task. PathBuf clone is cheap
    // (just a pointer + length + capacity copy) and the vec is typically
    // small (< 10 entries), so this is O(n) with negligible overhead.
    let model_dirs = state.model_dirs.clone();

    // Clone the registry Arc for the background task. Arc::clone is
    // a cheap pointer increment, not a deep copy.
    let registry = state.registry.clone();

    // Spawn a fire-and-forget background task for the scan.
    // The 202 response is already sent to the client — this task
    // runs independently. Tokio panics in the spawned task are
    // captured by the JoinHandle, but we intentionally discard it
    // because the tracing::error! log on failure provides observability.
    // This follows the fire-and-forget pattern from ANVILML_DESIGN.md §4.7.
    tokio::spawn(async move {
        // Use a mutable binding to capture the count from scan_and_upsert.
        // The scanner already logs the mandatory INFO "model scan completed"
        // log point with count= and dir= fields (ENVIRONMENT.md §9).
        match registry.scan_and_upsert(&model_dirs).await {
            Ok(count) => {
                // Join the directory paths into a single string for the
                // structured log field. This is the same dirs_string pattern
                // used by the startup scan in main.rs.
                let dirs_string: Vec<String> = model_dirs
                    .iter()
                    .map(|d| d.path.to_string_lossy().into_owned())
                    .collect();

                tracing::info!(
                    count = count,
                    dir = %dirs_string.join(","),
                    "rescan completed"
                );
            }
            Err(e) => {
                // Log the error so the operator knows the rescan failed.
                // The scanner's INFO log may or may not have fired depending
                // on whether the error occurred before or after scan() returned.
                tracing::error!(error = %e, "rescan failed");
            }
        }
    });

    // Respond immediately with 202 Accepted. The client knows the scan
    // is in progress and can poll GET /v1/models to see results.
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"status": "scanning"})),
    )
}
