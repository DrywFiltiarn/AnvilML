/// A graph that has passed every validation check.
///
/// This is a construction-gated newtype: the only way to obtain a
/// `ValidatedGraph` from outside this crate is a successful call to
/// `validate_graph()` (implemented in dag.rs, P12-A3). The inner
/// `serde_json::Value` field is `pub(crate)` so code within the
/// crate can inspect the validated graph, but there is no public
/// bypass constructor.
#[derive(Debug, Clone)]
#[allow(dead_code)] // The inner field is read via pub(crate) accessors and by validate_graph()
pub struct ValidatedGraph(pub(crate) serde_json::Value);

impl ValidatedGraph {
    /// Construct a ValidatedGraph from a serde_json::Value.
    ///
    /// This method is `pub` only so that integration test crates can
    /// exercise the construction-gated invariant. It is prefixed with
    /// `_test_` to signal it is an internal implementation detail and
    /// must not be used in production code. The only production
    /// constructor is `validate_graph()` in `dag.rs`.
    pub fn _test_new(value: serde_json::Value) -> Self {
        Self(value)
    }

    /// Access the inner value.
    ///
    /// `pub` only so that integration test crates can verify the
    /// `pub(crate)` field is accessible within the crate. Prefixed
    /// with `_test_` to signal it is an internal detail.
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
