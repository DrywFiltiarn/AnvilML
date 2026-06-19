//! Node type HTTP handlers.
//!
//! Provides `list_nodes` (GET /v1/nodes) for querying the current contents
//! of the node type registry — the set of node types registered by workers
//! that have reached `Ready`.

use axum::extract::State;
use axum::Json;

use anvilml_core::{AnvilError, NodeTypeDescriptor};

use crate::state::AppState;

/// GET /v1/nodes — list all registered node types.
///
/// Returns a JSON array of `NodeTypeDescriptor` objects, one per node type
/// registered by workers that have reached `Ready`. If no worker has ever
/// reached `Ready`, returns `503 Service Unavailable` with an error message.
///
/// A mock worker's `Ready` event reports an empty `node_types` list — this
/// is still a valid `Ready` event, so the endpoint returns `200 OK` with
/// an empty array `[]` after mock mode workers reach `Ready`.
///
/// # Returns
///
/// * `200 OK` with a JSON array of `NodeTypeDescriptor` objects (may be empty).
/// * `503 Service Unavailable` — no worker has ever reached `Ready`.
#[utoipa::path(
    get,
    path = "/v1/nodes",
    summary = "List registered node types",
    responses(
        (status = 200, description = "List of registered node types", body = Vec<NodeTypeDescriptor>),
        (status = 503, description = "No worker has reached Ready")
    ),
    tag = "nodes"
)]
pub async fn list_nodes(
    State(state): State<AppState>,
) -> Result<Json<Vec<NodeTypeDescriptor>>, AnvilError> {
    // Check if any worker has ever reached Ready. The registry's updated
    // flag is set on the first call to update_from_worker, regardless of
    // whether the worker reported any node types. This distinguishes
    // "no worker reached Ready" (503) from "worker reached Ready with
    // zero types" (200 with empty array).
    if !state.node_registry.has_been_updated().await {
        return Err(AnvilError::WorkersUnavailable(
            "no worker has reached Ready".to_string(),
        ));
    }

    // Fetch all registered node types. The order is not guaranteed
    // (hash map iteration order), but the caller should not depend
    // on ordering since this is a set of available capabilities.
    let types = state.node_registry.all_types().await;
    Ok(Json(types))
}
