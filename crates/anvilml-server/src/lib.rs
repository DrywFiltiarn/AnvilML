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
        // GET /v1/jobs — list jobs (with optional status/limit filters)
        // POST /v1/jobs — submit a new job
        // The GET route must be registered before the /v1/jobs/{id} route
        // so axum matches the literal path `/v1/jobs` before the parameterised
        // path `/v1/jobs/{id}`.
        .route(
            "/v1/jobs",
            axum::routing::get(handlers::jobs::list_jobs).post(handlers::jobs::submit_job),
        )
        // GET /v1/jobs/{id} — look up a single job by UUID
        // Axum 0.8+ uses `{capture}` syntax instead of `:capture` for path params.
        .route("/v1/jobs/{id}", axum::routing::get(handlers::jobs::get_job))
        .route("/v1/nodes", axum::routing::get(handlers::nodes::list_nodes))
        .with_state(app_state)
}
