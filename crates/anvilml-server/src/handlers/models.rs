use axum::{extract::Query, extract::State, http::StatusCode, response::Json};
use serde::Deserialize;
use std::sync::Arc;

use anvilml_core::ModelKind;

use crate::state::AppState;

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
pub async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelsListQuery>,
) -> (StatusCode, Json<Vec<anvilml_core::ModelMeta>>) {
    match state.registry.list(query.kind).await {
        Ok(models) => (StatusCode::OK, Json(models)),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![])),
    }
}
