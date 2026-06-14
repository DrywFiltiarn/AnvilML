//! Domain types for job management, model/asset metadata, and hardware inventory.
//!
//! Contains `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, and `SubmitJobResponse`
//! for job lifecycle management, plus `ModelMeta`, `ModelKind`, `ModelDtype`,
//! `ModelFormat`, and `ArtifactMeta` for model registry and artifact tracking.
//! Also includes `HardwareInfo`, `GpuDevice`, `HostInfo`, `InferenceCaps`, and
//! supporting enums for hardware detection and reporting.

pub mod artifact;
pub mod hardware;
pub mod job;
pub mod model;

pub use artifact::ArtifactMeta;
pub use hardware::{
    CapabilitySource, DeviceType, EnumerationSource, GpuDevice, HardwareInfo, HostInfo,
    InferenceCaps,
};
pub use job::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};
pub use model::{ModelDtype, ModelFormat, ModelKind, ModelMeta};
