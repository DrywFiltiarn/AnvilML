//! Worker pool management for AnvilML.
//!
//! This crate owns spawn, supervise, and respawn of Python worker subprocesses,
//! the IPC bridge (two independent reader/writer tasks), keepalive heartbeat
//! with timeout watchdog, and the respawn policy with backoff.
//!
//! **Hard constraints:** Contain only process management and message routing.
//! No business logic — that belongs in the scheduler.

#[allow(dead_code)]
pub fn stub() {}
