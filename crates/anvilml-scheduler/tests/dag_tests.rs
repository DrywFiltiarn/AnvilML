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
