//! Worker pool management for AnvilML.
//!
//! This crate owns spawn, supervise, and respawn of Python worker subprocesses,
//! the IPC bridge (two independent reader/writer tasks), keepalive heartbeat
//! with timeout watchdog, and the respawn policy with backoff.
//!
//! **Hard constraints:** Contain only process management and message routing.
//! No business logic — that belongs in the scheduler.

pub mod bridge;
pub mod demux;
pub mod env;
#[cfg(windows)]
pub mod job_object;
pub mod keepalive;
pub mod managed;
pub mod pool;
pub mod respawn;
pub mod spawn;
pub use bridge::start;
pub use demux::{
    deregister as deregister_route, register as register_route, start as start_demux, Route,
    RouteTable,
};
pub use env::build_worker_env;
pub use keepalive::start as start_keepalive;
pub use managed::ManagedWorker;
pub use pool::WorkerPool;
pub use respawn::RespawnPolicy;
pub use spawn::build_command;
