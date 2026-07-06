/// A graph that has passed every validation check.
///
/// This is a construction-gated newtype: the only way to obtain a
/// `ValidatedGraph` from outside this crate is a successful call to
/// `validate_graph()` (implemented in dag.rs, P12-A3). The inner
/// `serde_json::Value` field is `pub(crate)` so code within the
/// crate can inspect the validated graph, but there is no public
/// bypass constructor in a normal (non-test) build — see the
/// `test-util`-gated block below for the one exception, which does
/// not exist outside `cargo test`.
#[derive(Debug, Clone)]
#[allow(dead_code)] // The inner field is read via pub(crate) accessors and by validate_graph()
pub struct ValidatedGraph(pub(crate) serde_json::Value);

impl ValidatedGraph {
    /// Construct a `ValidatedGraph` from a `serde_json::Value`, bypassing
    /// `validate_graph()`.
    ///
    /// Gated behind the `test-util` feature, which is enabled **only** via
    /// this crate's own `[dev-dependencies]` self-reference (see
    /// `Cargo.toml`) — that feature is never active in a normal build, a
    /// release build, or as a transitive dependency of any other crate.
    /// This is what makes the construction-gate invariant real: outside
    /// `cargo test`, no code anywhere, in this crate or any dependent, can
    /// construct a `ValidatedGraph` except via a successful `validate_graph()`
    /// call. Integration tests under `tests/` need this because they compile
    /// as a separate crate and cannot see `pub(crate)` items or plain
    /// `#[cfg(test)]` items from the library.
    #[cfg(feature = "test-util")]
    pub fn _test_new(value: serde_json::Value) -> Self {
        Self(value)
    }

    /// Access the inner value. Same `test-util` gating and rationale as
    /// `_test_new` above.
    #[cfg(feature = "test-util")]
    pub fn _test_inner(&self) -> &serde_json::Value {
        &self.0
    }
}

/// Errors produced by graph validation.
///
/// Each variant corresponds to one of the six validation checks defined
/// in ANVILML_DESIGN.md §12.3, plus the structural root check (check 1).
/// All variants derive `Debug`, `Clone`, and `thiserror::Error`.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GraphError {
    /// The root JSON value is not an object (e.g. it is an array, string, or null).
    #[error("root is not an object")]
    NotAnObject,

    /// The root object does not contain a `"nodes"` key.
    #[error(r#"missing "nodes" array"#)]
    MissingNodesArray,

    /// A node `id` value appeared more than once in the nodes array.
    #[error("duplicate node id: {0}")]
    DuplicateNodeId(String),

    /// A node referenced a `type` that is not registered in the node type registry.
    #[error(r#"unknown node type "{type_name}" for node {node_id}"#)]
    UnknownNodeType { node_id: String, type_name: String },

    /// An edge references an output slot that the source node does not declare.
    #[error(r#"dangling edge: node {node_id} missing output slot "{slot_name}""#)]
    DanglingEdge { node_id: String, slot_name: String },

    /// An edge's output slot type is incompatible with the receiving input slot type.
    #[error(r#"slot type mismatch on node {node_id} slot "{slot_name}": expected {expected}, found {found}"#)]
    SlotTypeMismatch {
        node_id: String,
        slot_name: String,
        expected: String,
        found: String,
    },

    /// The graph contains a cycle; the Vec lists every node participating in the cycle.
    #[error("cycle detected involving nodes: {0:?}")]
    CycleDetected(Vec<String>),
}
