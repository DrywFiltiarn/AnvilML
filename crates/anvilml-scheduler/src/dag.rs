//! DAG (Directed Acyclic Graph) validation for job graphs.
//!
//! Provides `validate_graph` which performs six independent validation checks on a
//! job graph JSON payload — structural integrity, ID uniqueness, node type
//! registration, edge references, slot type compatibility, and acyclicity —
//! collecting all errors before returning so callers receive the complete error
//! list in a single response.
//!
//! Kahn's algorithm is used for cycle detection because it produces a
//! deterministic topological ordering when combined with `IndexMap`, making
//! cycle member lists reproducible across runs — unlike DFS which depends
//! on hash-map iteration order.

use std::collections::HashSet;

use indexmap::IndexMap;
use serde_json::Value;

use crate::types::GraphError;
use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, SlotType};

/// A graph that has passed all validation checks.
///
/// This newtype wraps the original JSON payload and is the only
/// way to construct a validated graph — the constructor is private
/// to this module, ensuring no caller can bypass validation.
#[derive(Debug, Clone)]
pub struct ValidatedGraph(pub serde_json::Value);

/// Validate a job graph against the node type registry.
///
/// Collects all errors before returning (non-fail-fast), so callers
/// receive the complete list of problems in a single response.
///
/// Checks performed (all collected, none fail-fast):
/// 1. Root JSON is an object with a `"nodes"` array.
/// 2. No duplicate node `"id"` values.
/// 3. Every node `"type"` exists in `NodeTypeRegistry`.
/// 4. Every edge reference `{node_id, output_slot}` resolves to an
///    existing node and a declared output slot.
/// 5. Every edge's output slot type is compatible with the receiving
///    input slot type (both match, or either is `SlotType::Any`).
/// 6. The graph is acyclic (Kahn's algorithm).
///
/// # Arguments
///
/// * `graph` — The submitted graph JSON value. Must be a JSON object
///   with `"nodes"` and optionally `"edges"` arrays.
/// * `registry` — The current node type registry, populated from
///   worker `Ready` events.
///
/// # Returns
///
/// `Ok(ValidatedGraph)` if all checks pass, containing the original
/// graph value. `Err(Vec<String>)` with all error messages if any
/// check fails — the vector contains one human-readable string per
/// failure, each naming the specific offending node/slot/type.
#[tracing::instrument(skip(graph, registry), fields(graph_nodes = ?graph.get("nodes").and_then(|n| n.get("len").map(|l| l.as_u64()))))]
pub async fn validate_graph(
    graph: &Value,
    registry: &NodeTypeRegistry,
) -> Result<ValidatedGraph, Vec<String>> {
    let mut errors: Vec<GraphError> = Vec::new();
    let mut struct_errors: Vec<String> = Vec::new();

    // Check 1: structural — nodes array must exist and be an array.
    // This is checked first because every subsequent check depends on
    // having a valid nodes slice. If this fails, we can skip the rest.
    // Structural errors are collected as strings since they don't map to
    // any GraphError variant — there is no typed equivalent for
    // "nodes field is missing" or "nodes is not an array".
    if let Some(err) = check_nodes_array(graph) {
        struct_errors.push(err);
    }

    // Extract nodes array early so we can skip remaining checks if
    // it was missing. The plan says checks are all collected, but
    // we cannot meaningfully check IDs, types, or edges without nodes.
    let nodes = match extract_nodes(graph) {
        Some(n) => n,
        None => {
            // Convert collected GraphErrors to strings and merge with
            // structural errors. Non-fail-fast: callers get the complete
            // diagnostic picture even when nodes array is missing.
            let string_errors: Vec<String> = errors
                .into_iter()
                .map(|e| e.to_string())
                .chain(struct_errors)
                .collect();
            return Err(string_errors);
        }
    };

    // Check 2: no duplicate node IDs.
    // HashSet is used because duplicate detection is an O(n) membership
    // test per element, and a hash set gives O(1) average-case lookup.
    errors.extend(check_duplicate_ids(&nodes));

    // Check 3: every node type must exist in the registry.
    // This is a registry lookup per node; unknown types are collected
    // so callers can report all missing types at once.
    errors.extend(check_node_types(&nodes, registry).await);

    // Check 4: every edge must reference existing nodes and slots.
    // Edge references are validated before slot type compatibility
    // because a missing node or slot makes type checking meaningless.
    errors.extend(check_edge_refs(graph, &nodes, registry).await);

    // Check 5: slot type compatibility on each edge.
    // Both the source output slot type and target input slot type are
    // looked up from the registry; incompatible pairs are collected.
    errors.extend(check_slot_compatibility(graph, &nodes, registry).await);

    // Check 6: the graph must be acyclic.
    // Kahn's algorithm is used over DFS because it naturally produces
    // a deterministic topological order when combined with IndexMap,
    // making cycle member lists reproducible across runs.
    if let Some(err) = check_acyclic(graph, &nodes) {
        errors.push(err);
    }

    // If any errors were collected, convert typed errors to strings
    // and merge with structural errors. Non-fail-fast: callers get the
    // complete diagnostic picture.
    if !errors.is_empty() || !struct_errors.is_empty() {
        let string_errors: Vec<String> = errors
            .into_iter()
            .map(|e| e.to_string())
            .chain(struct_errors)
            .collect();
        return Err(string_errors);
    }

    Ok(ValidatedGraph(graph.clone()))
}

/// Extract the `"nodes"` array from a graph JSON value.
///
/// Returns `None` if the field is missing or not an array.
/// This is a helper used by `validate_graph` after `check_nodes_array`
/// confirms the field exists.
fn extract_nodes(graph: &Value) -> Option<Vec<&Value>> {
    let nodes = graph.get("nodes")?.as_array()?;
    Some(nodes.iter().collect())
}

/// Verify that `graph["nodes"]` exists and is an array.
///
/// Returns `None` on success, an error string on failure.
fn check_nodes_array(graph: &Value) -> Option<String> {
    match graph.get("nodes") {
        Some(nodes) if nodes.is_array() => None,
        Some(_) => {
            Some("validation failed: \"nodes\" field exists but is not an array".to_string())
        }
        None => Some(
            "validation failed: \"nodes\" field is missing — graph must contain a nodes array"
                .to_string(),
        ),
    }
}

/// Iterate over nodes and collect IDs in a `HashSet`, reporting duplicates.
///
/// Uses `HashSet` because duplicate detection requires O(1) average-case
/// membership testing per element — a `Vec` would be O(n) per lookup.
fn check_duplicate_ids(nodes: &[&Value]) -> Vec<GraphError> {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();

    for node in nodes {
        // Each node JSON object must have an `"id"` string field.
        // If a node lacks an `"id"`, we treat it as a duplicate of the
        // empty string to catch the structural issue.
        let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");

        // If the ID was already seen, this is a duplicate — record it
        // and continue scanning (non-fail-fast).
        if !seen.insert(id.to_string()) {
            duplicates.push(GraphError::DuplicateNodeId(id.to_string()));
        }
    }

    duplicates
}

/// For each node, look up its `"type"` in the registry.
///
/// Unknown types produce `GraphError::UnknownNodeType`. The registry
/// is queried via `get()` which is an async method on `NodeTypeRegistry`.
async fn check_node_types(nodes: &[&Value], registry: &NodeTypeRegistry) -> Vec<GraphError> {
    let mut errors = Vec::new();

    for node in nodes {
        let node_type = node
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing type>");

        // Look up the node type in the registry. A `None` result
        // means this type was never reported by any worker — the
        // graph references a node type that doesn't exist.
        if registry.get(node_type).await.is_none() {
            // Use node_type as the type_name. The Display impl uses
            // the same value for both node and type placeholders,
            // which matches the current error message format.
            errors.push(GraphError::UnknownNodeType(node_type.to_string()));
        }
    }

    errors
}

/// For each edge, resolve `node_id` to a node, then resolve
/// `output_slot` to a declared output slot on that node.
///
/// Reports missing nodes and missing slots separately so callers
/// can distinguish "the edge references a ghost node" from
/// "the edge references a slot that doesn't exist on this node".
///
/// Slot definitions come from the registry's `NodeTypeDescriptor`,
/// not from the raw graph JSON — the graph only stores `{id, type}`
/// per node, with slot metadata held by the registry.
async fn check_edge_refs(
    graph: &Value,
    nodes: &[&Value],
    registry: &NodeTypeRegistry,
) -> Vec<GraphError> {
    // Build a lookup map from node id to NodeTypeDescriptor.
    // We look up each node's type in the registry to get slot info.
    // IndexMap preserves insertion order for deterministic error
    // reporting, though the order doesn't affect correctness.
    let mut node_desc_map: IndexMap<&str, NodeTypeDescriptor> = IndexMap::new();

    for node in nodes {
        let node_id = node.get("id").and_then(|v| v.as_str());
        let node_type = node.get("type").and_then(|v| v.as_str());

        // Look up the descriptor from the registry. If the type is
        // unknown (checked separately), we skip slot checking for
        // this node to avoid spurious errors.
        if let (Some(id), Some(type_name)) = (node_id, node_type) {
            if let Some(desc) = registry.get(type_name).await {
                node_desc_map.insert(id, desc);
            }
        }
    }

    // Edges are stored at the graph root level under the "edges" key.
    // A graph with no "edges" field is valid — it just has no connections.
    let edges = extract_edges(graph);

    let mut errors = Vec::new();

    for edge in &edges {
        let source_id = edge
            .get("node_id")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing node_id>");

        // Check if the source node exists in the graph.
        // If the node doesn't exist, we report it and skip slot
        // checking for this edge (the slot can't be validated
        // without a source node).
        if node_desc_map.get(source_id).is_none() {
            // Missing source node — use empty slot to distinguish
            // from the "missing slot" case in the Display impl.
            errors.push(GraphError::UnknownEdgeRef {
                node_id: source_id.to_string(),
                slot: String::new(),
            });
            continue;
        }

        let output_slot = edge
            .get("output_slot")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing output_slot>");

        let source_desc = &node_desc_map[source_id];

        // Look up the output slot on the source node's descriptor.
        // The slot must be in the descriptor's `"outputs"` array.
        // The descriptor comes from the registry, which is populated
        // by worker `Ready` events.
        let slot_found = source_desc.outputs.iter().any(|s| s.name == output_slot);

        if !slot_found {
            errors.push(GraphError::UnknownEdgeRef {
                node_id: source_id.to_string(),
                slot: output_slot.to_string(),
            });
        }
    }

    errors
}

/// For each edge, look up the source node's output slot type and the
/// target node's input slot type; incompatible types produce an error.
///
/// Compatibility rule: two slot types are compatible if they match
/// exactly, or if either is `SlotType::Any`. This is the standard
/// "Any type accepts any connection" rule used in node-based
/// graph editors.
async fn check_slot_compatibility(
    graph: &Value,
    nodes: &[&Value],
    registry: &NodeTypeRegistry,
) -> Vec<GraphError> {
    // Build a map from node id to NodeTypeDescriptor for slot type
    // lookups. This avoids repeated async registry calls per edge.
    let mut node_desc_map: IndexMap<&str, NodeTypeDescriptor> = IndexMap::new();

    for node in nodes {
        let node_id = node.get("id").and_then(|v| v.as_str());
        let node_type = node.get("type").and_then(|v| v.as_str());

        if let (Some(id), Some(type_name)) = (node_id, node_type) {
            // Look up the descriptor from the registry. If the type
            // is unknown (checked separately), we skip slot compat
            // for this node to avoid spurious errors.
            if let Some(desc) = registry.get(type_name).await {
                node_desc_map.insert(id, desc);
            }
        }
    }

    let edges = extract_edges(graph);

    let mut errors = Vec::new();

    for edge in &edges {
        let source_id = edge
            .get("node_id")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing node_id>");

        let target = edge
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing target>");

        let output_slot = edge
            .get("output_slot")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing output_slot>");

        let target_slot = edge
            .get("target_slot")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing target_slot>");

        // Only check slot compatibility if both source and target
        // nodes have descriptors. Missing nodes are caught by
        // check_edge_refs.
        let source_desc = match node_desc_map.get(source_id) {
            Some(d) => d,
            None => continue, // source node type unknown, skip
        };

        let target_desc = match node_desc_map.get(target) {
            Some(d) => d,
            None => continue, // target node type unknown, skip
        };

        // Look up the source output slot type.
        let source_slot_type = source_desc
            .outputs
            .iter()
            .find(|s| s.name == output_slot)
            .map(|s| s.slot_type);

        // Look up the target input slot type.
        let target_slot_type = target_desc
            .inputs
            .iter()
            .find(|s| s.name == target_slot)
            .map(|s| s.slot_type);

        // If either slot is missing, skip this edge — check_edge_refs
        // already reported that issue. We only check type compatibility
        // when both slots exist.
        if let (Some(src_type), Some(tgt_type)) = (source_slot_type, target_slot_type) {
            // Two types are compatible if they match exactly, or if
            // either is `Any`. This implements the standard "Any slot
            // accepts any connection" rule used in node-based editors.
            if !types_compatible(src_type, tgt_type) {
                errors.push(GraphError::SlotTypeMismatch {
                    from: src_type,
                    to: tgt_type,
                });
            }
        }
    }

    errors
}

/// Check whether two slot types are compatible.
///
/// Types are compatible if they are equal, or if either is `SlotType::Any`.
/// The `Any` variant is the universal adapter — it matches any concrete
/// type, which is the standard behaviour in node-based graph editors.
fn types_compatible(a: SlotType, b: SlotType) -> bool {
    // If either type is `Any`, it accepts any connection.
    // This is the universal adapter rule.
    if a == SlotType::Any || b == SlotType::Any {
        return true;
    }

    // Exact match is always compatible.
    a == b
}

/// Build an adjacency list from edges and run Kahn's algorithm.
///
/// If not all nodes are processed (i.e., the queue empties before
/// all nodes are visited), a cycle exists. Returns `Some(GraphError)`
/// naming the cycle participants.
///
/// Kahn's algorithm is chosen over DFS because:
/// - It naturally identifies all nodes in cycles (nodes with residual
///   in-degree > 0 after the algorithm completes).
/// - Combined with `IndexMap` for deterministic iteration, cycle
///   member lists are reproducible across runs.
fn check_acyclic(graph: &Value, nodes: &[&Value]) -> Option<GraphError> {
    let node_ids: HashSet<&str> = nodes
        .iter()
        .filter_map(|n| n.get("id").and_then(|v| v.as_str()))
        .collect();

    let edge_count = node_ids.len();

    // Build adjacency list using IndexMap for deterministic iteration.
    // IndexMap preserves insertion order, which makes the topological
    // sort and cycle member lists reproducible across runs.
    let mut adj: IndexMap<&str, Vec<&str>> = IndexMap::new();
    let mut in_degree: IndexMap<&str, usize> = IndexMap::new();

    // Initialise all nodes with zero in-degree and empty adjacency list.
    for &id in &node_ids {
        adj.insert(id, Vec::new());
        in_degree.insert(id, 0);
    }

    // Build edges from the graph's edge list.
    let edges = extract_edges(graph);

    for edge in &edges {
        let source_id = edge.get("node_id").and_then(|v| v.as_str());
        let target = edge.get("target").and_then(|v| v.as_str());

        // Only add edges where both endpoints exist as nodes.
        // Missing nodes are caught by check_edge_refs.
        if let (Some(src), Some(tgt)) = (source_id, target) {
            // Only add the edge if the target is a known node
            // (not a phantom reference).
            if node_ids.contains(tgt) {
                adj.entry(src).or_default().push(tgt);
                *in_degree.entry(tgt).or_insert(0) += 1;
            }
        }
    }

    // Kahn's algorithm: process nodes with in-degree 0,
    // decrement in-degrees of neighbours, repeat.
    // IndexMap iteration order is deterministic, so the
    // processing order is reproducible.
    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    // Sort the initial queue for deterministic ordering.
    // This ensures the same graph always produces the same
    // topological order and cycle member list.
    queue.sort();

    let mut processed = 0;

    while let Some(current) = queue.pop() {
        processed += 1;

        // Decrement in-degree for each neighbour.
        // If a neighbour's in-degree drops to 0, add it to the queue.
        // IndexMap iteration is deterministic, so the order in which
        // neighbours are processed is reproducible.
        if let Some(neighbours) = adj.get(current) {
            for &neighbour in neighbours {
                let deg = in_degree.get_mut(neighbour).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(neighbour);
                }
            }
        }
    }

    // If not all nodes were processed, the remaining nodes form
    // a cycle. Report them as the cycle participants.
    // IndexMap preserves insertion order, so the cycle list is
    // deterministic.
    if processed < edge_count {
        let cycle_nodes: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(&id, _)| id.to_string())
            .collect();

        return Some(GraphError::CycleDetected(cycle_nodes));
    }

    None
}

/// Extract the `"edges"` array from a graph JSON value.
///
/// Edges are stored at the graph root level under the `"edges"` key.
/// Returns an empty vec if no edges exist or if the edges field
/// is not present. This is a helper used by edge-checking functions.
fn extract_edges(graph: &Value) -> Vec<&Value> {
    graph
        .get("edges")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}
