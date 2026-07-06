use std::sync::Arc;

use anvilml_core::{NodeTypeRegistry, ServerConfig};

/// Shared application state for the AnvilML HTTP server.
///
/// `AppState` holds all mutable and read-only data that server handlers
/// need access to. Every field is wrapped in `Arc` so that it can be
/// cloned cheaply into multiple handler clones without sharing `&mut`
/// references.
///
/// This struct grows incrementally across tasks — each task adds one
/// or more fields and the corresponding integration tests. Only the
/// fields present at compile time are available; future fields are
/// added by later phase tasks.
#[derive(Clone)]
pub struct AppState {
    /// Server configuration loaded from `anvilml.toml` and env vars.
    pub config: Arc<ServerConfig>,

    /// Dynamic registry of Python-worker node types, populated at
    /// worker Ready time.
    pub node_registry: Arc<NodeTypeRegistry>,

    /// Monotonic clock instant captured at process startup.
    /// Used by the `/health` handler to compute elapsed uptime.
    pub start_time: std::time::Instant,
}
