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
#[utoipa::path(
    get,
    path = "/v1/nodes",
    tag = "Nodes",
    operation_id = "list_nodes",
    summary = "List node types",
    description = "Lists all registered Python worker node type descriptors.",
    responses(
        (status = 200, description = "List of node types", body = Vec<NodeTypeDescriptor>)
    )
)]
pub async fn list_nodes(State(state): State<AppState>) -> Json<Vec<NodeTypeDescriptor>> {
    // Delegate to the registry's list() method which acquires a read lock
    // and returns a Vec<NodeTypeDescriptor>. Returns an empty vector when
    // the registry has no registered types.
    Json(state.node_registry.list())
}
