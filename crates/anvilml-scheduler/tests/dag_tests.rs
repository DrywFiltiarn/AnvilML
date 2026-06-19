//! Tests for `dag.rs` — `validate_graph` DAG validation.
//!
//! Each test populates a `NodeTypeRegistry` with the node types needed
//! for that scenario, then calls `validate_graph` and asserts on the result.

use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor, SlotType};
use anvilml_scheduler::dag::{validate_graph, ValidatedGraph};

/// Submit a graph without a `"nodes"` field.
///
/// Verifies that `validate_graph` returns an error about the missing
/// nodes array and does not panic on malformed input.
#[tokio::test]
async fn test_missing_nodes_array() {
    let registry = NodeTypeRegistry::new().await;

    let graph = serde_json::json!({ "edges": [] });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("nodes"));
    assert!(errors[0].contains("missing"));
}

/// Submit a graph with two nodes sharing the same `"id"`.
///
/// Verifies that `validate_graph` detects the duplicate ID and reports
/// it in the error list.
#[tokio::test]
async fn test_duplicate_node_ids() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with a node type so type checking doesn't
    // also fail (we want to isolate the duplicate ID error).
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a diffusion model".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "n1", "type": "LoadModel" },
            { "id": "n1", "type": "LoadModel" }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Duplicate IDs and unknown types are collected together
    // (both nodes are "LoadModel" which is registered, so only dup).
    assert!(errors.iter().any(|e| e.contains("duplicate")));
    assert!(errors.iter().any(|e| e.contains("n1")));
}

/// Submit a graph with a node whose type is not registered.
///
/// Verifies that `validate_graph` reports the unknown type name.
#[tokio::test]
async fn test_unknown_node_type() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with only LoadModel.
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a diffusion model".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "n1", "type": "NonExistent" }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("NonExistent")));
    assert!(errors.iter().any(|e| e.contains("unknown type")));
}

/// Submit a graph with an edge referencing a node that does not exist.
///
/// Verifies that `validate_graph` reports the missing source node.
#[tokio::test]
async fn test_bad_edge_ref_missing_node() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with LoadModel.
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a diffusion model".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "model", "type": "LoadModel" }
        ],
        "edges": [
            { "node_id": "ghost", "output_slot": "model", "target": "sampler", "target_slot": "model" }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("ghost")));
    assert!(errors.iter().any(|e| e.contains("missing source node")));
}

/// Submit a graph with an edge referencing an output slot that does not
/// exist on the source node.
///
/// Verifies that `validate_graph` reports the missing slot.
#[tokio::test]
async fn test_bad_edge_ref_missing_slot() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with LoadModel (outputs "model" only).
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a diffusion model".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "model", "type": "LoadModel" }
        ],
        "edges": [
            { "node_id": "model", "output_slot": "nonexistent", "target": "sampler", "target_slot": "model" }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("nonexistent")));
    assert!(errors.iter().any(|e| e.contains("no output slot")));
}

/// Submit a graph with an edge connecting incompatible slot types.
///
/// LoadModel outputs `Model` type, SaveImage inputs `Image` type.
/// These are incompatible (neither is `Any`), so validation fails.
#[tokio::test]
async fn test_slot_type_mismatch() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with LoadModel (outputs Model) and SaveImage (inputs Image).
    registry
        .update_from_worker(
            "worker-0",
            vec![
                NodeTypeDescriptor {
                    type_name: "LoadModel".to_string(),
                    display_name: "Load Model".to_string(),
                    category: "loading".to_string(),
                    description: "Loads a diffusion model".to_string(),
                    inputs: vec![],
                    outputs: vec![SlotDescriptor {
                        name: "model".to_string(),
                        slot_type: SlotType::Model,
                        optional: false,
                    }],
                },
                NodeTypeDescriptor {
                    type_name: "SaveImage".to_string(),
                    display_name: "Save Image".to_string(),
                    category: "output".to_string(),
                    description: "Saves an image".to_string(),
                    inputs: vec![SlotDescriptor {
                        name: "image".to_string(),
                        slot_type: SlotType::Image,
                        optional: false,
                    }],
                    outputs: vec![],
                },
            ],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "model", "type": "LoadModel" },
            { "id": "save", "type": "SaveImage" }
        ],
        "edges": [
            { "node_id": "model", "output_slot": "model", "target": "save", "target_slot": "image" }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("type mismatch")));
    // SlotType debug format uses PascalCase (e.g. "Model", "Image").
    assert!(errors.iter().any(|e| e.contains("Model")));
    assert!(errors.iter().any(|e| e.contains("Image")));
}

/// Submit a graph with a cycle: A → B → C → A.
///
/// Verifies that `validate_graph` detects the cycle and reports all
/// three nodes in the cycle.
#[tokio::test]
async fn test_cycle_detected() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with three node types.
    registry
        .update_from_worker(
            "worker-0",
            vec![
                NodeTypeDescriptor {
                    type_name: "NodeA".to_string(),
                    display_name: "Node A".to_string(),
                    category: "test".to_string(),
                    description: "Node A".to_string(),
                    inputs: vec![SlotDescriptor {
                        name: "input".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                    outputs: vec![SlotDescriptor {
                        name: "output".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                },
                NodeTypeDescriptor {
                    type_name: "NodeB".to_string(),
                    display_name: "Node B".to_string(),
                    category: "test".to_string(),
                    description: "Node B".to_string(),
                    inputs: vec![SlotDescriptor {
                        name: "input".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                    outputs: vec![SlotDescriptor {
                        name: "output".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                },
                NodeTypeDescriptor {
                    type_name: "NodeC".to_string(),
                    display_name: "Node C".to_string(),
                    category: "test".to_string(),
                    description: "Node C".to_string(),
                    inputs: vec![SlotDescriptor {
                        name: "input".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                    outputs: vec![SlotDescriptor {
                        name: "output".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                },
            ],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "A", "type": "NodeA" },
            { "id": "B", "type": "NodeB" },
            { "id": "C", "type": "NodeC" }
        ],
        "edges": [
            { "node_id": "A", "output_slot": "output", "target": "B", "target_slot": "input" },
            { "node_id": "B", "output_slot": "output", "target": "C", "target_slot": "input" },
            { "node_id": "C", "output_slot": "output", "target": "A", "target_slot": "input" }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("cycle")));
    // All three nodes should be named in the cycle error.
    let cycle_error = errors.iter().find(|e| e.contains("cycle")).unwrap();
    assert!(cycle_error.contains("A"));
    assert!(cycle_error.contains("B"));
    assert!(cycle_error.contains("C"));
}

/// Submit a complete valid graph: LoadModel → Sampler → VaeDecode → SaveImage.
///
/// Verifies that `validate_graph` returns `Ok(ValidatedGraph)` when all
/// six checks pass.
#[tokio::test]
async fn test_valid_graph_returns_ok() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with all four node types needed for the graph.
    registry
        .update_from_worker(
            "worker-0",
            vec![
                // LoadModel: outputs model (Model type)
                NodeTypeDescriptor {
                    type_name: "LoadModel".to_string(),
                    display_name: "Load Model".to_string(),
                    category: "loading".to_string(),
                    description: "Loads a diffusion model".to_string(),
                    inputs: vec![],
                    outputs: vec![SlotDescriptor {
                        name: "model".to_string(),
                        slot_type: SlotType::Model,
                        optional: false,
                    }],
                },
                // Sampler: inputs model (Model), positive (Conditioning);
                // outputs samples (Latent)
                NodeTypeDescriptor {
                    type_name: "Sampler".to_string(),
                    display_name: "Sampler".to_string(),
                    category: "sampling".to_string(),
                    description: "Runs a diffusion sampling step".to_string(),
                    inputs: vec![
                        SlotDescriptor {
                            name: "model".to_string(),
                            slot_type: SlotType::Model,
                            optional: false,
                        },
                        SlotDescriptor {
                            name: "positive".to_string(),
                            slot_type: SlotType::Conditioning,
                            optional: false,
                        },
                    ],
                    outputs: vec![SlotDescriptor {
                        name: "samples".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                },
                // VaeDecode: inputs samples (Latent); outputs image (Image)
                NodeTypeDescriptor {
                    type_name: "VaeDecode".to_string(),
                    display_name: "VAE Decode".to_string(),
                    category: "decoding".to_string(),
                    description: "Decodes latent to image".to_string(),
                    inputs: vec![SlotDescriptor {
                        name: "samples".to_string(),
                        slot_type: SlotType::Latent,
                        optional: false,
                    }],
                    outputs: vec![SlotDescriptor {
                        name: "image".to_string(),
                        slot_type: SlotType::Image,
                        optional: false,
                    }],
                },
                // SaveImage: inputs image (Image)
                NodeTypeDescriptor {
                    type_name: "SaveImage".to_string(),
                    display_name: "Save Image".to_string(),
                    category: "output".to_string(),
                    description: "Saves an image to disk".to_string(),
                    inputs: vec![SlotDescriptor {
                        name: "image".to_string(),
                        slot_type: SlotType::Image,
                        optional: false,
                    }],
                    outputs: vec![],
                },
            ],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "model", "type": "LoadModel" },
            { "id": "sampler", "type": "Sampler" },
            { "id": "decode", "type": "VaeDecode" },
            { "id": "save", "type": "SaveImage" }
        ],
        "edges": [
            {
                "node_id": "model",
                "output_slot": "model",
                "target": "sampler",
                "target_slot": "model"
            },
            {
                "node_id": "sampler",
                "output_slot": "samples",
                "target": "decode",
                "target_slot": "samples"
            },
            {
                "node_id": "decode",
                "output_slot": "image",
                "target": "save",
                "target_slot": "image"
            }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_ok());
    let validated = result.unwrap();
    // The ValidatedGraph wraps the original graph value.
    assert!(matches!(validated, ValidatedGraph(_)));
}

/// Submit a graph with both duplicate IDs and an unknown type.
///
/// Verifies that `validate_graph` returns ≥ 2 error strings in a single
/// response, confirming non-fail-fast behaviour.
#[tokio::test]
async fn test_multiple_errors_collected() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with only LoadModel.
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a diffusion model".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "n1", "type": "LoadModel" },
            { "id": "n1", "type": "NonExistent" }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_err());
    let errors = result.unwrap_err();
    // Should have at least 2 errors: duplicate ID + unknown type.
    assert!(
        errors.len() >= 2,
        "expected ≥ 2 errors, got {}: {:?}",
        errors.len(),
        errors
    );
    // Verify both error types are present.
    assert!(errors.iter().any(|e| e.contains("duplicate")));
    assert!(errors.iter().any(|e| e.contains("NonExistent")));
}

/// Submit a graph with an edge connecting an `Any` output to a `Model` input.
///
/// Verifies that `SlotType::Any` is compatible with any concrete type,
/// so no type mismatch error is produced.
#[tokio::test]
async fn test_any_slot_type_compatible() {
    let registry = NodeTypeRegistry::new().await;

    // Populate registry with two node types.
    // NodeAny outputs "out" as Any type.
    // NodeModel inputs "model" as Model type.
    registry
        .update_from_worker(
            "worker-0",
            vec![
                NodeTypeDescriptor {
                    type_name: "NodeAny".to_string(),
                    display_name: "Node Any".to_string(),
                    category: "test".to_string(),
                    description: "Outputs Any type".to_string(),
                    inputs: vec![],
                    outputs: vec![SlotDescriptor {
                        name: "out".to_string(),
                        slot_type: SlotType::Any,
                        optional: false,
                    }],
                },
                NodeTypeDescriptor {
                    type_name: "NodeModel".to_string(),
                    display_name: "Node Model".to_string(),
                    category: "test".to_string(),
                    description: "Inputs Model type".to_string(),
                    inputs: vec![SlotDescriptor {
                        name: "model".to_string(),
                        slot_type: SlotType::Model,
                        optional: false,
                    }],
                    outputs: vec![],
                },
            ],
        )
        .await;

    let graph = serde_json::json!({
        "nodes": [
            { "id": "any", "type": "NodeAny" },
            { "id": "model", "type": "NodeModel" }
        ],
        "edges": [
            {
                "node_id": "any",
                "output_slot": "out",
                "target": "model",
                "target_slot": "model"
            }
        ]
    });

    let result = validate_graph(&graph, &registry).await;
    assert!(result.is_ok());
    let validated = result.unwrap();
    assert!(matches!(validated, ValidatedGraph(_)));
}
