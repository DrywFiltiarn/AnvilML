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
