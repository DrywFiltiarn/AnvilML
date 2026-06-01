use std::sync::{Arc, RwLock};

use anvilml_core::EnvReport;
use std::time::Instant;

/// Application state shared across all request handlers.
pub struct AppState {
    /// The time at which the application started.
    start_time: Instant,
    /// The application version string.
    version: String,
    /// Python environment health report (stubbed, updated by preflight).
    env_report: Arc<RwLock<EnvReport>>,
}

impl AppState {
    /// Create a new `AppState` with the given version string.
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            start_time: Instant::now(),
            version: version.into(),
            env_report: Arc::new(RwLock::new(EnvReport {
                python_path: String::new(),
                python_version: String::new(),
                torch_version: String::new(),
                preflight_ok: false,
                reason: "not_checked".to_string(),
            })),
        }
    }

    /// Seconds since application start.
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Returns the current version string.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns a clone of the current `EnvReport`.
    pub fn env_report(&self) -> EnvReport {
        self.env_report.read().unwrap().clone()
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            version: self.version.clone(),
            env_report: Arc::clone(&self.env_report),
        }
    }
}
