//! Tests for `NodeTypeRegistry` — thread-safe node type registration from
//! worker `Ready` events.

use anvilml_core::{NodeTypeDescriptor, SlotDescriptor, SlotType};
use anvilml_scheduler::NodeTypeRegistry;

/// Verify that `update_from_worker` inserts descriptors into the registry
/// and that `get` returns them by name. Also confirms `all_types` and
/// `is_empty` reflect the updated state.
#[tokio::test]
async fn test_update_populates_registry() {
    let registry = NodeTypeRegistry::new().await;

    let a = NodeTypeDescriptor {
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
    };

    let b = NodeTypeDescriptor {
        type_name: "KSampler".to_string(),
        display_name: "KSampler".to_string(),
        category: "sampling".to_string(),
        description: "Runs a diffusion sampling step".to_string(),
        inputs: vec![],
        outputs: vec![],
    };

    registry
        .update_from_worker("worker-0", vec![a.clone(), b.clone()])
        .await;

    // get returns both by name
    let got_a = registry.get("LoadModel").await;
    assert!(got_a.is_some());
    assert_eq!(got_a.unwrap().type_name, "LoadModel");

    let got_b = registry.get("KSampler").await;
    assert!(got_b.is_some());
    assert_eq!(got_b.unwrap().type_name, "KSampler");

    // all_types returns 2 items
    assert_eq!(registry.all_types().await.len(), 2);

    // is_empty returns false
    assert!(!registry.is_empty().await);
}

/// Verify that `get` returns `None` for a type name that was never registered.
#[tokio::test]
async fn test_get_returns_none_for_unknown_type() {
    let registry = NodeTypeRegistry::new().await;
    assert!(registry.get("NonExistent").await.is_none());
}

/// Verify that `all_types` returns all registered descriptors with correct values.
#[tokio::test]
async fn test_all_types_returns_all_descriptors() {
    let registry = NodeTypeRegistry::new().await;

    let types = vec![
        NodeTypeDescriptor {
            type_name: "A".to_string(),
            display_name: "Node A".to_string(),
            category: "cat".to_string(),
            description: "A".to_string(),
            inputs: vec![],
            outputs: vec![],
        },
        NodeTypeDescriptor {
            type_name: "B".to_string(),
            display_name: "Node B".to_string(),
            category: "cat".to_string(),
            description: "B".to_string(),
            inputs: vec![],
            outputs: vec![],
        },
        NodeTypeDescriptor {
            type_name: "C".to_string(),
            display_name: "Node C".to_string(),
            category: "cat".to_string(),
            description: "C".to_string(),
            inputs: vec![],
            outputs: vec![],
        },
    ];

    registry.update_from_worker("worker-0", types).await;

    let all = registry.all_types().await;
    assert_eq!(all.len(), 3);

    // Each descriptor's type_name must match one of the inputs.
    let type_names: Vec<&str> = all.iter().map(|d| d.type_name.as_str()).collect();
    assert!(type_names.contains(&"A"));
    assert!(type_names.contains(&"B"));
    assert!(type_names.contains(&"C"));
}

/// Verify that `is_empty` is `true` on a default registry and `false`
/// after `update_from_worker` populates it.
#[tokio::test]
async fn test_is_empty_before_and_after_update() {
    let registry = NodeTypeRegistry::new().await;
    assert!(registry.is_empty().await);

    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a model".to_string(),
                inputs: vec![],
                outputs: vec![],
            }],
        )
        .await;

    assert!(!registry.is_empty().await);
}

/// Verify that `update_from_worker` preserves existing entries not present
/// in the new batch (merge semantics). Different workers may register
/// different node type subsets.
#[tokio::test]
async fn test_update_from_worker_merges() {
    let registry = NodeTypeRegistry::new().await;

    // First update: register type A
    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "A".to_string(),
                display_name: "Node A".to_string(),
                category: "cat".to_string(),
                description: "A".to_string(),
                inputs: vec![],
                outputs: vec![],
            }],
        )
        .await;

    // Second update: register type B (no type A)
    registry
        .update_from_worker(
            "worker-1",
            vec![NodeTypeDescriptor {
                type_name: "B".to_string(),
                display_name: "Node B".to_string(),
                category: "cat".to_string(),
                description: "B".to_string(),
                inputs: vec![],
                outputs: vec![],
            }],
        )
        .await;

    // Both A and B should still be present
    assert!(registry.get("A").await.is_some());
    assert!(registry.get("B").await.is_some());
    assert_eq!(registry.all_types().await.len(), 2);
}

/// Verify that `has_been_updated` distinguishes "no worker has ever
/// reached `Ready`" from "a worker reached `Ready` and reported zero node
/// types" — a distinction `is_empty` cannot make on its own, since both
/// cases leave the underlying map empty. This matters for P11-A3's
/// `GET /v1/nodes` handler: it returns 503 only in the former case.
#[tokio::test]
async fn test_has_been_updated_distinguishes_never_updated_from_empty_update() {
    let registry = NodeTypeRegistry::new().await;

    // Before any update: both flags reflect "nothing has happened yet".
    assert!(registry.is_empty().await);
    assert!(!registry.has_been_updated().await);

    // A worker reaches Ready but reports zero node types (the mock-worker
    // case). The map stays empty, but has_been_updated must flip to true.
    registry.update_from_worker("mock-worker", Vec::new()).await;

    assert!(
        registry.is_empty().await,
        "is_empty() should still be true — an empty-vec update inserts \
         nothing into the map"
    );
    assert!(
        registry.has_been_updated().await,
        "has_been_updated() should be true — a Ready event occurred, \
         even though it carried no node types"
    );

    // has_been_updated() never resets to false, even once real types
    // are registered afterward.
    registry
        .update_from_worker(
            "worker-1",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a model".to_string(),
                inputs: vec![],
                outputs: vec![],
            }],
        )
        .await;

    assert!(!registry.is_empty().await);
    assert!(registry.has_been_updated().await);
}
