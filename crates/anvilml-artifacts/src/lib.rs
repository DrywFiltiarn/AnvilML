//! Content-addressed PNG artifact storage.
//!
//! Provides `ArtifactStore`, the persistence layer for generated PNG artifacts.
//! Artifacts are stored by content hash (SHA-256) in a configurable directory,
//! and their metadata is persisted in an SQLite database.

pub mod store;

pub use store::ArtifactStore;
