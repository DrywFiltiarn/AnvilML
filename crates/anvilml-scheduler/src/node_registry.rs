//! Thread-safe registry of node types, populated from worker `Ready` events.
//!
//! Stores a mapping from node type name (`String`) to `NodeTypeDescriptor`.
//! Updated via `update_from_worker` when a worker reports its capabilities.
//! Existing entries not present in the new list are preserved (merge semantics),
//! because different workers may register different subsets of node types.

use std::sync::Arc;

use anvilml_core::NodeTypeDescriptor;
use hashbrown::HashMap;
use tokio::sync::RwLock;

/// Thread-safe registry of node types, populated from worker `Ready` events.
///
/// Stores a mapping from node type name (`String`) to `NodeTypeDescriptor`.
/// Updated via `update_from_worker` when a worker reports its capabilities.
/// Existing entries not present in the new list are preserved (merge semantics),
/// because different workers may register different subsets of node types.
#[derive(Debug, Default)]
pub struct NodeTypeRegistry {
    types: Arc<RwLock<HashMap<String, NodeTypeDescriptor>>>,
}

impl NodeTypeRegistry {
    /// Create a new empty `NodeTypeRegistry`.
    ///
    /// Returns a registry with no registered node types. Use `update_from_worker`
    /// to populate the registry from worker `Ready` events.
    pub async fn new() -> Self {
        Self::default()
    }

    /// Update the registry with node types reported by a worker.
    ///
    /// Inserts or updates each descriptor keyed by `type_name`. Existing entries
    /// not present in the new list are preserved (merge semantics), because
    /// different workers may register different subsets of node types.
    ///
    /// # Arguments
    ///
    /// * `worker_id` — Identifier of the worker sending this update. Used for
    ///   structured logging.
    /// * `types` — List of `NodeTypeDescriptor` values reported by the worker.
    pub async fn update_from_worker(&self, worker_id: &str, types: Vec<NodeTypeDescriptor>) {
        // Merge semantics: different workers may register different subsets of
        // node types. Inserting into the existing map preserves entries from
        // prior workers while adding/updating this worker's types.
        let mut map = self.types.write().await;
        for desc in &types {
            map.insert(desc.type_name.clone(), desc.clone());
        }
        tracing::debug!(worker_id = %worker_id, node_count = types.len(), "node registry updated");
    }

    /// Look up a node type descriptor by its type name.
    ///
    /// Returns `None` if no descriptor with the given `type_name` is registered.
    ///
    /// # Arguments
    ///
    /// * `type_name` — The unique type identifier to look up (e.g. `"KSampler"`).
    pub async fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor> {
        self.types.read().await.get(type_name).cloned()
    }

    /// Return all registered node type descriptors.
    ///
    /// The order of entries is not guaranteed (hash map iteration order).
    pub async fn all_types(&self) -> Vec<NodeTypeDescriptor> {
        self.types.read().await.values().cloned().collect()
    }

    /// Return `true` if no node types are registered.
    pub async fn is_empty(&self) -> bool {
        self.types.read().await.is_empty()
    }
}
