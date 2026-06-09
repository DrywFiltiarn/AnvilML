use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::Json};

use anvilml_core::{EnvReport, HardwareInfo};

/// GET /v1/system/env handler.
///
/// Returns a stubbed `EnvReport` JSON object. The stub values (`python_path=""`,
/// `preflight_ok=false`, `reason="not_checked"`) are placeholders that will be
/// replaced by real preflight logic in Phase 18.
pub async fn get_env(State(state): State<Arc<crate::App>>) -> (StatusCode, Json<EnvReport>) {
    let report = state.env_report();
    (StatusCode::OK, Json(report))
}

/// GET /v1/system handler.
///
/// Returns the hardware detection result collected at server startup.
pub async fn get_system(State(state): State<Arc<crate::App>>) -> (StatusCode, Json<HardwareInfo>) {
    let info = state.hardware();
    (StatusCode::OK, Json(info))
}
