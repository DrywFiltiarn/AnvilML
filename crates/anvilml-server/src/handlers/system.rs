use axum::extract::State;
use axum::Json;

use crate::state::AppState;

/// GET /v1/system/env — returns the current environment report.
///
/// Extracts `State<AppState>` from the request and returns the `env_report`
/// field as a JSON response. This is a stub endpoint: the `EnvReport` is
/// populated with default values and will be filled by future tasks that
/// probe the Python worker environment at startup.
pub async fn get_env(State(state): State<AppState>) -> Json<anvilml_core::types::EnvReport> {
    // Return the env_report as-is. Currently a stub with default values;
    // future tasks will populate it from actual worker probe results.
    Json(state.env_report)
}
