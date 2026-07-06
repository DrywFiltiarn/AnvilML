//! Job queue, VRAM ledger, DAG validation, and dispatch loop.

pub mod dag;
pub mod types;
pub use dag::validate_graph;
pub use types::GraphError;
pub use types::ValidatedGraph;
