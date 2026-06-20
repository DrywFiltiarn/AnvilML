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

    // Build the job scheduler with the node registry, database pool,
    // and event broadcaster. The queue and ledger are freshly initialised
    // — the queue starts empty and the ledger has no registered devices
    // (VRAM checks are added in Phase 014).
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        Arc::clone(&node_registry),
        pool.clone(),
        Arc::new(EventBroadcaster::new()),
    ));

    // Spawn managed workers for all detected GPU devices.
    // Each worker is a Python subprocess that executes inference nodes.
    // The worker pool spawns a background monitoring task per worker that
    // broadcasts status changes to connected WebSocket clients.
    // We need an EventBroadcaster for the worker pool. Since the full
    // AppState requires the workers pool (circular dependency), we
    // create a temporary AppState first to obtain the broadcaster.
    //
    // However, hardware_info must be borrowed for spawn_all() before
    // it is moved into the temp AppState. We handle this by cloning
    // hardware_info for the temp state (acceptable cost — one hardware
    // snapshot is small, and the clone is dropped immediately after).
    let temp_state = AppState::new_with_hardware_no_workers(
        env!("CARGO_PKG_VERSION"),
        Arc::new(tokio::sync::RwLock::new(hardware_info.clone())),
        pool.clone(),
        Arc::new(ModelStore::new(pool.clone()).await),
        cfg.model_dirs.clone(),
        Arc::clone(&node_registry),
        scheduler.clone(),
    );

    let workers = WorkerPool::spawn_all(
        &cfg,
        &hardware_info.gpus,
        Arc::new(transport),
        temp_state.broadcaster.clone(),
        Arc::clone(&node_registry),
    )
    .await
    .expect("failed to spawn worker pool");

    // Drop the temp state — we only needed its broadcaster.
    // The temp hardware Arc is dropped here, freeing the original
    // hardware_info clone.
    drop(temp_state);

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
        Arc::new(workers),
        Arc::clone(&node_registry),
        scheduler,
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
    // The workers field was populated before this point, so the router
    // has access to the worker pool via state.workers.
    // Clone the workers Arc before moving state into the router, so we can
    // pass it to stats_tick::start() after the router is built.
    let workers = state.workers.clone().unwrap();
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
    // knowledge of the worker subprocesses spawned via `WorkerPool`. Without
    // this call, Ctrl+C (or SIGTERM) left every Python worker process
    // running after the supervisor exited, since dropping a
    // `tokio::process::Child` does not terminate the underlying OS process.
    // `shutdown_all` sends each worker a graceful `Shutdown` IPC message and
    // force-kills any that don't exit within their grace period.
    workers.shutdown_all().await;
}
