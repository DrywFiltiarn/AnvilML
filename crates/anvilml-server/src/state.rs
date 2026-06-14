/// AppState holds shared server state accessible to all HTTP handlers.
///
/// Constructed once at server boot and cloned into each handler via axum's
/// `State` extractor. `start_time` is used to compute server uptime;
/// `version` is the crate version from `CARGO_PKG_VERSION`.
#[allow(dead_code)]
// Fields are read by handlers in later tasks (health handler, uptime metrics).
// No handler exists yet, so the compiler flags them as unused.
#[derive(Clone)]
pub struct AppState {
    /// Instant at which this server instance was created.
    pub start_time: std::time::Instant,

    /// The server version string, typically `CARGO_PKG_VERSION`.
    pub version: String,
}

impl AppState {
    /// Create a new AppState with the given server version.
    ///
    /// The `start_time` is set to the current instant. The `version` is stored
    /// by converting the argument into an owned `String` via `Into<String>`,
    /// which accepts `String`, `&str`, and `&'static str`.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            version: version.into(),
        }
    }
}
