use std::sync::Arc;

/// AppState holds shared server state accessible to all HTTP handlers.
///
/// Constructed once at server boot and cloned into each handler via axum's
/// `State` extractor. `start_time` is used to compute server uptime;
/// `version` is the crate version from `CARGO_PKG_VERSION`;
/// `env_report` is the stub environment report returned by the `/v1/system/env`
/// endpoint (populated by future tasks); `hardware` is the hardware snapshot
/// populated by `detect_all_devices()` at startup (Phase 004).
#[derive(Clone)]
pub struct AppState {
    /// Instant at which this server instance was created.
    pub start_time: std::time::Instant,

    /// The server version string, typically `CARGO_PKG_VERSION`.
    pub version: String,

    /// Stub environment report. Populated by future tasks that probe the
    /// Python worker environment at startup.
    pub env_report: anvilml_core::types::EnvReport,

    /// Hardware snapshot populated by `detect_all_devices()` at startup.
    /// Shared via `Arc<RwLock<>>` so it can be updated independently of
    /// request handling without holding a lock across an await point.
    pub hardware: Arc<tokio::sync::RwLock<anvilml_core::types::HardwareInfo>>,
}

impl AppState {
    /// Create a new AppState with the given server version.
    ///
    /// The `start_time` is set to the current instant. The `version` is stored
    /// by converting the argument into an owned `String` via `Into<String>`,
    /// which accepts `String`, `&str`, and `&'static str`. The `env_report`
    /// field is initialized with default values (a stub for future population).
    /// The `hardware` field is initialized with a default (empty) `HardwareInfo`.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            version: version.into(),
            env_report: anvilml_core::types::EnvReport::default(),
            hardware: Arc::new(tokio::sync::RwLock::new(
                anvilml_core::types::HardwareInfo::default(),
            )),
        }
    }

    /// Create a new AppState with hardware detection results.
    ///
    /// This constructor is used at server startup after `detect_all_devices()`
    /// has populated the hardware snapshot. The version and hardware data are
    /// stored directly; `env_report` is initialised with default values.
    ///
    /// # Arguments
    ///
    /// * `version` — The server version string (e.g. from `CARGO_PKG_VERSION`).
    /// * `hardware` — A pre-detect `Arc<RwLock<HardwareInfo>>` containing the
    ///   hardware snapshot from `detect_all_devices()`.
    pub fn new_with_hardware(
        version: impl Into<String>,
        hardware: Arc<tokio::sync::RwLock<anvilml_core::types::HardwareInfo>>,
    ) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            version: version.into(),
            env_report: anvilml_core::types::EnvReport::default(),
            hardware,
        }
    }
}
