pub mod config;
pub mod config_load;
pub mod error;
pub mod types;

pub use config::*;
pub use config_load::{load_config, ConfigError, ConfigOverrides};
pub use error::AnvilError;

// Re-export model and artifact domain types for convenience.
pub use types::artifact::ArtifactMeta;
pub use types::model::{DType, ModelKind, ModelMeta};

// Re-export hardware domain types (§4.3).
pub use types::hardware::{DeviceType, GpuDevice, HardwareInfo, HostInfo, InferenceCaps};

// Re-export worker domain types (§4.4, §6.1).
pub use types::worker::{EnvReport, WorkerInfo, WorkerStatus};

// Re-export WebSocket event types (§4.5).
pub use types::events::{
    GpuStatSnapshot, JobCancelledEvent, JobCompletedEvent, JobFailedEvent, JobImageReadyEvent,
    JobProgressEvent, JobQueuedEvent, JobStartedEvent, SystemStatsEvent, WorkerStatusChangedEvent,
    WsEvent,
};

pub fn stub() {}
