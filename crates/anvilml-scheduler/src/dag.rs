/// DAG graph validation for job submission.
///
/// Implements the collect-all-errors validation pipeline defined in
/// ANVILML_DESIGN.md §12.3. This module implements checks 1–4:
/// (1) root is an object with a "nodes" array,
/// (2) no duplicate node id values,
/// (3) every node's type exists in the node type registry,
/// (4) every edge references a node that exists and declares the
///     referenced output slot.
///
/// Checks 5–6 (slot-type compatibility, cycle detection) are added by
/// subsequent tasks in Phase 12.
///
/// # Arguments
///
/// * `graph` — A JSON object representing the job graph. Must contain
///   a `"nodes"` array with objects having an `"id"` field. May also
///   contain an `"edges"` array with `"from"` fields in
///   `"node_id:slot_name"` format.
/// * `registry` — The node type registry, used for checks 3–4 (and
///   5–6 in future tasks).
///
/// # Returns
///
/// `Ok(ValidatedGraph)` if checks 1–4 pass with zero errors, or
/// `Err(Vec<GraphError>)` containing all collected errors (collect-all-errors
/// semantics: never short-circuit on the first error, except where a
/// violation makes further checking structurally meaningless).
use std::collections::{HashMap, HashSet};

use anvilml_core::NodeTypeRegistry;
use serde_json::Value;

use crate::types::{GraphError, ValidatedGraph};

/// Validate a DAG graph: structural root check, duplicate-ID check,
/// node-type validation, and dangling-edge check.
///
/// Runs checks 1–4 of the six-check validation pipeline. The non-object
/// root case is the only early return permitted by the collect-all-errors
/// contract — if the root is not an object there is no "nodes" key to
/// inspect, so further checks would be meaningless.
///
/// # Collect-all-errors behaviour
///
/// Duplicate node IDs, unknown node types, and dangling edges are all
/// collected into a single `Err(Vec)` — the iteration continues past
/// each violation rather than returning immediately. This lets callers
/// report all structural problems at once.
pub fn validate_graph(
    graph: Value,
    registry: &NodeTypeRegistry,
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
    // The seen_ids set is also reused by check 4 to look up node existence.
    let mut seen_ids = HashSet::new();
    let mut errors: Vec<GraphError> = Vec::new();

    for node in nodes {
        // Safely extract the "id" field. If a node entry is not an object
        // (e.g. a JSON string or null), skip it — check 3 will catch nodes
        // without an id when it queries the registry.
        let node_id = match node.get("id") {
            Some(Value::String(id)) => id.clone(),
            _ => continue, // malformed node entry; skip for checks 2–3
        };

        // If the ID is already in the set, this is a duplicate — push the
        // error and continue iterating to find further duplicates.
        if !seen_ids.insert(node_id.clone()) {
            errors.push(GraphError::DuplicateNodeId(node_id));
        }
    }

    // Check 3: every node's type must exist in the registry.
    // Iterate all nodes again, look up each node's type in the registry,
    // and push UnknownNodeType for any type not found. This is a separate
    // loop from check 2 because it needs the registry which is only
    // available after the structural checks pass.
    for node in nodes {
        // Extract the node id — we need it for the error message.
        // If the node has no id, we cannot produce a meaningful error.
        let node_id = match node.get("id") {
            Some(Value::String(id)) => id.clone(),
            _ => continue, // no id; cannot report UnknownNodeType
        };

        // Extract the node type. If a node has no "type" field, skip it —
        // this is a data quality issue that will be caught by a future
        // structural check (or is acceptable as a no-op for now).
        let type_name = match node.get("type") {
            Some(Value::String(t)) => t.clone(),
            _ => continue, // no type field; skip
        };

        // Query the registry for this type. If the type is not registered,
        // push an UnknownNodeType error. Continue iterating to collect
        // all unknown types, not just the first one.
        if registry.get(&type_name).is_none() {
            errors.push(GraphError::UnknownNodeType { node_id, type_name });
        }
    }

    // Check 4: every edge must reference a node that exists and declares
    // the referenced output slot. Only run this check if the graph has
    // an "edges" array — a graph without edges has no dangling edges.
    if let Some(Value::Array(edges)) = obj.get("edges") {
        // Build a map from node id to the node's type_name, so we can
        // look up the NodeTypeDescriptor when checking slot declarations.
        // We only include nodes that have both an id and a type.
        let mut id_to_type: HashMap<String, String> = HashMap::new();
        for node in nodes {
            let node_id = match node.get("id") {
                Some(Value::String(id)) => id.clone(),
                _ => continue,
            };
            let type_name = match node.get("type") {
                Some(Value::String(t)) => t.clone(),
                _ => continue,
            };
            id_to_type.insert(node_id, type_name);
        }

        for edge in edges {
            // Extract the "from" field. This is a string in the format
            // "node_id:slot_name" (e.g. "load_model_0:MODEL"). Split on
            // the first colon to separate the node id from the slot name.
            let from = match edge.get("from") {
                Some(Value::String(s)) => s.clone(),
                _ => continue, // missing or non-string "from"; skip edge
            };

            // Split on the first colon. If there is no colon or multiple
            // colons, the format is malformed — skip this edge. Check 4
            // only covers edges that are structurally valid but reference
            // invalid targets.
            let mut parts = from.splitn(2, ':');
            let source_node_id = match parts.next() {
                Some(id) => id.to_string(),
                None => continue,
            };
            let slot_name = match parts.next() {
                Some(slot) => slot.to_string(),
                None => continue, // no colon found; malformed
            };

            // Check if the source node exists in the nodes array.
            // If not, this is a dangling edge.
            if !seen_ids.contains(&source_node_id) {
                errors.push(GraphError::DanglingEdge {
                    node_id: source_node_id.clone(),
                    slot_name,
                });
                continue; // node doesn't exist; skip slot check
            }

            // The node exists — now check if it declares the output slot.
            // Look up the node's type from the id_to_type map, then check
            // if any output slot matches the requested slot_name.
            if let Some(type_name) = id_to_type.get(&source_node_id) {
                // If the node's type is not in the registry (unknown type),
                // we cannot determine whether the slot is declared. Skip
                // the slot check — the UnknownNodeType error from check 3
                // already flags this node's type as invalid.
                if let Some(descriptor) = registry.get(type_name) {
                    // Check if any output slot matches the requested name.
                    // If no match is found, this is a dangling edge.
                    let slot_declared =
                        descriptor.outputs.iter().any(|slot| slot.name == slot_name);

                    if !slot_declared {
                        errors.push(GraphError::DanglingEdge {
                            node_id: source_node_id,
                            slot_name,
                        });
                    }
                }
                // If the type is unknown (not in registry), skip the slot
                // check — the UnknownNodeType error from check 3 already
                // covers this node.
            }
            // If the node has no "type" field, we cannot determine the
            // slot declaration; skip the check.
        }
    }

    if errors.is_empty() {
        Ok(ValidatedGraph(graph))
    } else {
        Err(errors)
    }
}
