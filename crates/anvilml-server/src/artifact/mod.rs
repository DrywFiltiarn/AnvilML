//! Artifact persistence module for AnvilML.
//!
//! Provides `ArtifactStore`, a content-addressed storage backend for PNG
//! artifacts produced by job execution. Artifacts are stored by their
//! SHA-256 hash, ensuring idempotent saves — the same image bytes are
//! never written twice.

pub mod store;
pub use store::ArtifactStore;
