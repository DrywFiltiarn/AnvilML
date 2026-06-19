//! Typed error types for DAG validation.
//!
//! This module defines `GraphError`, a strongly-typed enum covering all graph
//! validation failure modes, replacing the previous `Vec<String>` error
//! collection with structured error values. Internal check functions return
//! `Vec<GraphError>`; the top-level `validate_graph` converts to strings
//! only at the return point, preserving the public API.

use std::fmt;

use anvilml_core::SlotType;

/// Typed error for DAG validation failures.
///
/// Each variant corresponds to one of the validation checks performed
/// by `validate_graph`. The structural nodes-array check is not included
/// here because it has no typed equivalent — it only checks that the
/// `"nodes"` field exists and is an array.
///
/// Variants:
/// * `UnknownNodeType` — a node references a type not in the registry.
/// * `DuplicateNodeId` — two or more nodes share the same `"id"`.
/// * `UnknownEdgeRef` — an edge references a missing node or a missing
///   output slot on an existing node.
/// * `SlotTypeMismatch` — an edge connects incompatible slot types.
/// * `CycleDetected` — the graph contains a directed cycle.
#[derive(Debug, Clone)]
pub enum GraphError {
    /// A node type was not found in the `NodeTypeRegistry`.
    ///
    /// The `type_name` field holds the exact string from the graph's
    /// `"type"` field that failed lookup.
    UnknownNodeType(String),

    /// Two or more nodes share the same `"id"` value.
    ///
    /// The `id` field holds the duplicated identifier.
    DuplicateNodeId(String),

    /// An edge references a node or slot that does not exist.
    ///
    /// `node_id` is the edge's `"node_id"` field. `slot` is the
    /// missing output slot name (empty string for missing nodes,
    /// the actual slot name for missing slots).
    UnknownEdgeRef { node_id: String, slot: String },

    /// An edge connects slot types that are incompatible.
    ///
    /// `from` is the source output slot's type; `to` is the target
    /// input slot's type. Neither is `SlotType::Any`, and they do
    /// not match exactly.
    SlotTypeMismatch { from: SlotType, to: SlotType },

    /// The graph contains a directed cycle.
    ///
    /// The `nodes` vector contains the IDs of all nodes participating
    /// in the cycle, in deterministic order (Kahn's algorithm with
    /// sorted initial queue).
    CycleDetected(Vec<String>),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // For UnknownNodeType, use node_id as the type_name since
            // check_node_types passes both as the same string.
            GraphError::UnknownNodeType(type_name) => {
                write!(
                    f,
                    "validation failed: node \"{type_name}\" has unknown type \"{type_name}\""
                )
            }
            GraphError::DuplicateNodeId(id) => {
                write!(f, "validation failed: duplicate node id \"{id}\"")
            }
            GraphError::UnknownEdgeRef { node_id, slot } => {
                if slot.is_empty() {
                    // Missing node — produce the same message as the
                    // original check_edge_refs "missing source node" error.
                    write!(
                        f,
                        "validation failed: edge references missing source node \"{node_id}\""
                    )
                } else {
                    // Missing output slot on an existing node.
                    write!(
                        f,
                        "validation failed: node \"{node_id}\" has no output slot \"{slot}\""
                    )
                }
            }
            GraphError::SlotTypeMismatch { from, to } => {
                // SlotType does not implement Display, only Debug.
                // Debug format produces PascalCase names (e.g. "Model",
                // "Image") which is what the existing tests assert on.
                write!(
                    f,
                    "validation failed: slot type mismatch on edge from \
                     (..) ({from:?}) to (..) ({to:?})"
                )
            }
            GraphError::CycleDetected(nodes) => {
                write!(
                    f,
                    "validation failed: cycle detected involving nodes: {}",
                    nodes.join(", ")
                )
            }
        }
    }
}

/// Re-export of `ValidatedGraph` from the `dag` module.
///
/// Placed here so downstream crates can import it via
/// `anvilml_scheduler::ValidatedGraph` instead of
/// `anvilml_scheduler::dag::ValidatedGraph`.
pub use crate::dag::ValidatedGraph;
