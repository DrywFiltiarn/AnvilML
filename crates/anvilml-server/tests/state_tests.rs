//! Integration tests for the `AppState` struct.
//!
//! Tests verify construction, cloning, and `Arc`-sharing semantics
//! of the shared application state used by the AnvilML HTTP server.

use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, ServerConfig};
use anvilml_server::AppState;
use std::sync::Arc;

/// Verify that `AppState` constructs with a default `ServerConfig`
/// and an empty `NodeTypeRegistry`.
///
/// Constructs `AppState` using `ServerConfig::default()` and
/// `NodeTypeRegistry::new()`, both wrapped in `Arc::new()`. Asserts
/// that the `config` and `node_registry` fields are accessible and
/// that the registry reports `is_empty() == true`.
#[test]
fn test_app_state_constructs() {
    let state = AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry: Arc::new(NodeTypeRegistry::new()),
    };

    // Verify both fields are accessible and the registry starts empty.
    assert!(!state.config.host.is_empty());
    assert!(state.node_registry.is_empty());
}

/// Verify that cloning `AppState` shares the underlying
/// `Arc<NodeTypeRegistry>` — mutations visible through one clone
/// are observable through the other.
///
/// Constructs `AppState`, clones it to `cloned`, registers a single
/// `NodeTypeDescriptor` via `state.node_registry.register_all()`, then
/// reads back via `cloned.node_registry.list()` and asserts the
/// descriptor is present. This proves both clones share the same
/// `Arc<NodeTypeRegistry>` heap allocation.
#[test]
fn test_app_state_clone_shares_node_registry() {
    let state = AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry: Arc::new(NodeTypeRegistry::new()),
    };

    // Clone before mutation — both clones share the same Arc.
    let cloned = state.clone();

    // Register a synthetic node descriptor via the original clone.
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    state.node_registry.register_all(vec![descriptor.clone()]);

    // Read back through the clone — the registered descriptor must be present.
    let list = cloned.node_registry.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].type_name, "TestNode");
}
