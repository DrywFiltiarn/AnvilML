//! axum HTTP/WS server, all handlers.

pub mod handlers;
pub mod state;

pub use state::AppState;

/// Build the application router with all registered HTTP routes.
///
/// Returns an `axum::Router` with every handler from the `handlers`
/// module wired to its route path. Callers pass the resulting router
/// to `axum::serve()` to start the HTTP server.
///
/// The `app_state` argument is a pre-constructed `AppState` that holds
/// the server configuration, node registry, and process-start instant.
/// The router uses `.with_state()` to inject this state into handlers
/// via `axum::extract::State<AppState>`.
pub fn build_router(app_state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(handlers::health::health))
        .route("/v1/nodes", axum::routing::get(handlers::nodes::list_nodes))
        .with_state(app_state)
}
