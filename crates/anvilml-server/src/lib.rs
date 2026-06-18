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
pub mod ws;
pub use handlers::health::health;
pub use handlers::system::get_env;
pub use handlers::system::get_system;
pub use state::AppState;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use handlers::models::{get_model, list_models, rescan_models};
use handlers::workers::{list_workers, restart_worker};
use ws::handler::ws_events;

/// Build the HTTP router with all registered handlers.
///
/// Creates a new `Router`, mounts the health handler at `GET /health`,
/// the system hardware info handler at `GET /v1/system`, the system env
/// stub at `GET /v1/system/env`, the model list handler at `GET /v1/models`,
/// the model detail handler at `GET /v1/models/:id`, the model rescan handler
/// at `POST /v1/models/rescan`, the worker list handler at `GET /v1/workers`,
/// the worker restart handler at `POST /v1/workers/{id}/restart`, and the
/// WebSocket event stream at `GET /v1/events`, applies the shared `AppState`
/// for injection into handlers, and wraps the router with middleware per
/// ANVILML_DESIGN.md §12.3 (outermost-first):
/// 1. `CorsLayer::permissive()` — allows all origins for local-only use.
/// 2. `TraceLayer` — structured request/response logging via `tracing`.
///
/// # Arguments
///
/// * `state` — The shared application state (version, start time, hardware,
///   model registry).
///
/// # Returns
///
/// An `axum::Router` ready to be passed to `axum::serve`.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        // Health handler is re-exported at the crate root, so reference it
        // directly rather than via the external crate name.
        .route("/health", get(health))
        // System hardware info — returns the full HardwareInfo snapshot
        // populated by detect_all_devices() at server startup.
        .route("/v1/system", get(get_system))
        // System env stub — returns default EnvReport until future tasks
        // populate it from actual worker probe results.
        .route("/v1/system/env", get(get_env))
        // Model list — returns all models, optionally filtered by kind.
        .route("/v1/models", get(list_models))
        // Model detail — returns a single model by ID, or 404 if not found.
        .route("/v1/models/{id}", get(get_model))
        // Model rescan — triggers a background directory scan.
        // Returns 202 Accepted immediately; the scan runs in a spawned task.
        .route("/v1/models/rescan", post(rescan_models))
        // Worker list — returns all workers and their current states.
        // Returns an empty array when no worker pool is configured.
        .route("/v1/workers", get(list_workers))
        // Worker restart — signals an unconditional force-kill and respawn
        // of the named worker, bypassing RespawnPolicy. Returns 202
        // immediately; the respawn is asynchronous.
        .route("/v1/workers/{id}/restart", post(restart_worker))
        // WebSocket event stream — accepts upgrade requests and forwards
        // broadcast events as JSON text frames to connected clients.
        .route("/v1/events", get(ws_events))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}
