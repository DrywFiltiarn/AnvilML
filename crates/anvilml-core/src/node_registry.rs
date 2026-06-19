//! Thread-safe registry of node types, populated from worker `Ready` events.
//!
//! Stores a mapping from node type name (`String`) to `NodeTypeDescriptor`.
//! Updated via `update_from_worker` when a worker reports its capabilities.
//! Existing entries not present in the new list are preserved (merge semantics),
//! because different workers may register different subsets of node types.
//!
//! # Why this lives in `anvilml-core`, not `anvilml-scheduler`
//!
//! `NodeTypeRegistry` was originally created in `anvilml-scheduler` (P11-A1).
//! P11-A2 needs `anvilml-worker` to call `update_from_worker` directly from
//! `ManagedWorker::run()`'s `Ready` event handler. `anvilml-scheduler`
//! already depends on `anvilml-worker` (for future dispatch — see
//! `ANVILML_DESIGN.md` for the scheduler's eventual role), so adding the
//! reverse edge (`anvilml-worker` → `anvilml-scheduler`) would create a
//! cycle: `anvilml-scheduler → anvilml-worker → anvilml-scheduler`. Moving
//! the registry to `anvilml-core` — a dependency leaf both crates already
//! sit above — breaks the cycle without either crate needing the other.
//! `anvilml-scheduler` re-exports `NodeTypeRegistry` from here for source
//! compatibility with existing call sites.
//!
//! This is a deliberate, explicit exception to this crate's documented
//! "zero I/O, zero async" constraint (see `lib.rs`'s crate-level doc
//! comment, which has been amended to acknowledge it): `NodeTypeRegistry`
//! is stateful (`Arc<RwLock<...>>`) and its methods are `async fn`. No
//! other type in this crate has these properties, and none should — this
//! module exists here solely to sit below both `anvilml-worker` and
//! `anvilml-scheduler` in the dependency graph, not because `anvilml-core`
//! is becoming a general home for async state.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::types::NodeTypeDescriptor;
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

    /// Set once, on the first call to `update_from_worker`, and never
    /// unset. Tracks "has any worker ever reached `Ready`", which is a
    /// distinct question from "are there any entries in `types` right
    /// now" — a mock worker's `Ready` event reports an empty `node_types`
    /// list, so `update_from_worker` can run, insert nothing, and leave
    /// `types` empty. `is_empty()` alone cannot tell that apart from "no
    /// worker has reached Ready yet", but `GET /v1/nodes`'s 503-vs-200
    /// logic (P11-A3) needs exactly that distinction — see
    /// `has_been_updated`'s doc for the precise contract.
    updated: AtomicBool,
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
    /// Marks the registry as updated (see `has_been_updated`) regardless
    /// of whether `types` is empty — a worker reporting zero node types is
    /// still a worker that reached `Ready`, which is the fact
    /// `has_been_updated` exists to record.
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
        // Relaxed is sufficient: this flag is only ever set to true, never
        // read-then-branched-on for synchronization with the map itself —
        // callers needing the map's contents already go through the
        // RwLock above for that. This just records "at least one update
        // happened, ever".
        self.updated.store(true, Ordering::Relaxed);
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

    /// Return `true` if no node types are currently registered.
    ///
    /// This reflects the contents of the underlying map only. It does
    /// **not** distinguish "no worker has ever reached `Ready`" from "a
    /// worker reached `Ready` and reported zero node types" — both leave
    /// this `true`. Callers that need that distinction (e.g. P11-A3's
    /// `GET /v1/nodes` 503-vs-200 logic) should use `has_been_updated`
    /// instead of, or alongside, this method.
    pub async fn is_empty(&self) -> bool {
        self.types.read().await.is_empty()
    }

    /// Return `true` if `update_from_worker` has been called at least
    /// once, regardless of whether the `types` it was called with was
    /// empty.
    ///
    /// This is the method P11-A3's `GET /v1/nodes` handler should check
    /// for its 503-vs-200 decision: 503 means "no worker has ever reached
    /// `Ready`" (`has_been_updated() == false`), not "no node types are
    /// currently registered" (`is_empty() == true`) — a mock worker's
    /// `Ready` event reports an empty `node_types` list, and that is a
    /// real `Ready` event, not the absence of one. Once set, this never
    /// resets to `false` for the lifetime of the registry.
    pub async fn has_been_updated(&self) -> bool {
        self.updated.load(Ordering::Relaxed)
    }
}
