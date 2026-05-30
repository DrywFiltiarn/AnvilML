//! Core domain types for AnvilML.
//!
//! This module defines the data contract between all AnvilML crates:
//! - **Job** lifecycle types (`job`): `Job`, `JobStatus`, `JobSettings`,
//!   `SubmitJobRequest`, `SubmitJobResponse`
//! - **Model** metadata (`model`): `ModelMeta`, `ModelKind`, `DType`
//! - **Artifact** metadata (`artifact`): `ArtifactMeta`
//! - **Hardware** detection output (`hardware`): `HardwareInfo`, `GpuDevice`,
//!   `DeviceType`, `HostInfo`, `InferenceCaps`
//! - **Worker** state (`worker`): `WorkerInfo`, `WorkerStatus`
//! - **WebSocket events** (`events`): `WsEvent` enum with 9 event structs
//!   and `GpuStatSnapshot`
//!
//! All types are pure serializable structs/enums with zero I/O or async.

pub mod artifact;
pub mod events;
pub mod hardware;
pub mod job;
pub mod model;
pub mod worker;

// ---------------------------------------------------------------------------
// Public re-exports — the public API of this module
// ---------------------------------------------------------------------------

// Job types
pub use job::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};

// Model types
pub use model::{DType, ModelKind, ModelMeta};

// Artifact types
pub use artifact::ArtifactMeta;

// Hardware types
pub use hardware::{DeviceType, GpuDevice, HardwareInfo, HostInfo, InferenceCaps};

// Worker types
pub use worker::{WorkerInfo, WorkerStatus};

// WebSocket event types
pub use events::{
    GpuStatSnapshot, JobCancelledEvent, JobCompletedEvent, JobFailedEvent, JobImageReadyEvent,
    JobProgressEvent, JobQueuedEvent, JobStartedEvent, SystemStatsEvent, WorkerStatusChangedEvent,
    WsEvent,
};
