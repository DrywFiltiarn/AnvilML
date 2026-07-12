//! HTTP handlers for model registry operations.
//!
//! Provides `list_models()`, `get_model()`, and `rescan_models()` — thin-delegation
//! handlers that read from or write to the `model_store` field of `AppState` and
//! return JSON responses. No business logic lives here; all data access is delegated
//! to `anvilml_registry::ModelStore` and `anvilml_registry::ModelScanner`.

use anvilml_core::{AnvilError, ModelKind, ModelMeta};
use anvilml_registry::ModelScanner;
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

    // Clone the SQLite pool from `state.db` — this is the same pool used by
    // `ModelStore`, so the scanner writes to the same database that the
    // list/get handlers read from. We clone rather than use `state.db` directly
    // because the spawned task needs to own the pool for its lifetime.
    let pool = state.db.clone();
    let model_dirs = state.config.model_dirs.clone();
    let model_scan_depth = state.config.model_scan_depth;

    // Spawn a fire-and-forget background task. The handler returns 202
    // immediately without awaiting the scan. Errors are logged at WARN
    // level rather than propagated, since the caller already got their
    // 202 response.
    tokio::spawn(async move {
        let scanner = ModelScanner::new(pool);

        for entry in &model_dirs {
            // When recursive is false, depth is 0 (scan root only).
            // When recursive is true, use the entry's max_depth or the
            // config default model_scan_depth.
            let depth = if entry.recursive {
                entry.max_depth.unwrap_or(model_scan_depth)
            } else {
                0
            };

            tracing::debug!(
                path = %entry.path.display(),
                depth,
                recursive = entry.recursive,
                "scanning model directory"
            );

            match scanner.scan_dir(&entry.path, depth).await {
                Ok(models) => {
                    tracing::debug!(
                        path = %entry.path.display(),
                        count = models.len(),
                        "scan complete for {}: {} models scanned",
                        entry.path.display(),
                        models.len()
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        path = %entry.path.display(),
                        error = %e,
                        "rescan failed for {}: {e}",
                        entry.path.display()
                    );
                }
            }
        }
    });

    StatusCode::ACCEPTED
}
