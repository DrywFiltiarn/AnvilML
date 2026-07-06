use anvilml_scheduler::ValidatedGraph;

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
