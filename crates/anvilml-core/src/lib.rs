//! Domain types, configuration schema, and error types for AnvilML.
//!
//! This crate owns all data structures that flow through the system:
//! job definitions, model metadata, hardware info, worker state,
//! node type descriptors, and WebSocket event types.
//!
//! **Hard constraints:** Zero I/O. Zero async. Zero network.
//! This crate is pure data — serialisable, clonable, and testable
//! without any external dependencies.

pub mod config;
pub mod config_load;
pub mod error;
pub mod types;

pub use config::{
    GpuSelectionConfig, HardwareOverrideConfig, LimitsConfig, ModelDirConfig, RocmConfig,
    ServerConfig,
};
pub use config_load::{load, ConfigOverrides};
pub use error::AnvilError;
pub use types::{
    ArtifactMeta, CapabilitySource, DeviceType, EnumerationSource, GpuDevice, HardwareInfo,
    HostInfo, InferenceCaps, Job, JobSettings, JobStatus, ModelDtype, ModelFormat, ModelKind,
    ModelMeta, SubmitJobRequest, SubmitJobResponse,
};
