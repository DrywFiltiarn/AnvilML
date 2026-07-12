//! axum HTTP/WS server, all handlers.

pub mod handlers;
pub mod state;
pub mod ws;

pub use state::AppState;

/// Build the application router with all registered HTTP routes.
///
/// Returns an `axum::Router` with every handler from the `handlers`
/// module wired to its route path, plus a permissive CORS layer so
/// that browser-based clients (e.g. AnvilML-TestUI, BloomeryUI) served
/// from a different origin/port than the API can call it directly.
/// Callers pass the resulting router to `axum::serve()` to start the
/// HTTP server.
///
/// The `app_state` argument is a pre-constructed `AppState` that holds
/// the server configuration, node registry, and process-start instant.
/// The router uses `.with_state()` to inject this state into handlers
/// via `axum::extract::State<AppState>`.
///
/// # CORS
///
/// Per `ANVILML_DESIGN.md §13.3`, this is local-only use: `CorsLayer::permissive()`
/// allows any origin, method, and header, and requires no `AppState` — it is
/// registered as middleware before `.with_state()` is applied. `tower-http`'s
/// `CorsLayer` intercepts CORS preflight (`OPTIONS`) requests directly, so
/// every route above gets preflight handling for free without a registered
/// `OPTIONS` handler.
pub fn build_router(app_state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(handlers::health::health))
        // GET /v1/system — hardware snapshot
        .route(
            "/v1/system",
            axum::routing::get(handlers::system::get_system),
        )
        // GET /v1/system/env — Python environment report
        .route(
            "/v1/system/env",
            axum::routing::get(handlers::system::get_system_env),
        )
        // GET /v1/system/versions — per-component version report
        .route(
            "/v1/system/versions",
            axum::routing::get(handlers::system::get_system_versions),
        )
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
        // POST /v1/jobs/{id} — cancel a job by UUID
        // Axum 0.8+ uses `{capture}` syntax instead of `:capture` for path params.
        .route(
            "/v1/jobs/{id}",
            axum::routing::get(handlers::jobs::get_job).post(handlers::jobs::cancel_job),
        )
        .route("/v1/nodes", axum::routing::get(handlers::nodes::list_nodes))
        // GET /v1/artifacts — list artifact metadata (with optional job_id filter)
        .route(
            "/v1/artifacts",
            axum::routing::get(handlers::artifacts::list_artifacts),
        )
        // GET /v1/artifacts/{hash} — serve raw PNG bytes for a content-addressed artifact
        .route(
            "/v1/artifacts/{hash}",
            axum::routing::get(handlers::artifacts::get_artifact),
        )
        // GET /v1/models — list all models, optionally filtered by kind
        .route(
            "/v1/models",
            axum::routing::get(handlers::models::list_models),
        )
        // GET /v1/models/{id} — look up a single model by its ID
        .route(
            "/v1/models/{id}",
            axum::routing::get(handlers::models::get_model),
        )
        // POST /v1/models/rescan — trigger a background model-directory rescan
        .route(
            "/v1/models/rescan",
            axum::routing::post(handlers::models::rescan_models),
        )
        // GET /v1/workers — list all workers and their current lifecycle states
        // Per ANVILML_DESIGN.md §13.4: GET /v1/workers → 200 Vec<WorkerInfo>
        .route(
            "/v1/workers",
            axum::routing::get(handlers::workers::list_workers),
        )
        // POST /v1/workers/{id}/restart — graceful shutdown + explicit
        // respawn into the same slot (P18-D3).
        .route(
            "/v1/workers/{id}/restart",
            axum::routing::post(handlers::workers::restart_worker),
        )
        // GET /v1/events — WebSocket upgrade for the live event stream.
        // Per ANVILML_DESIGN.md §13.6: subscribe, send the initial
        // SystemStats frame, then (from P16-C2 on) forward subsequent
        // WsEvents until a Lagged consumer is disconnected.
        .route("/v1/events", axum::routing::get(ws::ws_handler))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(app_state)
}
