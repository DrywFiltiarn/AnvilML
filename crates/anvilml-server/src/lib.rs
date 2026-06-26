//! axum HTTP/WS server, all handlers.

pub mod handlers;

/// Build the application router with all registered HTTP routes.
///
/// Returns an `axum::Router` with every handler from the `handlers`
/// module wired to its route path. Callers pass the resulting router
/// to `axum::serve()` to start the HTTP server.
pub fn build_router() -> axum::Router {
    axum::Router::new().route("/health", axum::routing::get(handlers::health::health))
}
