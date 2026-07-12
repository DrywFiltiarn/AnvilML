//! Liveness-check handler.
//!
//! Returns `200 OK` with a JSON body containing the server status, version,
//! and elapsed uptime — per `ANVILML_DESIGN.md §13.4`.

use axum::Json;
use axum::extract::State;

use crate::AppState;

/// JSON response body for the `/health` liveness probe.
///
/// Per `ANVILML_DESIGN.md §13.4`: `200 { status, version, uptime_s }`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct HealthResponse {
    /// Always `"ok"` for a healthy server.
    status: String,
    /// Compile-time crate version from `CARGO_PKG_VERSION`.
    version: String,
    /// Seconds of uptime, computed as `(Instant::now() - start_time).as_secs()`.
    uptime_s: u64,
}

/// Health-check handler.
///
/// Returns `200 OK` with a JSON body containing the server status, version,
/// and elapsed uptime — per `ANVILML_DESIGN.md §13.4`.
///
/// State is injected via `axum::extract::State<AppState>` which carries the
/// process-start instant for uptime calculation.
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    operation_id = "health_check",
    summary = "Health check",
    description = "Liveness probe returning server status, version, and elapsed uptime.",
    responses(
        (status = 200, description = "Server is healthy", body = HealthResponse)
    )
)]
pub(crate) async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    // Compute elapsed seconds since process start using monotonic clock.
    let uptime_s = (std::time::Instant::now() - state.start_time).as_secs();
    Json(HealthResponse {
        status: "ok".to_string(),
        // `CARGO_PKG_VERSION` is resolved at compile time for this crate.
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_s,
    })
}
