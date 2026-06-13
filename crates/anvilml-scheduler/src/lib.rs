//! Job scheduling, VRAM ledger, DAG validation, and dispatch for AnvilML.
//!
//! This crate owns the job queue (FIFO with O(1) cancel), VRAM ledger
//! (per-device reservation tracking), DAG validation using the dynamic
//! node type registry, and the dispatch loop.
//!
//! **Hard constraints:** No knowledge of HTTP request/response types.
//! The scheduler speaks in jobs, graphs, and VRAM — not routes or handlers.

#[allow(dead_code)]
pub fn stub() {}
