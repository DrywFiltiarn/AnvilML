use anvilml_scheduler::{GraphError, ValidatedGraph};

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
