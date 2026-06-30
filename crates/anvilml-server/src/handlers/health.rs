use axum::Json;
/// Liveness-check handler.
///
/// Returns `200 OK` with a JSON body containing the server status, version,
/// and elapsed uptime — per `ANVILML_DESIGN.md §13.4`.
///
/// State is injected via `axum::extract::State<HealthState>` which carries the
/// process-start instant for uptime calculation.
use axum::extract::State;

/// Application state carrying the process-start instant for uptime calculation.
#[derive(Clone)]
pub(crate) struct HealthState {
    /// Monotonic clock instant captured at process startup.
    pub(crate) start_time: std::time::Instant,
}

/// JSON response body for the `/health` liveness probe.
///
/// Per `ANVILML_DESIGN.md §13.4`: `200 { status, version, uptime_s }`.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct HealthResponse {
    /// Always `"ok"` for a healthy server.
    status: String,
    /// Compile-time crate version from `CARGO_PKG_VERSION`.
    version: String,
    /// Seconds of uptime, computed as `(Instant::now() - start_time).as_secs()`.
    uptime_s: u64,
}

pub(crate) async fn health(State(state): State<HealthState>) -> Json<HealthResponse> {
    // Compute elapsed seconds since process start using monotonic clock.
    let uptime_s = (std::time::Instant::now() - state.start_time).as_secs();
    Json(HealthResponse {
        status: "ok".to_string(),
        // `CARGO_PKG_VERSION` is resolved at compile time for this crate.
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_s,
    })
}
