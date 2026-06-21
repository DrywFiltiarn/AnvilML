//! AnvilML server binary — binds and serves the HTTP/WebSocket router.
//!
//! This binary is the entry point for the AnvilML server. It parses CLI
//! arguments, loads configuration from a TOML file with layered precedence,
//! creates shared application state, builds the axum router, binds a TCP
//! listener on the configured address, and runs the server until it is
//! terminated.

mod cli;
mod shutdown;

// Re-export config loading from anvilml-core under a local `config` module.
// This keeps the main.rs imports clean and matches the established convention
// of using a local module prefix for config-related types.
mod config {
    pub use anvilml_core::{load, ConfigOverrides};
}

use anvilml_artifacts::ArtifactStore;
use anvilml_core::NodeTypeRegistry;
use anvilml_hardware::detect_all_devices;
use anvilml_ipc::{EventBroadcaster, RouterTransport};
use anvilml_registry::{open, ModelStore};
use anvilml_scheduler::{ledger::VramLedger, queue::JobQueue, scheduler::JobScheduler};
use anvilml_server::{build_router, AppState};
use anvilml_worker::WorkerPool;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::filter::Directive;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Parse CLI arguments at startup. The clap-derived Args struct validates
    // all inputs (e.g. IpAddr format) at parse time, failing fast with a
    // user-friendly error message before any server setup begins.
    let args = cli::parse();

    // Build ConfigOverrides from CLI flags. The host field is converted from
    // Option<IpAddr> to Option<String> to match ConfigOverrides' field type.
    // IpAddr::to_string() produces a valid string representation (e.g.
    // "127.0.0.1" for IPv4, "[::1]" for IPv6) that ConfigOverrides stores
    // directly into the host field.
    let overrides = config::ConfigOverrides {
        host: args.host.map(|ip| ip.to_string()),
        port: args.port,
    };

    // Build the log filter from ANVILML_LOG, falling back to "info".
    // After resolving the base filter, unconditionally add a directive that
    // suppresses sqlx::query at debug and below. This prevents sqlx from
    // flooding logs with per-query execution traces during normal debug
    // sessions. A user who explicitly needs query traces can override by
    // including "sqlx::query=debug" in ANVILML_LOG, which will shadow
    // this directive because user-supplied directives take precedence over
    // programmatically-added ones in the order they are applied.
    let filter = EnvFilter::try_from_env("ANVILML_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive(
            "sqlx::query=off"
                .parse::<Directive>()
                .expect("valid directive"),
        );

    match args.log_format {
        cli::LogFormat::Plain => {
            // with_ansi(...): tracing-subscriber's own TTY detection for
            // whether to emit ANSI color/style codes is not reliable across
            // all pipe/redirect configurations, particularly on Windows.
            // We make the decision explicit instead: colorize only when
            // stdout is actually attached to a terminal. When piped (e.g.
            // to a log aggregator, a file, or a test harness reading stdout
            // programmatically), ANSI escape sequences would otherwise end
            // up embedded inside structured field names — for example
            // `addr=` rendered as `addr` + an ANSI reset + `=`, with no
            // contiguous "addr=" substring — which breaks any consumer
            // doing plain substring/field parsing on the output.
            //
            // std::io::IsTerminal::is_terminal() is stable since Rust 1.70
            // and requires no extra dependency.
            use std::io::IsTerminal;
            let interactive = std::io::stdout().is_terminal();
            tracing_subscriber::fmt::Subscriber::builder()
                .with_env_filter(filter)
                .with_ansi(interactive)
                .init();
        }
        cli::LogFormat::Json => {
            tracing_subscriber::fmt::Subscriber::builder()
                .with_env_filter(filter)
                .json()
                .init();
        }
    }

    // Load the full configuration: defaults → TOML file → env vars → CLI overrides.
    // The config::load() function implements the four-level precedence chain.
    // If the TOML file does not exist, defaults are used silently.
    let cfg = config::load(&args.config, &overrides).expect("failed to load config");

    // Log the resolved bind address and port at INFO level (mandatory log point
    // per ENVIRONMENT.md §9 — Server subsystem, "config loaded" event).
    // This is the first operational log line after config resolution.
    tracing::info!(host = %cfg.host, port = %cfg.port, "config loaded");

    // Open the real file-backed database. The `open()` function creates the
    // database file if it does not exist, enables WAL mode, runs migrations,
    // and resets ghost jobs from any unclean shutdown.
    let pool = open(&cfg.db_path).await.expect("failed to open database");

    // Log the database path at INFO level (mandatory log point per
    // ENVIRONMENT.md §9 — Database subsystem, "SQLite file created" event).
    // The `open()` function already logs "database created" when a new file
    // is created, but we log here unconditionally so the operator always sees
    // which database path the server is using at startup.
    tracing::info!(path = %cfg.db_path.display(), "database opened");

    // Run SHA256-gated SQL seed files. On first run this populates
    // device_capabilities (and any future seed tables). Subsequent
    // runs skip unchanged files via hash comparison.
    anvilml_registry::seed_loader::run(&pool, &cfg.seeds_path)
        .await
        .expect("seed loading failed");

    // Detect all hardware devices at startup. The real database pool is now
    // available for future device capability seeding.
    // detect_all_devices never panics and always returns at least one device.
    let hardware_info = detect_all_devices(&cfg, &pool)
        .await
        .expect("hardware detection failed");

    // Log each detected device at INFO level (mandatory log point per
    // ENVIRONMENT.md §9 — Hardware subsystem, "each detected device" event).
    for dev in &hardware_info.gpus {
        tracing::info!(
            index = dev.index,
            name = %dev.name,
            device_type = ?dev.device_type,
            vram_total_mib = dev.vram_total_mib,
            fp8 = dev.caps.fp8,
            "hardware detected"
        );
    }

    // Bind the IPC transport. The ROUTER socket accepts connections from
    // worker DEALER sockets. Binding to port 0 lets the OS assign an
    // available port, avoiding conflicts when multiple server instances
    // run concurrently. The assigned port is passed to workers via env vars.
    let transport = RouterTransport::bind()
        .await
        .expect("failed to bind IPC transport");

    // The node type registry is shared across the worker pool and AppState.
    // Constructed once before spawn_all() so the same Arc can be cloned
    // into every worker's spawn() call — a fresh registry per worker would
    // mean each worker's node types never accumulate into one shared,
    // queryable set.
    let node_registry = Arc::new(NodeTypeRegistry::new().await);

    // Create the artifact store before the scheduler — the scheduler needs
    // it to persist images when WorkerEvent::ImageReady arrives.
    // The artifact directory is created automatically on construction.
    let artifact_store = Arc::new(ArtifactStore::new(cfg.artifact_dir.clone(), pool.clone()).await);

    // The broadcaster is constructed once, here, and shared by both the
    // scheduler (which the event loop subscribes to via
    // `scheduler.broadcaster()`) and the worker pool (which calls
    // `broadcast_worker_event` on it from `ManagedWorker::run()`). These
    // two consumers MUST share the exact same `Arc<EventBroadcaster>`
    // instance — a `tokio::sync::broadcast` channel only delivers to
    // receivers subscribed to that specific sender, so a second,
    // independently-constructed `EventBroadcaster` has its own separate
    // channel with zero subscribers, and every `broadcast_worker_event`
    // call against it silently goes nowhere (logged at DEBUG as "no
    // subscribers", easy to miss). This was previously the case: the
    // temporary `AppState` built below (now removed) called
    // `new_with_hardware_no_workers`, which fabricates its own
    // `EventBroadcaster::new()` internally rather than accepting one —
    // so the worker pool was wired to a broadcaster the event loop was
    // never subscribed to, and `Completed`/`Failed` events vanished
    // silently even though every other part of the pipeline (dispatch,
    // VRAM, IPC addressing) was correct.
    let broadcaster = Arc::new(EventBroadcaster::new());

    // Spawn managed workers for all detected GPU devices.
    // Each worker is a Python subprocess that executes inference nodes.
    // The worker pool spawns a background monitoring task per worker that
    // broadcasts status changes to connected WebSocket clients.
    //
    // We must spawn workers before constructing the scheduler because the
    // scheduler needs a reference to the worker pool for job cancellation
    // (the `cancel_job` method sends IPC messages via the pool).
    //
    // Passes the same `broadcaster` constructed above — see that
    // variable's doc comment for why this must not be a second,
    // independently-constructed `EventBroadcaster`.
    let workers = WorkerPool::spawn_all(
        &cfg,
        &hardware_info.gpus,
        Arc::new(transport),
        Arc::clone(&broadcaster),
        Arc::clone(&node_registry),
    )
    .await
    .expect("failed to spawn worker pool");

    // Wrap the worker pool in an Arc now so it can be shared between the
    // dispatch loop, the event loop, AppState, and the later shutdown call.
    let workers = Arc::new(workers);

    // Build the job scheduler with the node registry, database pool,
    // event broadcaster, and worker pool reference. The queue and ledger
    // are freshly initialised — the queue starts empty and the ledger has
    // no registered devices (VRAM checks are added in Phase 014).
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        Arc::clone(&node_registry),
        pool.clone(),
        Arc::clone(&broadcaster),
        Arc::clone(&artifact_store),
        Some(Arc::clone(&workers)),
    ));

    // Restore any jobs left in `Queued` status by a prior process (e.g. an
    // unclean shutdown, or simply a restart) into the freshly-constructed
    // in-memory queue above. Without this, such jobs sit in the database
    // forever — invisible to the dispatch loop — and queue_position for
    // new submissions silently undercounts. Must happen before any HTTP
    // traffic is accepted and before the dispatch loop starts, so it can't
    // race with a concurrent submit().
    scheduler
        .rehydrate_queue()
        .await
        .expect("failed to rehydrate job queue from database");

    // Register every detected device's VRAM capacity with the dispatch
    // loop's VRAM ledger. The ledger starts with zero registered devices
    // (see `VramLedger::new()` above) — without this loop, every device
    // index is "unknown" to `would_fit`, which treats an unregistered
    // device as having zero free VRAM, so the dispatch loop would never
    // dispatch a single job regardless of how many workers are Idle.
    // Must happen before `start_dispatch_loop` below, so the ledger is
    // fully populated before the loop's first dispatch attempt.
    for gpu in &hardware_info.gpus {
        scheduler
            .register_device(gpu.index, gpu.vram_total_mib)
            .await;
        tracing::info!(
            index = gpu.index,
            vram_total_mib = gpu.vram_total_mib,
            "registered device with VRAM ledger"
        );
    }

    // Start the scheduler's two background tasks. Without these, jobs are
    // accepted via POST /v1/jobs and persisted as Queued, but nothing ever
    // moves them to Running/Completed/Failed:
    //
    // - `start_dispatch_loop` selects an Idle worker and sends Execute for
    //   queued jobs (wakes on submit() or a worker becoming Idle).
    // - `start_event_loop` subscribes to WorkerEvent::Completed/Failed and
    //   updates job status in the DB accordingly.
    //
    // Both return a JoinHandle for a task that runs for the lifetime of the
    // process. We bind both handles (an unused JoinHandle still represents
    // a live, running task — tokio tasks are NOT cancelled on drop, only
    // detached) so they are available to be aborted explicitly during
    // graceful shutdown below, rather than left to run past listener close.
    let dispatch_handle = scheduler.start_dispatch_loop(Arc::clone(&workers));
    let event_loop_handle = scheduler.start_event_loop();

    // Create the real shared application state with the worker pool included.
    // env!("CARGO_PKG_VERSION") is a compile-time literal that implements
    // Into<String>, matching the constructor's type.
    let registry = Arc::new(ModelStore::new(pool.clone()).await);
    let state = AppState::new_with_hardware(
        env!("CARGO_PKG_VERSION"),
        Arc::new(tokio::sync::RwLock::new(hardware_info)),
        pool,
        registry,
        cfg.model_dirs.clone(),
        Arc::clone(&workers),
        Arc::clone(&node_registry),
        scheduler,
        artifact_store,
    );

    // Run the initial model directory scan at startup. This populates the
    // model registry before the server starts accepting requests, so models
    // are available immediately without requiring a manual POST to /v1/models/rescan.
    // The scanner logs completion at INFO with count= and dir= fields.
    // If the scan fails (e.g. all directories missing), we log WARN but
    // continue — models will be picked up on the first manual rescan.
    let dirs_string: Vec<String> = cfg
        .model_dirs
        .iter()
        .map(|d| d.path.to_string_lossy().into_owned())
        .collect();
    match state.registry.scan_and_upsert(&cfg.model_dirs).await {
        Ok(n) => {
            tracing::info!(
                count = n,
                dir = %dirs_string.join(","),
                "initial scan completed"
            );
        }
        Err(e) => {
            // Initial scan failure is non-fatal — the server can still
            // start and models will be discovered on the first manual rescan.
            tracing::warn!(error = %e, "initial scan failed, will retry on first rescan");
        }
    }

    // Build the axum router with all registered handlers wired to their routes.
    // AppState is Clone because its fields (Arc, Vec, String) are all Clone.
    // `workers` (bound above, before AppState construction) is the same Arc
    // now stored in state.workers — no need to re-derive it from state here.
    let router = build_router(state);

    // Build the bind address from the resolved config values.
    // cfg.host is a String and cfg.port is a u16, so format! produces the
    // correct address string. IPv6 addresses from cfg.host will produce
    // "[::1]:port" which TcpListener::bind accepts natively.
    let addr = format!("{}:{}", cfg.host, cfg.port);

    // Bind a TCP listener on the configured address.
    // tokio::net::TcpListener::bind is async and must be awaited.
    // When port is 0, the OS assigns a random port. We extract the actual
    // bound address from the listener for logging, so tests can parse it.
    let listener = TcpListener::bind(&addr)
        .await
        .expect("failed to bind listener");

    // Extract the actual bound address from the listener.
    // This is important when port 0 was configured — the OS assigns
    // a random port and we need the real address for logging and tests.
    let actual_addr = listener
        .local_addr()
        .expect("failed to get local address of bound listener");

    // Log the bind address at INFO level (mandatory log point per ENVIRONMENT.md §9.2).
    // Uses the actual bound address rather than the configured one, so the
    // logged addr field always reflects the true bind point.
    tracing::info!(addr = %actual_addr, "listening");

    // Start the system stats background tick task.
    // This spawns a tokio task that broadcasts CPU and RAM metrics every
    // 5 seconds via the WebSocket event stream. Starting after the bind
    // log ensures the broadcaster is initialised but before accepting
    // connections so events flow immediately to the first subscriber.
    // The stats tick uses the WorkerPool for both the broadcaster
    // (via pool.broadcaster()) and the worker info snapshot.
    anvilml_server::ws::stats_tick::start(workers.clone());

    // Run the server until a fatal error occurs. The .expect() provides a
    // user-visible error message if the server encounters a fatal error during serving.
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown::shutdown_signal())
    .await
    .expect("server error");

    // `with_graceful_shutdown` only stops axum from accepting new HTTP
    // connections and waits for in-flight requests to finish — it has no
    // knowledge of the worker subprocesses spawned via `WorkerPool`, nor of
    // the scheduler's two background loops. Without this call, Ctrl+C (or
    // SIGTERM) left every Python worker process running after the
    // supervisor exited, since dropping a `tokio::process::Child` does not
    // terminate the underlying OS process.
    // `shutdown_all` sends each worker a graceful `Shutdown` IPC message and
    // force-kills any that don't exit within their grace period.
    workers.shutdown_all().await;

    // Stop the dispatch and event loops explicitly. Both loop forever by
    // design (`loop { ... }` with no exit condition), so once the listener
    // is closed and workers are shut down there is nothing left for them
    // to do — abort() is safe here because neither loop holds a resource
    // that needs orderly async cleanup beyond what's already been done by
    // shutdown_all() above.
    dispatch_handle.abort();
    event_loop_handle.abort();
}
