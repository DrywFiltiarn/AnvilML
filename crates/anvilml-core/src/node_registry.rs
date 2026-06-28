use std::collections::HashMap;
use std::sync::RwLock;

use crate::types::node::NodeTypeDescriptor;

/// A thread-safe, dynamic registry of node type descriptors.
///
/// The registry stores `NodeTypeDescriptor` values keyed by their `type_name` field.
/// It uses interior mutability (`RwLock`) so that it can be shared behind an `Arc`
/// without requiring a `&mut self` receiver — matching the eventual consumer pattern
/// in `anvilml-scheduler` where the scheduler holds `Arc<NodeTypeRegistry>`.
///
/// All public methods take `&self` (shared reference). The `RwLock` provides the
/// necessary mutability internally: `register_all` acquires a write lock to replace
/// the entire map, while `get`, `list`, and `len` acquire a read lock for concurrent
/// access.
pub struct NodeTypeRegistry {
    /// The type name → descriptor mapping, protected by a read-write lock.
    types: RwLock<HashMap<String, NodeTypeDescriptor>>,
}

impl NodeTypeRegistry {
    /// Create a new, empty `NodeTypeRegistry`.
    ///
    /// Returns a registry with zero registered node types. Call `register_all` to
    /// populate it before use.
    pub fn new() -> Self {
        Self {
            types: RwLock::new(HashMap::new()),
        }
    }

    /// Register a batch of node type descriptors, replacing any prior contents.
    ///
    /// This method takes `&self` (shared reference) rather than `&mut self` because
    /// the `RwLock` provides interior mutability. The entire prior map is discarded
    /// and replaced with the new set — this does **not** merge with existing entries.
    ///
    /// # Panics
    ///
    /// Panics if another thread holds the write lock when this is called (lock poisoned).
    /// A poisoned lock indicates a thread panicked while holding the lock, which is a
    /// logic bug — panicking is the correct failure mode for such a bug in a pure-data
    /// crate.
    pub fn register_all(&self, descs: Vec<NodeTypeDescriptor>) {
        // Replace the entire map: prior contents are discarded, not merged.
        // This is intentional — the registry is a snapshot of the current node set
        // as reported by the Python worker at Ready time.
        let mut map = self.types.write().unwrap();
        *map = descs
            .into_iter()
            .map(|d| (d.type_name.clone(), d))
            .collect();
    }

    /// Look up a node type descriptor by its unique type name.
    ///
    /// Returns `Some(descriptor)` if the type name is registered, or `None` if it
    /// is not found or the registry is empty.
    pub fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor> {
        // Acquire a read lock — multiple readers can coexist, only blocked by
        // an active writer from `register_all`.
        let map = self.types.read().unwrap();
        map.get(type_name).cloned()
    }

    /// Return a vector of all registered node type descriptors.
    ///
    /// The returned vector contains one `NodeTypeDescriptor` per registered type name.
    /// The order is non-deterministic (depends on `HashMap` iteration order).
    pub fn list(&self) -> Vec<NodeTypeDescriptor> {
        let map = self.types.read().unwrap();
        map.values().cloned().collect()
    }

    /// Return the number of registered node type descriptors.
    ///
    /// Returns `0` if the registry is empty or has not yet been populated.
    pub fn len(&self) -> usize {
        let map = self.types.read().unwrap();
        map.len()
    }

    /// Return `true` if the registry has no registered node type descriptors.
    ///
    /// This is the complement of `len() > 0`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for NodeTypeRegistry {
    /// Create a new, empty `NodeTypeRegistry`.
    ///
    /// Equivalent to `NodeTypeRegistry::new()`.
    fn default() -> Self {
        Self::new()
    }
}
