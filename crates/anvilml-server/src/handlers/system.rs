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

/// GET /v1/system — returns the full hardware information snapshot.
///
/// Reads the hardware snapshot from `AppState` under a read lock, clones
/// the `HardwareInfo` (all fields implement `Clone`), and returns it as
/// a JSON response. The hardware data is populated at server startup by
/// `detect_all_devices()` and stored in `AppState.hardware`.
pub async fn get_system(State(state): State<AppState>) -> Json<anvilml_core::types::HardwareInfo> {
    // Read the hardware snapshot under a read lock and clone it.
    // Cloning is cheap because all fields are Clone — the struct contains
    // primitives, Strings, Vec<GpuDevice>, and Option<String>.
    Json(state.hardware.read().await.clone())
}
