use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::state::AppState;

/// GET /health — returns server health status as JSON.
///
/// Extracts `State<AppState>` from the request to read the server version
/// and start time. Computes elapsed uptime in seconds and returns a JSON
/// object with three keys: `status` (always `"ok"`), `version` (the server
/// version string), and `uptime_s` (seconds since server start).
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    // Compute elapsed time since the server was started.
    let uptime_s = (std::time::Instant::now() - state.start_time).as_secs_f64();

    // Build the health response map. Using `serde_json::Value::Object` avoids
    // introducing a dedicated HealthResponse struct, keeping this task minimal.
    let mut map = serde_json::Map::new();
    map.insert("status".into(), "ok".into());
    map.insert("version".into(), state.version.clone().into());
    map.insert("uptime_s".into(), uptime_s.into());

    Json(Value::Object(map))
}
