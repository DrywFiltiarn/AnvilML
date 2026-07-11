//! System information handlers.
//!
//! Expose the hardware snapshot (`GET /v1/system`) and Python environment
//! report (`GET /v1/system/env`) over HTTP — per `ANVILML_DESIGN.md §13.4`.
//! Both handlers are thin delegations: read-lock the shared `AppState` field,
//! clone the value, return as JSON. No business logic.

use axum::Json;
use axum::extract::State;

use crate::AppState;

/// GET /v1/system handler.
///
/// Returns `200 OK` with the current `HardwareInfo` snapshot — per
/// `ANVILML_DESIGN.md §13.4`. Acquires a read lock on the shared
/// `hardware` field, clones the value, and returns it as JSON.
///
/// The clone ensures the response is independent of any concurrent
/// write that may occur after this handler returns.
pub(crate) async fn get_system(State(state): State<AppState>) -> Json<anvilml_core::HardwareInfo> {
    // Read-lock the hardware snapshot and clone the value out.
    // The clone ensures the response is independent of any concurrent
    // write that may occur after this handler returns.
    Json(state.hardware.read().await.clone())
}

/// GET /v1/system/env handler.
///
/// Returns `200 OK` with the current `EnvReport` — per
/// `ANVILML_DESIGN.md §13.4`. Acquires a read lock on the shared
/// `env_report` field, clones the value, and returns it as JSON.
///
/// The clone ensures the response is independent of any concurrent
/// write that may occur after this handler returns.
pub(crate) async fn get_system_env(State(state): State<AppState>) -> Json<anvilml_core::EnvReport> {
    // Read-lock the environment report and clone the value out.
    // Same pattern as get_system — one-line delegation with no
    // business logic, per ANVILML_DESIGN.md §3.3.
    Json(state.env_report.read().await.clone())
}
