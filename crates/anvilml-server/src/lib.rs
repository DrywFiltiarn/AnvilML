//! HTTP and WebSocket server for AnvilML.
//!
//! This crate owns the axum router, all HTTP handlers (health, system,
//! jobs, models, workers, artifacts, nodes), the WebSocket broadcaster,
//! and the artifact store. Handlers call into scheduler, worker, and
//! registry crates only — no business logic lives here.
//!
//! **Hard constraints:** No business logic. All handlers delegate to
//! the scheduler, worker pool, and model registry.

#[allow(dead_code)]
pub fn stub() {}
