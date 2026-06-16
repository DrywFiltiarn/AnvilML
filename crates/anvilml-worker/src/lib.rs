//! Worker pool management for AnvilML.
//!
//! This crate owns spawn, supervise, and respawn of Python worker subprocesses,
//! the IPC bridge (two independent reader/writer tasks), keepalive heartbeat
//! with timeout watchdog, and the respawn policy with backoff.
//!
//! **Hard constraints:** Contain only process management and message routing.
//! No business logic — that belongs in the scheduler.

pub mod bridge;
pub mod env;
pub mod keepalive;
pub mod spawn;
pub use bridge::start;
pub use env::build_worker_env;
pub use keepalive::start as start_keepalive;
pub use spawn::build_command;
