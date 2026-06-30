//! axum HTTP/WS server, all handlers.

pub mod handlers;

/// Build the application router with all registered HTTP routes.
///
/// Returns an `axum::Router` with every handler from the `handlers`
/// module wired to its route path. Callers pass the resulting router
/// to `axum::serve()` to start the HTTP server.
///
/// The `start_time` argument is captured at process startup and used by
/// the `/health` handler to compute elapsed uptime in seconds.
pub fn build_router(start_time: std::time::Instant) -> axum::Router {
    // Wrap the start instant in `HealthState` and set it as the router's
    // state type via `with_state()`, making it available to handlers
    // through `axum::extract::State<HealthState>`.
    let state = handlers::health::HealthState { start_time };
    axum::Router::new()
        .route("/health", axum::routing::get(handlers::health::health))
        .with_state(state)
}
