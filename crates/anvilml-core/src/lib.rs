//! Domain types, configuration schema, error types, and the node type
//! registry for AnvilML.
//!
//! This crate owns all data structures that flow through the system:
//! job definitions, model metadata, hardware info, worker state,
//! node type descriptors, WebSocket event types, and the node type
//! registry that tracks which types each connected worker supports.
//!
//! **Hard constraints:** Zero I/O. Zero network. This crate is pure data
//! — serialisable, clonable, and testable without any external
//! dependencies — with one explicit, narrow exception: `node_registry`
//! (`NodeTypeRegistry`) is stateful and async, because it must sit below
//! both `anvilml-worker` and `anvilml-scheduler` in the dependency graph
//! to avoid a cycle between them. See `node_registry`'s module doc for
//! why. No other module in this crate may follow that example — new
//! async or I/O-bearing types belong in a higher-level crate, not here.

pub mod config;
pub mod config_load;
pub mod error;
pub mod node_registry;
pub mod types;

pub use config::{
    GpuSelectionConfig, HardwareOverrideConfig, LimitsConfig, ModelDirConfig, RocmConfig,
    ServerConfig,
};
pub use config_load::{load, ConfigOverrides};
pub use error::AnvilError;
pub use node_registry::NodeTypeRegistry;
pub use types::{
    ArtifactMeta, CapabilitySource, DeviceType, EnumerationSource, EnvReport, GpuDevice,
    HardwareInfo, HostInfo, InferenceCaps, Job, JobSettings, JobStatus, ModelDtype, ModelFormat,
    ModelKind, ModelMeta, NodeTypeDescriptor, ProvisioningState, SlotDescriptor, SlotType,
    SubmitJobRequest, SubmitJobResponse, WorkerInfo, WorkerStatus, WsEvent,
};
