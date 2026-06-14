//! HTTP and WebSocket server for AnvilML.
//!
//! This crate owns the axum router, all HTTP handlers (health, system,
//! jobs, models, workers, artifacts, nodes), the WebSocket broadcaster,
//! and the artifact store. Handlers call into scheduler, worker, and
//! registry crates only — no business logic lives here.
//!
//! **Hard constraints:** No business logic. All handlers delegate to
//! the scheduler, worker pool, and model registry.

pub mod handlers;
pub mod state;
pub use handlers::health::health;
pub use state::AppState;

use axum::routing::get;
use axum::Router;

/// Build the HTTP router with all registered handlers.
///
/// Creates a new `Router`, mounts the health handler at `GET /health`,
/// and applies the shared `AppState` for injection into handlers.
///
/// # Arguments
///
/// * `state` — The shared application state (version, start time).
///
/// # Returns
///
/// An `axum::Router` ready to be passed to `axum::serve`.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Health handler is re-exported at the crate root, so reference it
        // directly rather than via the external crate name.
        .route("/health", get(health))
        .with_state(state)
}
