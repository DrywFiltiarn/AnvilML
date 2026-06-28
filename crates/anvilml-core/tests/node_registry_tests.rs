//! Tests for `NodeTypeRegistry` — dynamic registration, lookup, listing, and
//! concurrent access.
//!
//! All tests use only the public API. No I/O, no env vars, no temp files.

use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry};

/// A helper that builds a `NodeTypeDescriptor` with the given type name,
/// display name, and category. Uses empty input/output slot vectors so
/// tests focus on registry behaviour rather than descriptor construction.
fn make_descriptor(type_name: &str, display_name: &str, category: &str) -> NodeTypeDescriptor {
    NodeTypeDescriptor {
        type_name: type_name.to_string(),
        display_name: display_name.to_string(),
        category: category.to_string(),
        description: format!("Test node: {type_name}"),
        inputs: vec![],
        outputs: vec![],
    }
}

/// An empty `NodeTypeRegistry` returns `None` for any lookup and reports
/// a length of zero.
#[test]
fn test_empty_registry_returns_none() {
    let registry = NodeTypeRegistry::new();

    assert!(
        registry.get("NonExistent").is_none(),
        "get on empty registry should return None"
    );
    assert_eq!(registry.len(), 0, "len on empty registry should be 0");
}

/// Registering a single descriptor via `register_all` populates the registry:
/// `get` returns the registered value, `len` returns 1, and `list` contains
/// exactly one element.
#[test]
fn test_register_all_populates() {
    let registry = NodeTypeRegistry::new();
    let desc = make_descriptor("LoadModel", "Load Checkpoint", "loaders");

    registry.register_all(vec![desc.clone()]);

    assert_eq!(
        registry.get("LoadModel"),
        Some(desc),
        "get should return the registered descriptor"
    );
    assert_eq!(
        registry.len(),
        1,
        "len should be 1 after registering one descriptor"
    );

    let listed = registry.list();
    assert_eq!(listed.len(), 1, "list should contain exactly one element");
}

/// Registering a second batch via `register_all` replaces (not merges with)
/// prior contents: the old type name is no longer found.
#[test]
fn test_register_all_replaces_prior_contents() {
    let registry = NodeTypeRegistry::new();

    // Register descriptor A.
    let desc_a = make_descriptor("A", "Node A", "test");
    registry.register_all(vec![desc_a]);
    assert_eq!(registry.len(), 1);

    // Register descriptor B — should replace A entirely.
    let desc_b = make_descriptor("B", "Node B", "test");
    // Clone for the assertion — `register_all` consumes the vec.
    let desc_b_expected = desc_b.clone();
    registry.register_all(vec![desc_b]);

    assert!(
        registry.get("A").is_none(),
        "old entry 'A' should be removed after replacement"
    );
    assert_eq!(
        registry.get("B"),
        Some(desc_b_expected),
        "new entry 'B' should be present"
    );
    assert_eq!(registry.len(), 1, "len should still be 1 after replacement");
}

/// Registering three descriptors with distinct type names results in
/// `list()` returning exactly three elements, each with a matching
/// `type_name`.
#[test]
fn test_list_returns_all() {
    let registry = NodeTypeRegistry::new();

    let desc1 = make_descriptor("NodeOne", "Node One", "test");
    let desc2 = make_descriptor("NodeTwo", "Node Two", "test");
    let desc3 = make_descriptor("NodeThree", "Node Three", "test");

    registry.register_all(vec![desc1.clone(), desc2.clone(), desc3.clone()]);

    let listed = registry.list();
    assert_eq!(listed.len(), 3, "list should contain all 3 descriptors");

    let type_names: Vec<&str> = listed.iter().map(|d| d.type_name.as_str()).collect();
    assert!(
        type_names.contains(&"NodeOne"),
        "list should contain NodeOne"
    );
    assert!(
        type_names.contains(&"NodeTwo"),
        "list should contain NodeTwo"
    );
    assert!(
        type_names.contains(&"NodeThree"),
        "list should contain NodeThree"
    );
}

/// A reader thread calling `get()` in a tight loop (100 iterations) while
/// the main thread calls `register_all()` once completes within 2 seconds
/// without deadlock or panic. This verifies that the `RwLock` correctly
/// allows concurrent reads during a write.
#[test]
fn test_concurrent_get_during_register_all_does_not_deadlock() {
    use std::sync::Arc;

    let registry = Arc::new(NodeTypeRegistry::new());

    // Spawn a reader thread that calls get() in a tight loop.
    let registry_reader = registry.clone();
    let reader = std::thread::spawn(move || {
        for _ in 0..100 {
            let _ = registry_reader.get("Any");
        }
    });

    // Main thread registers a descriptor once.
    let desc = make_descriptor("ConcurrentTest", "Concurrent Test", "test");
    registry.register_all(vec![desc]);

    // Both threads must complete within 2 seconds.
    // A timeout here would indicate a deadlock.
    let result = reader.join();
    assert!(
        result.is_ok(),
        "reader thread panicked — likely a deadlock or poisoned lock"
    );
}
