//! Domain types for job management and model/asset metadata.
//!
//! Contains `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, and `SubmitJobResponse`
//! for job lifecycle management, plus `ModelMeta`, `ModelKind`, `ModelDtype`,
//! `ModelFormat`, and `ArtifactMeta` for model registry and artifact tracking.

pub mod artifact;
pub mod job;
pub mod model;

pub use artifact::ArtifactMeta;
pub use job::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};
pub use model::{ModelDtype, ModelFormat, ModelKind, ModelMeta};
