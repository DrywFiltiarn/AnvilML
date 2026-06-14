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

pub use config::{
    GpuSelectionConfig, HardwareOverrideConfig, LimitsConfig, ModelDirConfig, RocmConfig,
    ServerConfig,
};
