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
pub use handlers::system::get_env;
pub use state::AppState;

use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Build the HTTP router with all registered handlers.
///
/// Creates a new `Router`, mounts the health handler at `GET /health`,
/// applies the shared `AppState` for injection into handlers, and wraps
/// the router with middleware per ANVILML_DESIGN.md §12.3 (outermost-first):
/// 1. `CorsLayer::permissive()` — allows all origins for local-only use.
/// 2. `TraceLayer` — structured request/response logging via `tracing`.
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
        // System env stub — returns default EnvReport until future tasks
        // populate it from actual worker probe results.
        .route("/v1/system/env", get(get_env))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}
