use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor, SlotType};
use anvilml_scheduler::{GraphError, ValidatedGraph, validate_graph};

/// Test that the inner serde_json::Value is accessible within the crate
/// via the #[cfg(test)] _test_inner() method, confirming pub(crate)
/// visibility works correctly: same-crate test code can inspect the
/// graph through the helper, proving the field is not pub (no direct
/// field access from the test crate) but is accessible within the
/// crate boundary.
#[test]
fn test_validated_graph_inner_is_pub_crate() {
    let inner = serde_json::json!({"nodes": []});
    let vg = ValidatedGraph::_test_new(inner.clone());
    let retrieved = vg._test_inner();
    assert_eq!(
        retrieved, &inner,
        "_test_inner() must return a reference to the same serde_json::Value"
    );
}

/// Test that ValidatedGraph correctly derives Debug and Clone.
///
/// Verifies that format!("{:?}", ...) produces a non-empty string
/// containing "ValidatedGraph", and that cloning produces an equal value.
#[test]
fn test_validated_graph_derives_debug_and_clone() {
    let inner = serde_json::json!({"nodes": [], "edges": []});
    let vg = ValidatedGraph::_test_new(inner);

    // Debug derive: format!("{:?}", ...) must produce a string containing "ValidatedGraph"
    let debug_str = format!("{:?}", vg);
    assert!(
        debug_str.contains("ValidatedGraph"),
        "Debug output should contain the struct name, got: {debug_str}"
    );

    // Clone derive: cloned value must be equal to the original
    let cloned = vg.clone();
    assert_eq!(
        format!("{:?}", cloned),
        debug_str,
        "Cloned ValidatedGraph must produce the same Debug representation"
    );
}

/// Test that GraphError::NotAnObject produces the correct Display string.
///
/// Verifies the Display output is non-empty and equals the exact expected
/// message defined in the enum's #[error(...)] attribute.
#[test]
fn test_graph_error_not_an_object_display() {
    let err = GraphError::NotAnObject;
    let msg = err.to_string();
    assert!(!msg.is_empty(), "Display must not be empty");
    assert_eq!(msg, "root is not an object");
}

/// Test that GraphError::MissingNodesArray produces a non-empty Display string.
///
/// Verifies the Display output is non-empty, confirming the #[error(...)]
/// attribute is correctly wired.
#[test]
fn test_graph_error_missing_nodes_array_display() {
    let err = GraphError::MissingNodesArray;
    let msg = err.to_string();
    assert!(!msg.is_empty(), "Display must not be empty");
}

/// Test that GraphError::DuplicateNodeId includes the node ID in Display output.
///
/// Constructs the error with "node_a" and verifies the ID appears in the
/// Display string, confirming struct-field interpolation works.
#[test]
fn test_graph_error_duplicate_node_id_display() {
    let err = GraphError::DuplicateNodeId("node_a".into());
    let msg = err.to_string();
    assert!(
        msg.contains("node_a"),
        "Display must contain the duplicate node ID, got: {msg}"
    );
}

/// Test that GraphError::UnknownNodeType includes both node_id and type_name.
///
/// Constructs with node_id="n1" and type_name="BadNode", verifies both
/// identifiers appear in the Display output.
#[test]
fn test_graph_error_unknown_node_type_display() {
    let err = GraphError::UnknownNodeType {
        node_id: "n1".into(),
        type_name: "BadNode".into(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("n1"),
        "Display must contain node_id, got: {msg}"
    );
    assert!(
        msg.contains("BadNode"),
        "Display must contain type_name, got: {msg}"
    );
}

/// Test that GraphError::DanglingEdge includes both node_id and slot_name.
///
/// Constructs with node_id="n2" and slot_name="output", verifies both
/// identifiers appear in the Display output.
#[test]
fn test_graph_error_dangling_edge_display() {
    let err = GraphError::DanglingEdge {
        node_id: "n2".into(),
        slot_name: "output".into(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("n2"),
        "Display must contain node_id, got: {msg}"
    );
    assert!(
        msg.contains("output"),
        "Display must contain slot_name, got: {msg}"
    );
}

/// Test that GraphError::SlotTypeMismatch includes all four fields.
///
/// Constructs with node_id="n3", slot_name="in", expected="FLOAT",
/// found="INT", verifies all four values appear in the Display output.
#[test]
fn test_graph_error_slot_type_mismatch_display() {
    let err = GraphError::SlotTypeMismatch {
        node_id: "n3".into(),
        slot_name: "in".into(),
        expected: "FLOAT".into(),
        found: "INT".into(),
    };
    let msg = err.to_string();
    assert!(
        msg.contains("n3"),
        "Display must contain node_id, got: {msg}"
    );
    assert!(
        msg.contains("in"),
        "Display must contain slot_name, got: {msg}"
    );
    assert!(
        msg.contains("FLOAT"),
        "Display must contain expected type, got: {msg}"
    );
    assert!(
        msg.contains("INT"),
        "Display must contain found type, got: {msg}"
    );
}

/// Test that GraphError::CycleDetected includes "cycle detected" in Display.
///
/// Constructs with three node IDs and verifies the message contains the
/// cycle indicator string.
#[test]
fn test_graph_error_cycle_detected_display() {
    let err = GraphError::CycleDetected(vec!["A".into(), "B".into(), "C".into()]);
    let msg = err.to_string();
    assert!(
        msg.contains("cycle detected"),
        "Display must contain 'cycle detected', got: {msg}"
    );
}

/// Test that all 7 GraphError variants produce pairwise distinct Display strings.
///
/// This confirms the error messages are useful for operator diagnosis —
/// no two variants produce the same string, enabling callers to
/// distinguish error classes from the Display output alone.
#[test]
fn test_graph_error_display_distinct() {
    let not_an_object = GraphError::NotAnObject.to_string();
    let missing_nodes = GraphError::MissingNodesArray.to_string();
    let duplicate_id = GraphError::DuplicateNodeId("x".into()).to_string();
    let unknown_type = GraphError::UnknownNodeType {
        node_id: "x".into(),
        type_name: "X".into(),
    }
    .to_string();
    let dangling = GraphError::DanglingEdge {
        node_id: "x".into(),
        slot_name: "s".into(),
    }
    .to_string();
    let mismatch = GraphError::SlotTypeMismatch {
        node_id: "x".into(),
        slot_name: "s".into(),
        expected: "A".into(),
        found: "B".into(),
    }
    .to_string();
    let cycle = GraphError::CycleDetected(vec!["A".into()]).to_string();

    let all = [
        not_an_object,
        missing_nodes,
        duplicate_id,
        unknown_type,
        dangling,
        mismatch,
        cycle,
    ];

    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(
                all[i], all[j],
                "Variant {} and variant {} produce the same Display string: {}",
                i, j, all[i]
            );
        }
    }
}

/// Test validate_graph check 1: a non-object root (JSON array) returns
/// Err containing exactly one NotAnObject error.
///
/// The root is a JSON array `[]`, which is not an object. validate_graph
/// must detect this and return Err([NotAnObject]) immediately, since
/// a non-object root has no "nodes" key to inspect.
#[test]
fn test_validate_graph_non_object_root_returns_not_an_object() {
    let registry = NodeTypeRegistry::new();
    let result = validate_graph(serde_json::json!([]), &registry);

    assert!(result.is_err(), "Expected Err for non-object root");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    assert!(matches!(errors[0], GraphError::NotAnObject));
}

/// Test validate_graph check 1: an object without a "nodes" key returns
/// Err containing exactly one MissingNodesArray error.
///
/// The root is a JSON object but lacks the required "nodes" key.
/// validate_graph must detect this and return Err([MissingNodesArray]).
#[test]
fn test_validate_graph_missing_nodes_array_returns_missing_nodes_array() {
    let registry = NodeTypeRegistry::new();
    let result = validate_graph(serde_json::json!({"edges": []}), &registry);

    assert!(result.is_err(), "Expected Err for missing nodes array");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    assert!(matches!(errors[0], GraphError::MissingNodesArray));
}

/// Test validate_graph check 2: duplicate node IDs are all collected in
/// a single Err, not just the first duplicate.
///
/// The graph has three nodes: id "a", id "b", id "a" (duplicate).
/// validate_graph must report the second occurrence of "a" as a
/// DuplicateNodeId error, confirming collect-all-errors semantics.
/// Only the 2nd+ occurrence of each ID is reported.
#[test]
fn test_validate_graph_duplicate_ids_all_reported() {
    let registry = NodeTypeRegistry::new();
    let graph = serde_json::json!({
        "nodes": [
            {"id": "a"},
            {"id": "b"},
            {"id": "a"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for duplicate IDs");
    let errors = result.unwrap_err();
    assert_eq!(
        errors.len(),
        1,
        "Expected one DuplicateNodeId error (second occurrence of 'a')"
    );
    assert!(matches!(errors[0], GraphError::DuplicateNodeId(ref id) if id == "a"));
}

/// Test validate_graph checks 1–2 pass with zero errors, returning
/// Ok(ValidatedGraph).
///
/// The graph has two nodes with unique IDs ("a" and "b"). Both checks
/// pass: root is an object with a "nodes" array, and no duplicates exist.
#[test]
fn test_validate_graph_no_duplicates_passes_cleanly() {
    let registry = NodeTypeRegistry::new();
    let graph = serde_json::json!({
        "nodes": [
            {"id": "a"},
            {"id": "b"}
        ]
    });
    let result = validate_graph(graph.clone(), &registry);

    assert!(result.is_ok(), "Expected Ok for clean graph");
    let validated = result.unwrap();
    assert_eq!(validated._test_inner(), &graph);
}

/// Test validate_graph check 2: multiple different duplicate IDs are all
/// reported in one Err, preserving the order of second-occurrence detection.
///
/// The graph has five nodes: a, b, a, c, b. The duplicates detected are:
/// the second "a" (3rd position), and the second "b" (5th position).
/// validate_graph must report both duplicate occurrences in order.
#[test]
fn test_validate_graph_multiple_duplicate_violations_collected() {
    let registry = NodeTypeRegistry::new();
    let graph = serde_json::json!({
        "nodes": [
            {"id": "a"},
            {"id": "b"},
            {"id": "a"},
            {"id": "c"},
            {"id": "b"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for multiple duplicate IDs");
    let errors = result.unwrap_err();
    assert_eq!(
        errors.len(),
        2,
        "Expected two DuplicateNodeId errors (second 'a' and second 'b')"
    );
    // Verify each error is a DuplicateNodeId with the expected id
    for err in &errors {
        match err {
            GraphError::DuplicateNodeId(id) => {
                assert!(
                    id == "a" || id == "b",
                    "Unexpected id in DuplicateNodeId: {id}"
                );
            }
            other => panic!("Expected DuplicateNodeId, got: {other:?}"),
        }
    }
}

// ====== Check 3: Unknown node type validation ======

/// Test check 3: a node with an unregistered type produces an
/// UnknownNodeType error with the correct node_id and type_name.
///
/// The registry is empty (no types registered). The graph has one
/// node with type "NonExistentType". validate_graph must return
/// Err containing exactly one UnknownNodeType error.
#[test]
fn test_validate_graph_unknown_node_type_reported() {
    let registry = NodeTypeRegistry::new();
    let graph = serde_json::json!({
        "nodes": [{"id": "n1", "type": "NonExistentType"}]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for unknown node type");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::UnknownNodeType { node_id, type_name } => {
            assert_eq!(node_id, "n1", "node_id must match");
            assert_eq!(type_name, "NonExistentType", "type_name must match");
        }
        other => panic!("Expected UnknownNodeType, got: {other:?}"),
    }
}

/// Test check 3: a node with a registered type passes cleanly.
///
/// The registry contains "LoadModel" with two output slots. The graph
/// has one node with type "LoadModel". validate_graph must return
/// Ok(ValidatedGraph) — no unknown type errors.
#[test]
fn test_validate_graph_valid_type_passes_check3() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "LoadModel".into(),
        display_name: "Load Model".into(),
        category: "loaders".into(),
        description: "Loads a model checkpoint".into(),
        inputs: vec![],
        outputs: vec![
            SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            },
            SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Clip,
                optional: false,
            },
        ],
    }]);

    let graph = serde_json::json!({
        "nodes": [{"id": "n1", "type": "LoadModel"}]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_ok(),
        "Expected Ok for registered type, got errors: {:?}",
        result.err()
    );
}

// ====== Check 4: Dangling edge validation ======

/// Test check 4: an edge referencing a node that does not exist
/// produces a DanglingEdge error.
///
/// The graph has one node "a" and an edge from "nonexistent:output"
/// which references a node that is not in the nodes array.
#[test]
fn test_validate_graph_edge_to_nonexistent_node() {
    let registry = NodeTypeRegistry::new();
    let graph = serde_json::json!({
        "nodes": [{"id": "a"}],
        "edges": [{"from": "nonexistent:output"}]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_err(),
        "Expected Err for dangling edge to nonexistent node"
    );
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::DanglingEdge { node_id, slot_name } => {
            assert_eq!(node_id, "nonexistent", "node_id must match");
            assert_eq!(slot_name, "output", "slot_name must match");
        }
        other => panic!("Expected DanglingEdge, got: {other:?}"),
    }
}

/// Test check 4: an edge referencing a valid node but an undeclared
/// output slot produces a DanglingEdge error.
///
/// The registry contains "LoadModel" with outputs ["MODEL", "CLIP"].
/// The graph has node "a" of type "LoadModel" and an edge from
/// "a:nonexistent_slot" which does not match any declared output.
#[test]
fn test_validate_graph_edge_to_undeclared_slot() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "LoadModel".into(),
        display_name: "Load Model".into(),
        category: "loaders".into(),
        description: "Loads a model checkpoint".into(),
        inputs: vec![],
        outputs: vec![
            SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            },
            SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Clip,
                optional: false,
            },
        ],
    }]);

    let graph = serde_json::json!({
        "nodes": [{"id": "a", "type": "LoadModel"}],
        "edges": [{"from": "a:nonexistent_slot"}]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for undeclared slot");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::DanglingEdge { node_id, slot_name } => {
            assert_eq!(node_id, "a", "node_id must match");
            assert_eq!(slot_name, "nonexistent_slot", "slot_name must match");
        }
        other => panic!("Expected DanglingEdge, got: {other:?}"),
    }
}

/// Test check 4: a graph with valid edges and registered types
/// passes cleanly.
///
/// The registry contains "LoadModel" with outputs ["MODEL", "CLIP"].
/// The graph has node "a" of type "LoadModel" and an edge from
/// "a:MODEL" which matches a declared output slot.
#[test]
fn test_validate_graph_valid_edges_pass_cleanly() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "LoadModel".into(),
        display_name: "Load Model".into(),
        category: "loaders".into(),
        description: "Loads a model checkpoint".into(),
        inputs: vec![],
        outputs: vec![
            SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            },
            SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Clip,
                optional: false,
            },
        ],
    }]);

    let graph = serde_json::json!({
        "nodes": [{"id": "a", "type": "LoadModel"}],
        "edges": [{"from": "a:MODEL"}]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_ok(),
        "Expected Ok for valid edges, got errors: {:?}",
        result.err()
    );
}

/// Test checks 3 and 4 together: multiple violations across unknown
/// types and dangling edges are all collected in one Err.
///
/// The registry is empty. The graph has two nodes with unregistered
/// types ("Foo" and "Bar") and one edge referencing a nonexistent node.
/// validate_graph must return Err with exactly 3 errors: two
/// UnknownNodeType + one DanglingEdge.
#[test]
fn test_validate_graph_multiple_violations_collected() {
    let registry = NodeTypeRegistry::new();
    let graph = serde_json::json!({
        "nodes": [
            {"id": "n1", "type": "Foo"},
            {"id": "n2", "type": "Bar"}
        ],
        "edges": [{"from": "nonexistent:out"}]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for multiple violations");
    let errors = result.unwrap_err();
    assert_eq!(
        errors.len(),
        3,
        "Expected exactly 3 errors (2 UnknownNodeType + 1 DanglingEdge), got {}",
        errors.len()
    );

    // Verify the first two errors are UnknownNodeType
    match &errors[0] {
        GraphError::UnknownNodeType { node_id, type_name } => {
            assert_eq!(node_id, "n1");
            assert_eq!(type_name, "Foo");
        }
        other => panic!("Expected UnknownNodeType, got: {other:?}"),
    }
    match &errors[1] {
        GraphError::UnknownNodeType { node_id, type_name } => {
            assert_eq!(node_id, "n2");
            assert_eq!(type_name, "Bar");
        }
        other => panic!("Expected UnknownNodeType, got: {other:?}"),
    }
    // Verify the third error is DanglingEdge
    match &errors[2] {
        GraphError::DanglingEdge { node_id, slot_name } => {
            assert_eq!(node_id, "nonexistent");
            assert_eq!(slot_name, "out");
        }
        other => panic!("Expected DanglingEdge, got: {other:?}"),
    }
}

// ====== Check 5: Slot-type compatibility validation ======

/// Test check 5: a MODEL→CLIP slot type mismatch produces a
/// SlotTypeMismatch error with the correct fields.
///
/// The registry contains "LoadModel" with outputs MODEL (SlotType::Model)
/// and CLIP (SlotType::Clip), and "ClipTextEncode" with input CLIP
/// (SlotType::Clip). The edge connects LoadModel's MODEL output to
/// ClipTextEncode's CLIP input — a type mismatch (Model ≠ Clip).
#[test]
fn test_validate_graph_slot_type_mismatch_reported() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "LoadModel".into(),
            display_name: "Load Model".into(),
            category: "loaders".into(),
            description: "Loads a model checkpoint".into(),
            inputs: vec![],
            outputs: vec![
                SlotDescriptor {
                    name: "MODEL".into(),
                    slot_type: SlotType::Model,
                    optional: false,
                },
                SlotDescriptor {
                    name: "CLIP".into(),
                    slot_type: SlotType::Clip,
                    optional: false,
                },
            ],
        },
        NodeTypeDescriptor {
            type_name: "ClipTextEncode".into(),
            display_name: "Clip Text Encode".into(),
            category: "conditioning".into(),
            description: "Encodes text with CLIP".into(),
            inputs: vec![SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Clip,
                optional: false,
            }],
            outputs: vec![],
        },
    ]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "LoadModel"},
            {"id": "b", "type": "ClipTextEncode"}
        ],
        "edges": [
            {"from": "a:MODEL", "to": "b:CLIP"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for slot type mismatch");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::SlotTypeMismatch {
            node_id,
            slot_name,
            expected,
            found,
        } => {
            assert_eq!(node_id, "b", "node_id must be destination node");
            assert_eq!(slot_name, "CLIP", "slot_name must match destination slot");
            assert_eq!(
                expected, "Clip",
                "expected must be the input slot's type label, got: {expected}"
            );
            assert_eq!(
                found, "Model",
                "found must be the output slot's type label, got: {found}"
            );
        }
        other => panic!("Expected SlotTypeMismatch, got: {other:?}"),
    }
}

/// Test check 5: matching output/input types pass cleanly.
///
/// The registry contains "LoadModel" with MODEL output and
/// "CLIPTextEncode" with MODEL input. The edge connects MODEL→MODEL,
/// which is an exact type match — no SlotTypeMismatch should be produced.
#[test]
fn test_validate_graph_exact_slot_type_match_passes() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "LoadModel".into(),
            display_name: "Load Model".into(),
            category: "loaders".into(),
            description: "Loads a model checkpoint".into(),
            inputs: vec![],
            outputs: vec![SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
        },
        NodeTypeDescriptor {
            type_name: "CLIPTextEncode".into(),
            display_name: "Clip Text Encode".into(),
            category: "conditioning".into(),
            description: "Encodes text with CLIP".into(),
            inputs: vec![SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
            outputs: vec![],
        },
    ]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "LoadModel"},
            {"id": "b", "type": "CLIPTextEncode"}
        ],
        "edges": [
            {"from": "a:MODEL", "to": "b:CLIP"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_ok(),
        "Expected Ok for matching types, got errors: {:?}",
        result.err()
    );
}

/// Test check 5: source output is SlotType::Any — passes regardless
/// of destination type.
///
/// The registry contains "AnyOutput" with Any output and
/// "Consumer" with MODEL input. The edge connects Any→MODEL — since
/// the source is Any, no mismatch is reported.
#[test]
fn test_validate_graph_any_on_source_side_passes() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "AnyOutput".into(),
            display_name: "Any Output".into(),
            category: "utility".into(),
            description: "Outputs any type".into(),
            inputs: vec![],
            outputs: vec![SlotDescriptor {
                name: "ANY".into(),
                slot_type: SlotType::Any,
                optional: false,
            }],
        },
        NodeTypeDescriptor {
            type_name: "Consumer".into(),
            display_name: "Consumer".into(),
            category: "utility".into(),
            description: "Consumes a model".into(),
            inputs: vec![SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
            outputs: vec![],
        },
    ]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "AnyOutput"},
            {"id": "b", "type": "Consumer"}
        ],
        "edges": [
            {"from": "a:ANY", "to": "b:MODEL"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_ok(),
        "Expected Ok when source is Any, got errors: {:?}",
        result.err()
    );
}

/// Test check 5: destination input is SlotType::Any — passes regardless
/// of source type.
///
/// The registry contains "Producer" with MODEL output and
/// "AnyConsumer" with Any input. The edge connects MODEL→Any — since
/// the destination is Any, no mismatch is reported.
#[test]
fn test_validate_graph_any_on_dest_side_passes() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "Producer".into(),
            display_name: "Producer".into(),
            category: "utility".into(),
            description: "Produces a model".into(),
            inputs: vec![],
            outputs: vec![SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
        },
        NodeTypeDescriptor {
            type_name: "AnyConsumer".into(),
            display_name: "Any Consumer".into(),
            category: "utility".into(),
            description: "Consumes any type".into(),
            inputs: vec![SlotDescriptor {
                name: "ANY".into(),
                slot_type: SlotType::Any,
                optional: false,
            }],
            outputs: vec![],
        },
    ]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "Producer"},
            {"id": "b", "type": "AnyConsumer"}
        ],
        "edges": [
            {"from": "a:MODEL", "to": "b:ANY"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_ok(),
        "Expected Ok when dest is Any, got errors: {:?}",
        result.err()
    );
}

/// Test check 5: an edge flagged DanglingEdge in check 4 is not also
/// reported as SlotTypeMismatch.
///
/// The registry is empty (no types registered). The graph has two nodes
/// with unregistered types and an edge from "a:MODEL" to "b:CLIP".
/// Check 4 will flag both nodes as DanglingEdge (unknown types → skip).
/// Check 5 must not double-report this edge.
#[test]
fn test_validate_graph_dangling_edge_not_double_reported() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "Foo".into(),
            display_name: "Foo".into(),
            category: "test".into(),
            description: "Foo node".into(),
            inputs: vec![],
            outputs: vec![SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
        },
        NodeTypeDescriptor {
            type_name: "Bar".into(),
            display_name: "Bar".into(),
            category: "test".into(),
            description: "Bar node".into(),
            inputs: vec![SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Clip,
                optional: false,
            }],
            outputs: vec![],
        },
    ]);

    // Both "a" and "b" reference registered types, so check 4 passes.
    // But the edge from "a:MODEL" to "b:CLIP" has a type mismatch.
    // This test verifies that the edge IS reported as SlotTypeMismatch
    // when both nodes are valid (not dangling).
    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "Foo"},
            {"id": "b", "type": "Bar"}
        ],
        "edges": [
            {"from": "a:MODEL", "to": "b:CLIP"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for slot type mismatch");
    let errors = result.unwrap_err();
    // Only one error: the SlotTypeMismatch. No DanglingEdge because
    // both nodes are registered and declare their slots.
    assert_eq!(
        errors.len(),
        1,
        "Expected exactly one error (SlotTypeMismatch), got {}: {:?}",
        errors.len(),
        errors
    );
    assert!(
        matches!(errors[0], GraphError::SlotTypeMismatch { .. }),
        "Expected SlotTypeMismatch, got: {:?}",
        errors[0]
    );
}

/// Test check 5: two edges with different mismatches produce two
/// SlotTypeMismatch errors collected in one Err.
///
/// The registry contains "NodeA" with MODEL output, "NodeB" with
/// CLIP output, and "NodeC" with MODEL input and CLIP input.
/// Edge 1: NodeA:MODEL → NodeC:MODEL (exact match, passes).
/// Edge 2: NodeB:CLIP → NodeC:CLIP (exact match, passes).
/// We need a mismatch — let me restructure:
/// NodeA:MODEL → NodeC:CLIP (mismatch)
/// NodeB:CLIP → NodeC:MODEL (mismatch)
#[test]
fn test_validate_graph_multiple_slot_type_mismatches_collected() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "NodeA".into(),
            display_name: "Node A".into(),
            category: "test".into(),
            description: "Node A".into(),
            inputs: vec![],
            outputs: vec![SlotDescriptor {
                name: "MODEL".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
        },
        NodeTypeDescriptor {
            type_name: "NodeB".into(),
            display_name: "Node B".into(),
            category: "test".into(),
            description: "Node B".into(),
            inputs: vec![],
            outputs: vec![SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Clip,
                optional: false,
            }],
        },
        NodeTypeDescriptor {
            type_name: "NodeC".into(),
            display_name: "Node C".into(),
            category: "test".into(),
            description: "Node C".into(),
            inputs: vec![
                SlotDescriptor {
                    name: "MODEL".into(),
                    slot_type: SlotType::Model,
                    optional: false,
                },
                SlotDescriptor {
                    name: "CLIP".into(),
                    slot_type: SlotType::Clip,
                    optional: false,
                },
            ],
            outputs: vec![],
        },
    ]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "NodeA"},
            {"id": "b", "type": "NodeB"},
            {"id": "c", "type": "NodeC"}
        ],
        "edges": [
            {"from": "a:MODEL", "to": "c:CLIP"},
            {"from": "b:CLIP", "to": "c:MODEL"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_err(),
        "Expected Err for multiple mismatches, got Ok"
    );
    let errors = result.unwrap_err();
    assert_eq!(
        errors.len(),
        2,
        "Expected two SlotTypeMismatch errors, got {}: {:?}",
        errors.len(),
        errors
    );
    // Both errors must be SlotTypeMismatch
    for err in &errors {
        assert!(
            matches!(err, GraphError::SlotTypeMismatch { .. }),
            "Expected SlotTypeMismatch, got: {:?}",
            err
        );
    }
}

// ====== Check 6: Cycle detection (Kahn's algorithm) ======

/// Test check 6: a 2-node cycle (a→b, b→a) produces a CycleDetected
/// error containing both node IDs.
///
/// The registry contains "NodeA" with output "OUT" and "NodeB" with
/// input "IN". The graph has nodes "a" and "b" connected in both
/// directions, forming a cycle. validate_graph must detect both nodes
/// as cycle participants.
#[test]
fn test_validate_graph_simple_two_node_cycle() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "NodeA".into(),
            display_name: "Node A".into(),
            category: "test".into(),
            description: "Node A".into(),
            inputs: vec![SlotDescriptor {
                name: "IN".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
            outputs: vec![SlotDescriptor {
                name: "OUT".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
        },
        NodeTypeDescriptor {
            type_name: "NodeB".into(),
            display_name: "Node B".into(),
            category: "test".into(),
            description: "Node B".into(),
            inputs: vec![SlotDescriptor {
                name: "IN".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
            outputs: vec![SlotDescriptor {
                name: "OUT".into(),
                slot_type: SlotType::Model,
                optional: false,
            }],
        },
    ]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "NodeA"},
            {"id": "b", "type": "NodeB"}
        ],
        "edges": [
            {"from": "a:OUT", "to": "b:IN"},
            {"from": "b:OUT", "to": "a:IN"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for 2-node cycle");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::CycleDetected(nodes) => {
            assert_eq!(
                nodes.len(),
                2,
                "Expected both nodes in cycle, got: {:?}",
                nodes
            );
            assert!(
                nodes.contains(&"a".to_string()),
                "Cycle must include node 'a'"
            );
            assert!(
                nodes.contains(&"b".to_string()),
                "Cycle must include node 'b'"
            );
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

/// Test check 6: a 3-node cycle (a→b→c→a) produces a CycleDetected
/// error containing all three node IDs.
///
/// The registry contains "NodeX" with output "OUT" and input "IN".
/// The graph has nodes "a", "b", "c" connected in a cycle.
/// validate_graph must detect all three nodes as cycle participants.
#[test]
fn test_validate_graph_three_node_cycle() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "NodeX".into(),
        display_name: "Node X".into(),
        category: "test".into(),
        description: "Node X".into(),
        inputs: vec![SlotDescriptor {
            name: "IN".into(),
            slot_type: SlotType::Model,
            optional: false,
        }],
        outputs: vec![SlotDescriptor {
            name: "OUT".into(),
            slot_type: SlotType::Model,
            optional: false,
        }],
    }]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "NodeX"},
            {"id": "b", "type": "NodeX"},
            {"id": "c", "type": "NodeX"}
        ],
        "edges": [
            {"from": "a:OUT", "to": "b:IN"},
            {"from": "b:OUT", "to": "c:IN"},
            {"from": "c:OUT", "to": "a:IN"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for 3-node cycle");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::CycleDetected(nodes) => {
            assert_eq!(
                nodes.len(),
                3,
                "Expected all 3 nodes in cycle, got: {:?}",
                nodes
            );
            assert!(nodes.contains(&"a".to_string()), "Cycle must include 'a'");
            assert!(nodes.contains(&"b".to_string()), "Cycle must include 'b'");
            assert!(nodes.contains(&"c".to_string()), "Cycle must include 'c'");
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

/// Test check 6: a fully valid acyclic graph with all six checks passing
/// returns Ok(ValidatedGraph).
///
/// The registry contains "LoadModel" (outputs MODEL, CLIP) and
/// "ClipTextEncode" (input CLIP). The graph has two nodes connected
/// with a single edge a→b (no cycles), all types registered, and
/// correct slot types. validate_graph must return Ok.
#[test]
fn test_validate_graph_acyclic_graph_with_all_checks_passing() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![
        NodeTypeDescriptor {
            type_name: "LoadModel".into(),
            display_name: "Load Model".into(),
            category: "loaders".into(),
            description: "Loads a model checkpoint".into(),
            inputs: vec![],
            outputs: vec![
                SlotDescriptor {
                    name: "MODEL".into(),
                    slot_type: SlotType::Model,
                    optional: false,
                },
                SlotDescriptor {
                    name: "CLIP".into(),
                    slot_type: SlotType::Clip,
                    optional: false,
                },
            ],
        },
        NodeTypeDescriptor {
            type_name: "ClipTextEncode".into(),
            display_name: "Clip Text Encode".into(),
            category: "conditioning".into(),
            description: "Encodes text with CLIP".into(),
            inputs: vec![SlotDescriptor {
                name: "CLIP".into(),
                slot_type: SlotType::Clip,
                optional: false,
            }],
            outputs: vec![],
        },
    ]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "load_model_0", "type": "LoadModel"},
            {"id": "clip_encode_0", "type": "ClipTextEncode"}
        ],
        "edges": [
            {"from": "load_model_0:CLIP", "to": "clip_encode_0:CLIP"}
        ]
    });
    let result = validate_graph(graph.clone(), &registry);

    assert!(
        result.is_ok(),
        "Expected Ok for fully valid acyclic graph, got errors: {:?}",
        result.err()
    );
    let validated = result.unwrap();
    assert_eq!(validated._test_inner(), &graph);
}

/// Test check 6 combined with check 3: a graph with both a cycle
/// and an unknown node type produces errors for both violations
/// in a single Err(Vec).
///
/// The registry is empty (no types registered). The graph has nodes
/// "a" and "b" with unregistered types, connected in a cycle.
/// validate_graph must collect both UnknownNodeType and CycleDetected
/// errors in one Err.
#[test]
fn test_validate_graph_cycle_with_other_violations() {
    let registry = NodeTypeRegistry::new();
    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "UnknownType1"},
            {"id": "b", "type": "UnknownType2"}
        ],
        "edges": [
            {"from": "a:OUT", "to": "b:IN"},
            {"from": "b:OUT", "to": "a:IN"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(
        result.is_err(),
        "Expected Err for graph with cycle + unknown types"
    );
    let errors = result.unwrap_err();
    // Should have: 2 UnknownNodeType + 1 CycleDetected = 3 errors
    assert_eq!(
        errors.len(),
        3,
        "Expected 3 errors (2 UnknownNodeType + 1 CycleDetected), got {}",
        errors.len()
    );

    // Verify the first two are UnknownNodeType
    match &errors[0] {
        GraphError::UnknownNodeType { node_id, type_name } => {
            assert_eq!(node_id, "a");
            assert_eq!(type_name, "UnknownType1");
        }
        other => panic!("Expected UnknownNodeType, got: {:?}", other),
    }
    match &errors[1] {
        GraphError::UnknownNodeType { node_id, type_name } => {
            assert_eq!(node_id, "b");
            assert_eq!(type_name, "UnknownType2");
        }
        other => panic!("Expected UnknownNodeType, got: {:?}", other),
    }
    // Verify the third is CycleDetected
    match &errors[2] {
        GraphError::CycleDetected(nodes) => {
            assert_eq!(
                nodes.len(),
                2,
                "Cycle must include both nodes, got: {:?}",
                nodes
            );
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

/// Test check 6: a graph with nodes but no edges is trivially acyclic.
///
/// With no edges, in-degrees are all 0, so all nodes are processed
/// and no cycle is detected. validate_graph must return Ok.
#[test]
fn test_validate_graph_no_edges_no_cycle() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "NodeX".into(),
        display_name: "Node X".into(),
        category: "test".into(),
        description: "Node X".into(),
        inputs: vec![],
        outputs: vec![],
    }]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "NodeX"},
            {"id": "b", "type": "NodeX"},
            {"id": "c", "type": "NodeX"}
        ]
    });
    let result = validate_graph(graph.clone(), &registry);

    assert!(
        result.is_ok(),
        "Expected Ok for graph with nodes but no edges, got errors: {:?}",
        result.err()
    );
    let validated = result.unwrap();
    assert_eq!(validated._test_inner(), &graph);
}

/// Test check 6: a single-node self-loop (a→a) produces
/// CycleDetected(["a"]).
///
/// The registry contains "NodeX" with matching input/output slots.
/// The graph has one node "a" with an edge from "a" to "a".
/// validate_graph must detect this as a cycle.
#[test]
fn test_validate_graph_self_loop_cycle() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "NodeX".into(),
        display_name: "Node X".into(),
        category: "test".into(),
        description: "Node X".into(),
        inputs: vec![SlotDescriptor {
            name: "IN".into(),
            slot_type: SlotType::Model,
            optional: false,
        }],
        outputs: vec![SlotDescriptor {
            name: "OUT".into(),
            slot_type: SlotType::Model,
            optional: false,
        }],
    }]);

    let graph = serde_json::json!({
        "nodes": [{"id": "a", "type": "NodeX"}],
        "edges": [{"from": "a:OUT", "to": "a:IN"}]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for self-loop cycle");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::CycleDetected(nodes) => {
            assert_eq!(
                nodes.len(),
                1,
                "Expected exactly one node in cycle, got: {:?}",
                nodes
            );
            assert_eq!(nodes[0], "a", "Cycle must include only node 'a'");
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}

/// Test check 6: a 4-node graph where 3 form a cycle and 1 is a
/// valid leaf — only the 3 cycle nodes appear in CycleDetected.
///
/// The registry contains "NodeX" with matching input/output slots.
/// Nodes "a", "b", "c" form a cycle (a→b→c→a). Node "d" is a valid
/// leaf with no incoming or outgoing edges. validate_graph must
/// detect only {a, b, c} as cycle nodes — "d" must NOT be listed.
#[test]
fn test_validate_graph_partial_cycle_in_larger_graph() {
    let registry = NodeTypeRegistry::new();
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "NodeX".into(),
        display_name: "Node X".into(),
        category: "test".into(),
        description: "Node X".into(),
        inputs: vec![SlotDescriptor {
            name: "IN".into(),
            slot_type: SlotType::Model,
            optional: false,
        }],
        outputs: vec![SlotDescriptor {
            name: "OUT".into(),
            slot_type: SlotType::Model,
            optional: false,
        }],
    }]);

    let graph = serde_json::json!({
        "nodes": [
            {"id": "a", "type": "NodeX"},
            {"id": "b", "type": "NodeX"},
            {"id": "c", "type": "NodeX"},
            {"id": "d", "type": "NodeX"}
        ],
        "edges": [
            {"from": "a:OUT", "to": "b:IN"},
            {"from": "b:OUT", "to": "c:IN"},
            {"from": "c:OUT", "to": "a:IN"}
        ]
    });
    let result = validate_graph(graph, &registry);

    assert!(result.is_err(), "Expected Err for partial cycle, got Ok");
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1, "Expected exactly one error");
    match &errors[0] {
        GraphError::CycleDetected(nodes) => {
            assert_eq!(
                nodes.len(),
                3,
                "Expected exactly 3 nodes in cycle, got: {:?}",
                nodes
            );
            assert!(nodes.contains(&"a".to_string()), "Cycle must include 'a'");
            assert!(nodes.contains(&"b".to_string()), "Cycle must include 'b'");
            assert!(nodes.contains(&"c".to_string()), "Cycle must include 'c'");
            assert!(
                !nodes.contains(&"d".to_string()),
                "Non-cycle node 'd' must NOT be in CycleDetected, got: {:?}",
                nodes
            );
        }
        other => panic!("Expected CycleDetected, got: {:?}", other),
    }
}
