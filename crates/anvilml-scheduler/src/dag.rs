use std::collections::HashMap;

use serde_json::Value;

use super::nodes::{get_node_slots, KNOWN_NODE_TYPES};

/// A zero-cost newtype proving that a graph value passed validation.
#[derive(Debug, Clone)]
pub struct ValidatedGraph(pub Value);

/// Validate a graph JSON value, collecting all errors (non-fail-fast).
///
/// Checks performed:
/// - The root value must be a JSON object.
/// - The `"nodes"` field must be present and be an array.
/// - Each node's `"id"` must be unique within the array.
/// - Each node's `"type"` must be one of the known node types.
pub fn validate_graph(v: &Value) -> Result<ValidatedGraph, Vec<String>> {
    // a. Assert that v is an object.
    let obj = match v {
        Value::Object(obj) => obj,
        _ => return Err(vec!["invalid_graph: expected object".to_string()]),
    };

    // b. Extract the "nodes" field; if absent or not an array, return error.
    let nodes = match obj.get("nodes") {
        Some(Value::Array(nodes)) => nodes,
        _ => {
            return Err(vec![
                "invalid_graph: missing or invalid 'nodes' field".to_string()
            ])
        }
    };

    // c. Iterate nodes collecting errors.
    let mut errors: Vec<String> = Vec::new();
    let mut seen_ids: Vec<&str> = Vec::new();

    for node in nodes {
        let node_obj = match node {
            Value::Object(obj) => obj,
            _ => {
                errors.push("invalid_graph: expected object".to_string());
                continue;
            }
        };

        // Track seen IDs — duplicate detection.
        if let Some(Value::String(id)) = node_obj.get("id") {
            if seen_ids.contains(&id.as_str()) {
                errors.push(format!("duplicate_node_id: {id}"));
            } else {
                seen_ids.push(id.as_str());
            }
        }

        // Check each node's "type" field against KNOWN_NODE_TYPES.
        let type_str = node_obj.get("type").and_then(|v| v.as_str());
        match type_str {
            Some(t) if KNOWN_NODE_TYPES.contains(&t) => {}
            Some(t) => errors.push(format!("unknown_node_type: {t}")),
            None => errors.push("unknown_node_type: (missing)".to_string()),
        }
    }

    // d. Edge-reference validation: check that each node's inputs reference
    //    valid nodes and valid output slots on those nodes.
    //    Also build an adjacency list for cycle detection.
    let mut id_type_map: HashMap<String, String> = HashMap::new();
    for node in nodes {
        if let Value::Object(obj) = node {
            if let (Some(Value::String(id)), Some(Value::String(t))) =
                (obj.get("id"), obj.get("type"))
            {
                id_type_map.insert(id.to_string(), t.to_string());
            }
        }
    }

    // Build adjacency list for cycle detection: edge from node → dependency
    // (node depends on the referenced node's output).
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for node in nodes {
        if let Value::Object(obj) = node {
            if let Some(Value::String(node_id)) = obj.get("id") {
                adj.entry(node_id.to_string()).or_default();
            }
            if let Some(inputs_val) = obj.get("inputs") {
                if let Some(inputs_obj) = inputs_val.as_object() {
                    for (_slot_name, input_value) in inputs_obj {
                        // If input is an object with node_id + output_slot keys, it's an edge ref.
                        if let (Some(Value::String(ref_node_id)), Some(Value::String(ref_slot))) =
                            (input_value.get("node_id"), input_value.get("output_slot"))
                        {
                            // Check 1: does the referenced node exist?
                            if !id_type_map.contains_key(ref_node_id.as_str()) {
                                errors.push(format!("unknown_node_ref: {}", ref_node_id));
                                continue; // skip slot check — node doesn't exist
                            }

                            // Check 2: does that node's type declare this output slot?
                            let ref_type = &id_type_map[ref_node_id.as_str()];
                            if let Some(slots) = get_node_slots(ref_type) {
                                if !slots.outputs.contains(&ref_slot.as_str()) {
                                    errors.push(format!(
                                        "unknown_output_slot: {}.{}",
                                        ref_node_id, ref_slot
                                    ));
                                }
                            }

                            // Build adjacency edge: current node depends on ref_node_id.
                            let cur_id = node
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            adj.entry(cur_id).or_default().push(ref_node_id.to_string());
                        }
                    }
                }
            }
        }
    }

    // e. If any errors were collected, return Err(errors).
    if !errors.is_empty() {
        return Err(errors);
    }

    // f. Cycle detection via Kahn's algorithm.
    let total_nodes = adj.len();
    if total_nodes > 0 {
        // Compute in-degree: for each node, count how many other nodes point to it.
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for id in adj.keys() {
            in_degree.insert(id.clone(), 0);
        }
        for edges in adj.values() {
            for dep in edges {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        // Seed queue with nodes having in-degree 0.
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_k, v)| **v == 0)
            .map(|(k, _)| k.clone())
            .collect();
        queue.sort();

        let mut processed_count: usize = 0;
        while let Some(node) = queue.pop() {
            processed_count += 1;
            // Decrement in-degree of all nodes that depend on this node.
            if let Some(deps) = adj.get(&node) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }

        // If not all nodes were processed, a cycle exists.
        if processed_count < total_nodes {
            let mut sorted_ids: Vec<String> = in_degree
                .iter()
                .filter(|(_k, v)| **v > 0)
                .map(|(k, _)| k.clone())
                .collect();
            sorted_ids.sort();
            let ids_str = sorted_ids.join(",");
            errors.push(format!("cycle_detected: {ids_str}"));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    // g. Otherwise return Ok(ValidatedGraph(v.clone())).
    Ok(ValidatedGraph(v.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duplicate_node_id() {
        let graph = serde_json::json!({
            "nodes": [
                { "id": "n0", "type": "ZitLoadPipeline" },
                { "id": "n0", "type": "ZitTextEncode" }
            ]
        });
        let result = validate_graph(&graph);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e == "duplicate_node_id: n0"),
            "expected 'duplicate_node_id: n0' in errors, got: {errors:?}"
        );
    }

    #[test]
    fn test_unknown_node_type() {
        let graph = serde_json::json!({
            "nodes": [
                { "id": "n0", "type": "NopeNode" }
            ]
        });
        let result = validate_graph(&graph);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e == "unknown_node_type: NopeNode"),
            "expected 'unknown_node_type: NopeNode' in errors, got: {errors:?}"
        );
    }

    #[test]
    fn test_valid_graph() {
        let graph = serde_json::json!({
            "nodes": [
                { "id": "load", "type": "ZitLoadPipeline" },
                { "id": "encode", "type": "ZitTextEncode" }
            ]
        });
        let result = validate_graph(&graph);
        assert!(
            result.is_ok(),
            "expected Ok, got Err: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn test_unknown_node_ref() {
        // Graph with two nodes where one references a non-existent node ID.
        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "load",
                    "type": "ZitLoadPipeline"
                },
                {
                    "id": "encode",
                    "type": "ZitTextEncode",
                    "inputs": {
                        "pipeline": { "node_id": "ghost_node", "output_slot": "pipeline" }
                    }
                }
            ]
        });
        let result = validate_graph(&graph);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e == "unknown_node_ref: ghost_node"),
            "expected 'unknown_node_ref: ghost_node' in errors, got: {errors:?}"
        );
    }

    #[test]
    fn test_unknown_output_slot() {
        // Graph where output slot doesn't exist on the referenced type.
        // ZitLoadPipeline only outputs "pipeline", not "latents".
        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "load",
                    "type": "ZitLoadPipeline"
                },
                {
                    "id": "sample",
                    "type": "ZitSampler",
                    "inputs": {
                        "pipeline": { "node_id": "load", "output_slot": "latents" }
                    }
                }
            ]
        });
        let result = validate_graph(&graph);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e == "unknown_output_slot: load.latents"),
            "expected 'unknown_output_slot: load.latents' in errors, got: {errors:?}"
        );
    }

    #[test]
    fn test_valid_edge_references() {
        // Valid ZiT 2-node graph (ZitLoadPipeline → ZitTextEncode).
        // ZitLoadPipeline outputs "pipeline", and ZitTextEncode's "pipeline" input
        // correctly references that output.
        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "load",
                    "type": "ZitLoadPipeline"
                },
                {
                    "id": "encode",
                    "type": "ZitTextEncode",
                    "inputs": {
                        "pipeline": { "node_id": "load", "output_slot": "pipeline" }
                    }
                }
            ]
        });
        let result = validate_graph(&graph);
        assert!(
            result.is_ok(),
            "expected Ok, got Err: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn test_cycle_detected_2node() {
        // Two nodes referencing each other → cycle.
        // A's input references B's output, B's input references A's output.
        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "a",
                    "type": "ZitLoadPipeline",
                    "inputs": {
                        "model_id": { "node_id": "b", "output_slot": "pipeline" }
                    }
                },
                {
                    "id": "b",
                    "type": "ZitLoadPipeline",
                    "inputs": {
                        "model_id": { "node_id": "a", "output_slot": "pipeline" }
                    }
                }
            ]
        });
        let result = validate_graph(&graph);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("cycle_detected")),
            "expected 'cycle_detected' in errors, got: {errors:?}"
        );
    }

    #[test]
    fn test_valid_zit_5node_passes() {
        // Full ZiT pipeline: ZitLoadPipeline → ZitTextEncode → ZitSampler →
        // ZitDecode → SaveImage. All edges valid, no cycles.
        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "load",
                    "type": "ZitLoadPipeline"
                },
                {
                    "id": "encode",
                    "type": "ZitTextEncode",
                    "inputs": {
                        "pipeline": { "node_id": "load", "output_slot": "pipeline" }
                    }
                },
                {
                    "id": "sampler",
                    "type": "ZitSampler",
                    "inputs": {
                        "pipeline": { "node_id": "load", "output_slot": "pipeline" },
                        "conditioning": { "node_id": "encode", "output_slot": "conditioning" }
                    }
                },
                {
                    "id": "decode",
                    "type": "ZitDecode",
                    "inputs": {
                        "pipeline": { "node_id": "load", "output_slot": "pipeline" },
                        "latents": { "node_id": "sampler", "output_slot": "latents" }
                    }
                },
                {
                    "id": "save",
                    "type": "SaveImage",
                    "inputs": {
                        "image": { "node_id": "decode", "output_slot": "image" }
                    }
                }
            ]
        });
        let result = validate_graph(&graph);
        assert!(
            result.is_ok(),
            "expected Ok, got Err: {:?}",
            result.unwrap_err()
        );
    }
}
