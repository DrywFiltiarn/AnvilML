use std::time::Instant;

/// Application state shared across all request handlers.
pub struct AppState {
    /// The time at which the application started.
    start_time: Instant,
    /// The application version string.
    version: String,
}

impl AppState {
    /// Create a new `AppState` with the given version string.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            start_time: Instant::now(),
            version: version.into(),
        }
    }

    /// Seconds since application start.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            version: self.version.clone(),
        }
    }
}
