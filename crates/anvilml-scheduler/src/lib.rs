//! Job scheduling, VRAM ledger, DAG validation, and dispatch for AnvilML.
//!
//! This crate owns the job queue (FIFO with O(1) cancel), VRAM ledger
//! (per-device reservation tracking), DAG validation using the dynamic
//! node type registry, and the dispatch loop.
//!
//! **Hard constraints:** No knowledge of HTTP request/response types.
//! The scheduler speaks in jobs, graphs, and VRAM — not routes or handlers.

// `NodeTypeRegistry` is defined in `anvilml_core::node_registry`, not here.
// P11-A1 originally created it in this crate; P11-A2 moved it to break a
// dependency cycle (this crate depends on `anvilml-worker`, and
// `anvilml-worker` needs to call `update_from_worker` directly — see
// `anvilml_core::node_registry`'s module doc for the full explanation).
// Re-exported here so existing call sites (e.g.
// `crates/anvilml-scheduler/tests/node_registry_tests.rs`, which imports
// `anvilml_scheduler::NodeTypeRegistry`) keep working unchanged.
pub use anvilml_core::NodeTypeRegistry;

pub mod types;
pub use types::GraphError;

pub mod dag;
pub mod event_loop;
pub mod ledger;
pub mod queue;
pub mod scheduler;

pub use scheduler::JobScheduler;
