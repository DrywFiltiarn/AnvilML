//! System information handlers.
//!
//! Expose the hardware snapshot (`GET /v1/system`), Python environment
//! report (`GET /v1/system/env`), and per-component version report
//! (`GET /v1/system/versions`) over HTTP — per `ANVILML_DESIGN.md §13.4`.
//! All handlers are thin delegations: read-lock the shared `AppState`
//! field, clone the value, return as JSON. No business logic.

use axum::Json;
use axum::extract::State;

use crate::AppState;

/// Per-component version report returned by `GET /v1/system/versions`.
///
/// Fields:
/// - `anvilml_version`: compile-time crate version from `CARGO_PKG_VERSION`.
/// - `rust_version`: runtime `rustc` SemVer from `rustc_version_runtime::version()`.
/// - `python_version`: from `AppState.env_report`, `None` if not yet collected.
/// - `torch_version`: from `AppState.env_report`, `None` if not yet collected.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ComponentVersions {
    /// AnvilML crate version, resolved at compile time.
    pub(crate) anvilml_version: String,
    /// Rust compiler version, resolved at runtime via `rustc_version_runtime`.
    pub(crate) rust_version: String,
    /// Python interpreter version string, or `None` if the worker has not
    /// yet reported it.
    pub(crate) python_version: Option<String>,
    /// PyTorch version string, or `None` if the import failed or torch
    /// is not installed.
    pub(crate) torch_version: Option<String>,
}

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

/// GET /v1/system/versions handler.
///
/// Returns `200 OK` with a `ComponentVersions` struct containing the
/// AnvilML crate version (compile-time), the Rust compiler version
/// (runtime), and the Python/PyTorch versions from the env report
/// — per `ANVILML_DESIGN.md §13.4`.
///
/// Acquires a read lock on `env_report` to extract python_version and
/// torch_version, then constructs the response with the compile-time
/// and runtime version values.
pub(crate) async fn get_system_versions(State(state): State<AppState>) -> Json<ComponentVersions> {
    // Read-lock the environment report to extract python_version and
    // torch_version. The report is cloned so the lock is released
    // before constructing the response.
    let report = state.env_report.read().await.clone();

    Json(ComponentVersions {
        anvilml_version: env!("CARGO_PKG_VERSION").to_string(),
        // `rustc_version_runtime::version()` returns a &'static str
        // containing the SemVer version of the rustc compiler used to
        // build this binary — a compile-time constant exposed at runtime.
        rust_version: rustc_version_runtime::version().to_string(),
        python_version: report.python_version,
        torch_version: report.torch_version,
    })
}
