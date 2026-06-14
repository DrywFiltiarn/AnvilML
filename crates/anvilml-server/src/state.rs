/// AppState holds shared server state accessible to all HTTP handlers.
///
/// Constructed once at server boot and cloned into each handler via axum's
/// `State` extractor. `start_time` is used to compute server uptime;
/// `version` is the crate version from `CARGO_PKG_VERSION`;
/// `env_report` is the stub environment report returned by the `/v1/system/env`
/// endpoint (populated by future tasks).
#[allow(dead_code)]
// Fields are read by handlers in later tasks (health handler, uptime metrics,
// system env). No handler exists yet for env_report, so the compiler flags
// it as unused.
#[derive(Clone)]
pub struct AppState {
    /// Instant at which this server instance was created.
    pub start_time: std::time::Instant,

    /// The server version string, typically `CARGO_PKG_VERSION`.
    pub version: String,

    /// Stub environment report. Populated by future tasks that probe the
    /// Python worker environment at startup.
    pub env_report: anvilml_core::types::EnvReport,
}

impl AppState {
    /// Create a new AppState with the given server version.
    ///
    /// The `start_time` is set to the current instant. The `version` is stored
    /// by converting the argument into an owned `String` via `Into<String>`,
    /// which accepts `String`, `&str`, and `&'static str`. The `env_report`
    /// field is initialized with default values (a stub for future population).
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            version: version.into(),
            env_report: anvilml_core::types::EnvReport::default(),
        }
    }
}
