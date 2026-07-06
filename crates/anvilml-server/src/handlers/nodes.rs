//! `/v1/nodes` handler — list all registered node types.

use anvilml_core::NodeTypeDescriptor;
use axum::Json;
use axum::extract::State;

use crate::AppState;

/// List all registered node type descriptors.
///
/// Returns `200 OK` with a JSON array of `NodeTypeDescriptor` objects,
/// one per registered node type. The array is empty (not `null`) when
/// no nodes have been registered yet.
///
/// Per `ANVILML_DESIGN.md §13.4`: `GET /v1/nodes → 200 [NodeTypeDescriptor, ...]`.
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `NodeTypeRegistry` through `state.node_registry`.
pub async fn list_nodes(State(state): State<AppState>) -> Json<Vec<NodeTypeDescriptor>> {
    // Delegate to the registry's list() method which acquires a read lock
    // and returns a Vec<NodeTypeDescriptor>. Returns an empty vector when
    // the registry has no registered types.
    Json(state.node_registry.list())
}
