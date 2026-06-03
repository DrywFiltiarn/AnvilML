use std::sync::{Arc, RwLock};

use anvilml_core::{EnvReport, HardwareInfo};
use std::time::Instant;

/// Application state shared across all request handlers.
pub struct AppState {
    /// The time at which the application started.
    start_time: Instant,
    /// The application version string.
    version: String,
    /// Python environment health report (stubbed, updated by preflight).
    env_report: Arc<RwLock<EnvReport>>,
    /// Hardware detection result (populated at startup by `detect_all_devices`).
    hardware: Arc<RwLock<HardwareInfo>>,
}

impl AppState {
    /// Create a new `AppState` with the given version string.
    ///
    /// The hardware field is initialised with an empty `HardwareInfo`.
    /// Use [`Self::new_with_hardware`] for production use where hardware
    /// has been detected at startup.
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
            hardware: Arc::new(RwLock::new(HardwareInfo {
                host: anvilml_core::HostInfo {
                    os: String::new(),
                    cpu_model: String::new(),
                    ram_total_mib: 0,
                    ram_free_mib: 0,
                },
                gpus: Vec::new(),
                inference_caps: anvilml_core::InferenceCaps::default(),
            })),
        }
    }

    /// Create a new `AppState` with the given version string and pre-detected
    /// hardware information.
    pub fn new_with_hardware(version: impl Into<String>, hardware: HardwareInfo) -> Self {
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
            hardware: Arc::new(RwLock::new(hardware)),
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

    /// Returns a clone of the current hardware information.
    pub fn hardware(&self) -> HardwareInfo {
        self.hardware.read().unwrap().clone()
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            version: self.version.clone(),
            env_report: Arc::clone(&self.env_report),
            hardware: Arc::clone(&self.hardware),
        }
    }
}
