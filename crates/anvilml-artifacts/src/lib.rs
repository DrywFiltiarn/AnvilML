//! Content-addressed PNG artifact storage for AnvilML.
//!
//! Stores generated images by SHA-256 hash and records metadata in SQLite.
//! Shared by `anvilml-scheduler` and `anvilml-server`; neither owns the
//! other's copy.

pub mod store;

pub use store::ArtifactStore;
