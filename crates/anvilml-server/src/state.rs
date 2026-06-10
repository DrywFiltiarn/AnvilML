use std::sync::{Arc, RwLock};

use anvilml_core::config::ServerConfig;
use anvilml_core::types::artifact::ArtifactSave;
use anvilml_core::{config::ModelDirConfig, EnvReport, HardwareInfo};
use anvilml_registry::ModelRegistry;
use anvilml_scheduler::JobScheduler;
use anvilml_worker::WorkerPool;
use sqlx::SqlitePool;
use std::time::Instant;

use crate::ws::broadcaster::EventBroadcaster;

/// Application state shared across all request handlers.
pub struct AppState<A: ArtifactSave + Clone + 'static> {
    /// The time at which the application started.
    start_time: Instant,
    /// The application version string.
    version: String,
    /// Python environment health report (populated by preflight at startup).
    env_report: Arc<RwLock<EnvReport>>,
    /// Hardware detection result (populated at startup by `detect_all_devices`).
    hardware: Arc<RwLock<HardwareInfo>>,
    /// SQLite connection pool for the job/model/artifact registry.
    pub db: Option<SqlitePool>,
    /// Model metadata registry (initialised at startup, scanned in background).
    pub registry: Arc<ModelRegistry>,
    /// Configured model directories for scanning.
    pub model_dirs: Vec<ModelDirConfig>,
    /// WebSocket event broadcaster.
    pub broadcaster: Arc<EventBroadcaster>,
    /// Worker pool — spawned after hardware detection at startup.
    pub workers: Option<Arc<WorkerPool>>,
    /// Job scheduler — orchestrates job submission and dispatch coordination.
    pub scheduler: Option<Arc<JobScheduler<A>>>,
    /// Artifact store for persisting generated images.
    pub artifact_store: A,
    /// Server configuration — used by handlers that need config (e.g. worker restart).
    pub config: ServerConfig,
}

impl<A: ArtifactSave + Clone + 'static> AppState<A> {
    /// Create a new `AppState` with the given version string and optional
    /// SQLite connection pool.
    ///
    /// The hardware field is initialised with an empty `HardwareInfo`.
    /// Use [`Self::new_with_hardware`] for production use where hardware
    /// has been detected at startup.
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        version: impl Into<String>,
        db: Option<SqlitePool>,
        registry: Option<Arc<ModelRegistry>>,
        model_dirs: Option<Vec<ModelDirConfig>>,
        broadcaster: Arc<EventBroadcaster>,
        workers: Option<Arc<WorkerPool>>,
        scheduler: Option<Arc<JobScheduler<A>>>,
        artifact_store: A,
        config: ServerConfig,
    ) -> Self {
        let registry = match (registry, &db) {
            (Some(r), _) => r,
            (None, Some(pool)) => Arc::new(ModelRegistry::new(pool.clone())),
            (None, None) => Arc::new(ModelRegistry::new(
                SqlitePool::connect_lazy("sqlite::memory:")
                    .expect("in-memory SQLite pool must be creatable"),
            )),
        };
        Self {
            start_time: Instant::now(),
            version: version.into(),
            env_report: Arc::new(RwLock::new(EnvReport {
                python_path: String::new(),
                python_version: String::new(),
                torch_version: String::new(),
                preflight_ok: false,
                reason: "unavailable".to_string(),
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
            db,
            registry,
            model_dirs: model_dirs.unwrap_or_default(),
            broadcaster,
            workers,
            scheduler,
            artifact_store,
            config,
        }
    }

    /// Create a new `AppState` with the given version string, pre-detected
    /// hardware information, and optional SQLite connection pool.
    #[expect(clippy::too_many_arguments)]
    pub fn new_with_hardware(
        version: impl Into<String>,
        hardware: HardwareInfo,
        db: Option<SqlitePool>,
        registry: Option<Arc<ModelRegistry>>,
        model_dirs: Option<Vec<ModelDirConfig>>,
        broadcaster: Arc<EventBroadcaster>,
        workers: Option<Arc<WorkerPool>>,
        scheduler: Option<Arc<JobScheduler<A>>>,
        artifact_store: A,
        config: ServerConfig,
    ) -> Self {
        let registry = match (registry, &db) {
            (Some(r), _) => r,
            (None, Some(pool)) => Arc::new(ModelRegistry::new(pool.clone())),
            (None, None) => Arc::new(ModelRegistry::new(
                SqlitePool::connect_lazy("sqlite::memory:")
                    .expect("in-memory SQLite pool must be creatable"),
            )),
        };
        Self {
            start_time: Instant::now(),
            version: version.into(),
            env_report: Arc::new(RwLock::new(EnvReport {
                python_path: String::new(),
                python_version: String::new(),
                torch_version: String::new(),
                preflight_ok: false,
                reason: "unavailable".to_string(),
            })),
            hardware: Arc::new(RwLock::new(hardware)),
            db,
            registry,
            model_dirs: model_dirs.unwrap_or_default(),
            broadcaster,
            workers,
            scheduler,
            artifact_store,
            config,
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

    /// Replace the current `EnvReport` with a new value.
    ///
    /// Called by preflight at startup to populate real environment data.
    pub fn set_env_report(&self, report: EnvReport) {
        *self.env_report.write().unwrap() = report;
    }

    /// Returns a clone of the current hardware information.
    pub fn hardware(&self) -> HardwareInfo {
        self.hardware.read().unwrap().clone()
    }
}

impl<A: ArtifactSave + Clone + 'static> Clone for AppState<A> {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            version: self.version.clone(),
            env_report: Arc::clone(&self.env_report),
            hardware: Arc::clone(&self.hardware),
            db: self.db.clone(),
            registry: Arc::clone(&self.registry),
            model_dirs: self.model_dirs.clone(),
            broadcaster: Arc::clone(&self.broadcaster),
            workers: self.workers.clone(),
            scheduler: self.scheduler.clone(),
            artifact_store: self.artifact_store.clone(),
            config: self.config.clone(),
        }
    }
}
