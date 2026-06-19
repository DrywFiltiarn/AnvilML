//! Job submission handler for `POST /v1/jobs`.
//!
//! Validates the submitted computation graph against the node type registry
//! and returns a placeholder job ID. Full job persistence and dispatch are
//! deferred to Phase 013.

use crate::state::AppState;
use anvilml_core::types::{SubmitJobRequest, SubmitJobResponse};
use anvilml_core::AnvilError;
use axum::extract::{Json, State};
use axum::http::StatusCode;
use serde_json::Value;
use uuid::Uuid;

/// Submit a new job for execution.
///
/// Validates the submitted computation graph against the registered node
/// types. If no workers have ever reported (empty registry), returns 503.
/// If validation fails, returns 422 with the list of validation errors.
/// If the graph is valid, returns 202 with a placeholder job ID.
///
/// # Arguments
///
/// * `state` — Shared application state containing the node type registry.
/// * `req` — The job submission request containing the graph JSON and
///   optional settings (device preference, etc.).
///
/// # Returns
///
/// * `503 Service Unavailable` — no workers have reported Ready yet.
/// * `422 Unprocessable Entity` — graph validation failed (unknown node
///   types, duplicate IDs, invalid edges, cycles, slot mismatches).
/// * `202 Accepted` — graph is valid, job queued with a placeholder ID.
#[tracing::instrument(skip(state, req), fields(graph_nodes = ?req.graph.get("nodes").and_then(|n| n.get("len").map(|l| l.as_u64()))))]
pub async fn submit_job(
    State(state): State<AppState>,
    Json(req): Json<SubmitJobRequest>,
) -> Result<(StatusCode, Json<SubmitJobResponse>), AnvilError> {
    // Check if any worker has ever reached Ready. An empty registry means
    // no worker has connected yet — we cannot validate the graph without
    // knowing what node types exist.
    if state.node_registry.is_empty().await {
        return Err(AnvilError::WorkersUnavailable(
            "no workers available to validate graph".into(),
        ));
    }

    // Validate the graph against the node type registry. This performs six
    // independent checks (nodes array, duplicate IDs, type registration,
    // edge references, slot compatibility, acyclicity) and collects all
    // errors before returning — the client gets the full diagnostic picture
    // in a single response.
    //
    // Auto-deref from &Arc<NodeTypeRegistry> to &NodeTypeRegistry —
    // clippy's explicit-auto-deref lint forbids &* so we rely on the
    // compiler's automatic deref coercion.
    let graph: Value = req.graph;
    if let Err(errors) = anvilml_scheduler::dag::validate_graph(&graph, &state.node_registry).await
    {
        // Validation failed — return all error messages as a 422 response.
        // The IntoResponse impl for AnvilError::InvalidGraph will produce
        // the correct 422 status code (changed from 400 in this task).
        return Err(AnvilError::InvalidGraph(errors));
    }

    // Graph is valid — return a placeholder 202 response.
    // Full job persistence and dispatch are deferred to Phase 013.
    // Uuid::new_v4() generates a unique identifier for correlation.
    Ok((
        StatusCode::ACCEPTED,
        Json(SubmitJobResponse {
            job_id: Uuid::new_v4(),
            queue_position: 0,
        }),
    ))
}
