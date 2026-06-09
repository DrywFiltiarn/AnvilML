use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::Json};
use serde::Serialize;

use crate::App;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: String,
    pub uptime_s: u64,
}

/// Health-check endpoint handler.
///
/// Returns HTTP 200 with a JSON body containing the application status,
/// version, and uptime in seconds since start.
pub async fn health(State(state): State<Arc<App>>) -> (StatusCode, Json<HealthResponse>) {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            version: state.version().to_string(),
            uptime_s: state.uptime_secs(),
        }),
    )
}
