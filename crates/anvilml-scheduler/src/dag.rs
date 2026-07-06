/// DAG graph validation for job submission.
///
/// Implements the collect-all-errors validation pipeline defined in
/// ANVILML_DESIGN.md §12.3. This task covers checks 1–2:
/// (1) root is an object with a "nodes" array,
/// (2) no duplicate node id values.
///
/// Checks 3–6 (unknown node types, dangling edges, slot-type
/// compatibility, cycle detection) are added by subsequent tasks
/// in Phase 12.
///
/// # Arguments
///
/// * `graph` — A JSON object representing the job graph. Must contain
///   a `"nodes"` array with objects having an `"id"` field.
/// * `_registry` — The node type registry, used for checks 3–6.
///   **defers_to: P12-A4 — registry used for checks 3-6 (unknown node types,
///   dangling edges, slot compatibility, cycle detection)**
///
/// # Returns
///
/// `Ok(ValidatedGraph)` if checks 1–2 pass with zero errors, or
/// `Err(Vec<GraphError>)` containing all collected errors (collect-all-errors
/// semantics: never short-circuit on the first error, except where a
/// violation makes further checking structurally meaningless).
use std::collections::HashSet;

use anvilml_core::NodeTypeRegistry;
use serde_json::Value;

use crate::types::{GraphError, ValidatedGraph};

/// Validate a DAG graph: structural root check and duplicate-ID check.
///
/// Runs checks 1–2 of the six-check validation pipeline. The non-object
/// root case is the only early return permitted by the collect-all-errors
/// contract — if the root is not an object there is no "nodes" key to
/// inspect, so further checks would be meaningless.
///
/// # Collect-all-errors behaviour
///
/// Duplicate node IDs are all collected into a single `Err(Vec)` — the
/// iteration continues past the first duplicate rather than returning
/// immediately. This lets callers report all structural problems at once.
pub fn validate_graph(
    graph: Value,
    _registry: &NodeTypeRegistry,
) -> Result<ValidatedGraph, Vec<GraphError>> {
    // Check 1a: root must be a JSON object. A non-object root has no
    // "nodes" key to inspect, so no further checks are meaningful.
    if !graph.is_object() {
        return Err(vec![GraphError::NotAnObject]);
    }

    let obj = graph.as_object().unwrap(); // safe: just checked is_object()

    // Check 1b: the object must contain a "nodes" key whose value is an array.
    let nodes = match obj.get("nodes") {
        Some(Value::Array(nodes)) => nodes,
        _ => return Err(vec![GraphError::MissingNodesArray]),
    };

    // Check 2: no duplicate node id values. Iterate the entire array and
    // collect every duplicate occurrence — do not stop at the first one.
    let mut seen_ids = HashSet::new();
    let mut errors: Vec<GraphError> = Vec::new();

    for node in nodes {
        // Safely extract the "id" field. If a node entry is not an object
        // (e.g. a JSON string or null), skip it — the unknown-node-type
        // check (check 3, deferred) will catch this properly.
        let node_id = match node.get("id") {
            Some(Value::String(id)) => id.clone(),
            _ => continue, // malformed node entry; check 3 will report it
        };

        // If the ID is already in the set, this is a duplicate — push the
        // error and continue iterating to find further duplicates.
        if !seen_ids.insert(node_id.clone()) {
            errors.push(GraphError::DuplicateNodeId(node_id));
        }
    }

    if errors.is_empty() {
        Ok(ValidatedGraph(graph))
    } else {
        Err(errors)
    }
}
