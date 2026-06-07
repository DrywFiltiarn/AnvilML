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
    let mut id_type_map: HashMap<&str, &str> = HashMap::new();
    for node in nodes {
        if let Value::Object(obj) = node {
            if let (Some(Value::String(id)), Some(Value::String(t))) =
                (obj.get("id"), obj.get("type"))
            {
                id_type_map.insert(id.as_str(), t.as_str());
            }
        }
    }

    for node in nodes {
        if let Value::Object(obj) = node {
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
                            let ref_type = id_type_map[ref_node_id.as_str()];
                            if let Some(slots) = get_node_slots(ref_type) {
                                if !slots.outputs.contains(&ref_slot.as_str()) {
                                    errors.push(format!(
                                        "unknown_output_slot: {}.{}",
                                        ref_node_id, ref_slot
                                    ));
                                }
                            }
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

    // f. Otherwise return Ok(ValidatedGraph(v.clone())).
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
}
