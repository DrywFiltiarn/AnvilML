//! Core domain types for AnvilML.
//!
//! This module defines the data contract between all AnvilML crates:
//! - **Job** lifecycle types (`job`): `Job`, `JobStatus`, `JobSettings`,
//!   `SubmitJobRequest`, `SubmitJobResponse`
//! - **Model** metadata (`model`): `ModelMeta`, `ModelKind`, `DType`
//! - **Artifact** metadata (`artifact`): `ArtifactMeta`
//!
//! All types are pure serializable structs/enums with zero I/O or async.

pub mod artifact;
pub mod job;
pub mod model;

// ---------------------------------------------------------------------------
// Public re-exports — the public API of this module
// ---------------------------------------------------------------------------

// Job types
pub use job::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};

// Model types
pub use model::{DType, ModelKind, ModelMeta};

// Artifact types
pub use artifact::ArtifactMeta;
