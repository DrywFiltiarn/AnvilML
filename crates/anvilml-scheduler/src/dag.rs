/// DAG graph validation for job submission.
///
/// Implements the collect-all-errors validation pipeline defined in
/// ANVILML_DESIGN.md §12.3. This module implements checks 1–6:
/// (1) root is an object with a "nodes" array,
/// (2) no duplicate node id values,
/// (3) every node's type exists in the node type registry,
/// (4) every edge references a node that exists and declares the
///     referenced output slot,
/// (5) every edge's output slot type is compatible with the
///     destination input slot type (exact match or `Any` on either side),
/// (6) the directed graph of node connections contains no cycles
///     (detected via Kahn's algorithm).
///
/// # Arguments
///
/// * `graph` — A JSON object representing the job graph. Must contain
///   a `"nodes"` array with objects having an `"id"` field. May also
///   contain an `"edges"` array with `"from"` and `"to"` fields in
///   `"node_id:slot_name"` format.
/// * `registry` — The node type registry, used for checks 3–5.
///
/// # Returns
///
/// `Ok(ValidatedGraph)` if checks 1–6 pass with zero errors, or
/// `Err(Vec<GraphError>)` containing all collected errors (collect-all-errors
/// semantics: never short-circuit on the first error, except where a
/// violation makes further checking structurally meaningless).
use std::collections::{HashMap, HashSet};

use anvilml_core::{NodeTypeRegistry, SlotType};
use serde_json::Value;

use crate::types::{GraphError, ValidatedGraph};

/// Convert a `SlotType` to its `SCREAMING_SNAKE_CASE` label string.
///
/// `SlotType` derives `Debug` (which uses PascalCase, e.g. `Model`) and
/// uses `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` (which produces
/// `MODEL`). This function matches the serde serialization form, which
/// is the form used in the `SlotTypeMismatch` error message fields
/// `expected` and `found`.
fn slot_type_label(t: SlotType) -> String {
    match t {
        SlotType::Model => "Model",
        SlotType::Clip => "Clip",
        SlotType::Vae => "Vae",
        SlotType::Conditioning => "Conditioning",
        SlotType::Latent => "Latent",
        SlotType::Image => "Image",
        SlotType::String => "String",
        SlotType::Int => "Int",
        SlotType::Float => "Float",
        SlotType::Bool => "Bool",
        SlotType::Any => "Any",
    }
    .to_string()
}

/// Validate a DAG graph: structural root check, duplicate-ID check,
/// node-type validation, dangling-edge check, slot-type compatibility,
/// and cycle detection.
///
/// Runs all six checks of the validation pipeline. The non-object root
/// case is the only early return permitted by the collect-all-errors
/// contract — if the root is not an object there is no "nodes" key to
/// inspect, so further checks would be meaningless.
///
/// # Collect-all-errors behaviour
///
/// Duplicate node IDs, unknown node types, dangling edges, slot-type
/// mismatches, and cycles are all collected into a single `Err(Vec)` —
/// the iteration continues past each violation rather than returning
/// immediately. This lets callers report all structural problems at once.
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

    // Build a map from node id to the node's type_name, so we can
    // look up the NodeTypeDescriptor when checking slot declarations.
    // We only include nodes that have both an id and a type.
    // This map is shared by checks 4 and 5.
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

    // Check 4: every edge must reference a node that exists and declares
    // the referenced output slot. Only run this check if the graph has
    // an "edges" array — a graph without edges has no dangling edges.
    if let Some(Value::Array(edges)) = obj.get("edges") {
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

    // Check 5: for every edge that has a "to" field, compare the
    // source output slot type with the destination input slot type.
    // Only run this check on edges whose source node ID was NOT flagged
    // as DanglingEdge in check 4 — this prevents double-reporting the
    // same edge with both DanglingEdge and SlotTypeMismatch errors.
    //
    // First, build a set of source node IDs that were reported as
    // DanglingEdge during check 4, so we can skip them here.
    let dangling_sources: HashSet<String> = errors
        .iter()
        .filter_map(|e| match e {
            GraphError::DanglingEdge { node_id, .. } => Some(node_id.clone()),
            _ => None,
        })
        .collect();

    // Only proceed if the graph has an "edges" array.
    if let Some(Value::Array(edges)) = obj.get("edges") {
        for edge in edges {
            // Only process edges that have a "to" field — edges without
            // "to" have no destination input to check.
            let to = match edge.get("to") {
                Some(Value::String(s)) => s.clone(),
                _ => continue, // no "to" field; skip this edge
            };

            // Parse the "from" field (source_node_id:source_slot_name).
            let from = match edge.get("from") {
                Some(Value::String(s)) => s.clone(),
                _ => continue, // missing or non-string "from"; skip edge
            };

            let mut parts = from.splitn(2, ':');
            let source_node_id = match parts.next() {
                Some(id) => id.to_string(),
                None => continue,
            };
            let source_slot_name = match parts.next() {
                Some(slot) => slot.to_string(),
                None => continue,
            };

            // Skip edges whose source node was flagged as DanglingEdge
            // in check 4 — it was already reported and we don't want
            // to double-report it as a SlotTypeMismatch too.
            if dangling_sources.contains(&source_node_id) {
                continue;
            }

            // Parse the "to" field (dest_node_id:dest_slot_name).
            let mut to_parts = to.splitn(2, ':');
            let dest_node_id = match to_parts.next() {
                Some(id) => id.to_string(),
                None => continue,
            };
            let dest_slot_name = match to_parts.next() {
                Some(slot) => slot.to_string(),
                None => continue,
            };

            // Look up the source node's descriptor from id_to_type.
            // If the source node doesn't have a registered type, skip —
            // the UnknownNodeType error from check 3 already flags it.
            let source_descriptor = if let Some(type_name) = id_to_type.get(&source_node_id) {
                registry.get(type_name)
            } else {
                None
            };

            // Look up the destination node's descriptor.
            let dest_descriptor = if let Some(type_name) = id_to_type.get(&dest_node_id) {
                registry.get(type_name)
            } else {
                None
            };

            // If either node's type is unknown, skip — already covered
            // by check 3 errors.
            let (source_desc, dest_desc) = match (source_descriptor, dest_descriptor) {
                (Some(s), Some(d)) => (s, d),
                _ => continue,
            };

            // Find the matching output slot on the source node.
            // If not found, skip — the DanglingEdge error from check 4
            // already flags this slot.
            let output_slot = match source_desc
                .outputs
                .iter()
                .find(|slot| slot.name == source_slot_name)
            {
                Some(slot) => slot,
                None => continue,
            };

            // Find the matching input slot on the destination node.
            // If not found, skip — the DanglingEdge error from check 4
            // already flags this slot.
            let input_slot = match dest_desc
                .inputs
                .iter()
                .find(|slot| slot.name == dest_slot_name)
            {
                Some(slot) => slot,
                None => continue,
            };

            // Compare slot types: a mismatch is only an error if the
            // types differ AND neither side is SlotType::Any. Any acts
            // as a wildcard that accepts any connection.
            if output_slot.slot_type != input_slot.slot_type
                && output_slot.slot_type != SlotType::Any
                && input_slot.slot_type != SlotType::Any
            {
                errors.push(GraphError::SlotTypeMismatch {
                    node_id: dest_node_id,
                    slot_name: dest_slot_name,
                    expected: slot_type_label(input_slot.slot_type),
                    found: slot_type_label(output_slot.slot_type),
                });
            }
        }
    }

    // Check 6: cycle detection via Kahn's algorithm.
    // Build a directed adjacency list from the edge list, compute in-degrees,
    // and iteratively remove zero-in-degree nodes. Any node not processed
    // is part of a cycle. This runs in O(V + E) time.
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(Value::Array(edges)) = obj.get("edges") {
        for edge in edges {
            // Parse the "from" field (same format as checks 4–5).
            let from = match edge.get("from") {
                Some(Value::String(s)) => s.clone(),
                _ => continue,
            };

            let mut parts = from.splitn(2, ':');
            let source_node_id = match parts.next() {
                Some(id) => id.to_string(),
                None => continue,
            };

            // Only add the edge if the source node actually exists in the
            // graph — edges from nonexistent nodes were already flagged as
            // DanglingEdge in check 4 and should not contribute to cycles.
            if !seen_ids.contains(&source_node_id) {
                continue;
            }

            // Parse the "to" field (same format as check 5).
            let to = match edge.get("to") {
                Some(Value::String(s)) => s.clone(),
                _ => continue,
            };

            let mut to_parts = to.splitn(2, ':');
            let dest_node_id = match to_parts.next() {
                Some(id) => id.to_string(),
                None => continue,
            };

            // Only add the edge if the destination node also exists.
            if !seen_ids.contains(&dest_node_id) {
                continue;
            }

            // Add the directed edge source → dest to the adjacency list.
            // An edge from a node to itself (self-loop) is a valid cycle
            // candidate and is included here.
            adjacency
                .entry(source_node_id)
                .or_default()
                .push(dest_node_id);
        }
    }

    // Compute in-degree for every node in the graph.
    // Initialize all nodes to 0, then increment for each edge destination.
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for node in nodes {
        // Extract the node id from the JSON object. If the node is not
        // an object (e.g. a JSON string), skip it — it has no "id" field.
        if let Some(obj_node) = node.as_object()
            && let Some(Value::String(id)) = obj_node.get("id")
        {
            in_degree.insert(id.clone(), 0);
        }
    }
    // Increment in-degree for each destination node in the adjacency list.
    // Use entry().or_insert(0) to safely handle the (shouldn't happen)
    // case where a destination node is missing from the nodes array.
    for neighbors in adjacency.values() {
        for dest in neighbors {
            *in_degree.entry(dest.clone()).or_insert(0) += 1;
        }
    }

    // Initialize the queue with all nodes that have in-degree 0.
    // These are the nodes that have no incoming edges and can be
    // topologically sorted first.
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect();

    // Process the queue using Kahn's algorithm: pop a node, add it to
    // the processed set, decrement in-degrees of its neighbors, and
    // push any neighbor whose in-degree reaches 0.
    let mut processed: HashSet<String> = HashSet::new();
    while let Some(node) = queue.pop() {
        processed.insert(node.clone());
        // Decrement in-degree for each neighbor and enqueue if it reaches 0.
        if let Some(neighbors) = adjacency.get(&node) {
            for neighbor in neighbors {
                if let Some(degree) = in_degree.get_mut(neighbor) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(neighbor.clone());
                    }
                }
            }
        }
    }

    // Any node not in the processed set is part of a cycle.
    // Collect these remaining node IDs into a Vec for the error.
    let remaining: Vec<String> = in_degree
        .keys()
        .filter(|id| !processed.contains(*id))
        .cloned()
        .collect();

    if !remaining.is_empty() {
        errors.push(GraphError::CycleDetected(remaining));
    }

    if errors.is_empty() {
        Ok(ValidatedGraph(graph))
    } else {
        Err(errors)
    }
}
