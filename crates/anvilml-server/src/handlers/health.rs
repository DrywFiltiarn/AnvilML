use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::App;

#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Service status — always "ok" when healthy.
    pub status: &'static str,
    /// Application version string.
    pub version: String,
    /// Uptime in seconds since server start.
    pub uptime_s: u64,
}

/// Health-check endpoint handler.
///
/// Returns HTTP 200 with a JSON body containing the application status,
/// version, and uptime in seconds since start.
#[utoipa::path(
    get,
    path = "/health",
    summary = "Health check",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
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
