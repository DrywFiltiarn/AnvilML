use std::sync::Arc;

/// AppState holds shared server state accessible to all HTTP handlers.
///
/// Constructed once at server boot and cloned into each handler via axum's
/// `State` extractor. `start_time` is used to compute server uptime;
/// `version` is the crate version from `CARGO_PKG_VERSION`;
/// `env_report` is the stub environment report returned by the `/v1/system/env`
/// endpoint (populated by future tasks); `hardware` is the hardware snapshot
/// populated by `detect_all_devices()` at startup (Phase 004); `db` is the
/// file-backed SQLite connection pool wired at startup (Phase 005);
/// `registry` is the model store for CRUD operations on model metadata;
/// `broadcaster` is the WebSocket event broadcaster for pushing real-time
/// events to connected clients (Phase 007).
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

    /// File-backed SQLite connection pool for persistent storage.
    /// Opened at server startup via `anvilml_registry::open()` and
    /// shared across all handlers that need database access.
    pub db: sqlx::SqlitePool,

    /// Model store for CRUD operations on model metadata.
    /// Shared via `Arc` so all handlers can access the model registry
    /// without cloning the pool or the store.
    pub registry: Arc<anvilml_registry::ModelStore>,

    /// Configured model directories to scan.
    ///
    /// Populated from `cfg.model_dirs` at server startup. The rescan
    /// handler reads these paths to know which directories to scan.
    /// Cloned into each handler via axum's `State` extractor.
    pub model_dirs: Vec<anvilml_core::ModelDirConfig>,

    /// WebSocket event broadcaster for real-time event delivery.
    ///
    /// Wrapped in `Arc` so it can be shared across handlers and spawned
    /// tasks without cloning the broadcast sender. Each clone of `AppState`
    /// shares the same `Arc<EventBroadcaster>`, so all handlers broadcast
    /// to the same set of subscribers.
    pub broadcaster: Arc<crate::ws::EventBroadcaster>,

    /// The managed worker pool, if spawned at server startup.
    ///
    /// `Some(pool)` in production after `WorkerPool::spawn_all()` succeeds.
    /// `None` in tests that use `AppState::new()` for stub mode — the
    /// `GET /v1/workers` handler returns an empty JSON array when this
    /// field is `None`.
    pub workers: Option<Arc<anvilml_worker::WorkerPool>>,
}

impl AppState {
    /// Create a new AppState with the given server version.
    ///
    /// The `start_time` is set to the current instant. The `version` is stored
    /// by converting the argument into an owned `String` via `Into<String>`,
    /// which accepts `String`, `&str`, and `&'static str`. The `env_report`
    /// field is initialized with default values (a stub for future population).
    /// The `hardware` field is initialized with a default (empty) `HardwareInfo`.
    ///
    /// This constructor is async because `SqlitePool::connect` is async.
    /// It is intended for use in tests and stubs — production code should
    /// use `new_with_hardware` instead. The `db` field is initialised with
    /// an in-memory pool.
    pub async fn new(version: impl Into<String>) -> Self {
        // Use open_in_memory() to create an in-memory pool with migrations
        // already applied. This is critical — the ModelStore queries tables
        // that only exist after migrations run. Using raw SqlitePool::connect
        // would result in 500 errors on any database operation.
        let pool = anvilml_registry::open_in_memory()
            .await
            .expect("in-memory pool for stub AppState");

        Self {
            start_time: std::time::Instant::now(),
            version: version.into(),
            env_report: anvilml_core::types::EnvReport::default(),
            hardware: Arc::new(tokio::sync::RwLock::new(
                anvilml_core::types::HardwareInfo::default(),
            )),
            db: pool.clone(),
            // Construct the model store from the in-memory pool.
            // This is only used by tests — production code constructs the
            // ModelStore separately and passes it via new_with_hardware.
            registry: Arc::new(anvilml_registry::ModelStore::new(pool).await),
            // Empty model_dirs for tests — the rescan handler will scan
            // no directories when model_dirs is empty, which is the
            // correct behavior (202 response with no models found).
            model_dirs: Vec::new(),
            // The broadcaster is shared across all handlers and spawned tasks.
            // Cloning AppState clones the Arc, not the sender itself.
            broadcaster: Arc::new(crate::ws::EventBroadcaster::new()),
            // Workers pool is None for stub/test mode — the workers handler
            // returns an empty array when workers is None.
            workers: None,
        }
    }

    /// Create a new AppState with hardware detection results, a database
    /// connection pool, and a model store.
    ///
    /// This constructor is used at server startup after `detect_all_devices()`
    /// has populated the hardware snapshot and `anvilml_registry::open()` has
    /// opened the file-backed database. The version, hardware, and database
    /// data are stored directly; `env_report` is initialised with default
    /// values.
    ///
    /// # Arguments
    ///
    /// * `version` — The server version string (e.g. from `CARGO_PKG_VERSION`).
    /// * `hardware` — A pre-detect `Arc<RwLock<HardwareInfo>>` containing the
    ///   hardware snapshot from `detect_all_devices()`.
    /// * `db` — A file-backed `SqlitePool` opened via `anvilml_registry::open()`
    ///   at the path specified in server configuration.
    /// * `registry` — A pre-built `Arc<ModelStore>` for model metadata CRUD.
    ///   The caller constructs this after opening the pool, avoiding the
    ///   sync/async boundary in this synchronous constructor.
    /// * `model_dirs` — Configured model directories for the scanner.
    ///   Passed from `cfg.model_dirs` at server startup.
    /// * `workers` — The managed worker pool, already spawned via
    ///   `WorkerPool::spawn_all()`. Provides worker state for the
    ///   `/v1/workers` handler and the system stats tick.
    pub fn new_with_hardware(
        version: impl Into<String>,
        hardware: Arc<tokio::sync::RwLock<anvilml_core::types::HardwareInfo>>,
        db: sqlx::SqlitePool,
        registry: Arc<anvilml_registry::ModelStore>,
        model_dirs: Vec<anvilml_core::ModelDirConfig>,
        workers: Arc<anvilml_worker::WorkerPool>,
    ) -> Self {
        // Borrow the broadcaster that the pool was already constructed with,
        // so AppState.broadcaster and pool.broadcaster() are the same Arc.
        let broadcaster = workers.broadcaster().clone();
        Self {
            start_time: std::time::Instant::now(),
            version: version.into(),
            env_report: anvilml_core::types::EnvReport::default(),
            hardware,
            db,
            registry,
            model_dirs,
            broadcaster,
            workers: Some(workers),
        }
    }

    /// Create a new AppState with hardware detection results but without
    /// a worker pool.
    ///
    /// This constructor is used for the temporary AppState at server
    /// startup to obtain the `EventBroadcaster` before spawning workers.
    /// The `workers` field is `None`; production code should use
    /// `new_with_hardware` with a populated worker pool instead.
    ///
    /// # Arguments
    ///
    /// * `version` — The server version string (e.g. from `CARGO_PKG_VERSION`).
    /// * `hardware` — A pre-detect `Arc<RwLock<HardwareInfo>>` containing the
    ///   hardware snapshot from `detect_all_devices()`.
    /// * `db` — A file-backed `SqlitePool` opened via `anvilml_registry::open()`.
    /// * `registry` — A pre-built `Arc<ModelStore>` for model metadata CRUD.
    /// * `model_dirs` — Configured model directories for the scanner.
    pub fn new_with_hardware_no_workers(
        version: impl Into<String>,
        hardware: Arc<tokio::sync::RwLock<anvilml_core::types::HardwareInfo>>,
        db: sqlx::SqlitePool,
        registry: Arc<anvilml_registry::ModelStore>,
        model_dirs: Vec<anvilml_core::ModelDirConfig>,
    ) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            version: version.into(),
            env_report: anvilml_core::types::EnvReport::default(),
            hardware,
            db,
            registry,
            model_dirs,
            // The broadcaster is shared across all handlers and spawned tasks.
            // Cloning AppState clones the Arc, not the sender itself.
            broadcaster: Arc::new(crate::ws::EventBroadcaster::new()),
            workers: None,
        }
    }
}
