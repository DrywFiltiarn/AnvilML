mod cli;

use tracing_subscriber::EnvFilter;

use anvilml::shutdown;
use anvilml_artifacts::ArtifactStore;
use anvilml_core::CliOverrides;
use anvilml_core::EnvReport;
use anvilml_core::NodeTypeRegistry;
use anvilml_core::ProvisioningState;
use anvilml_core::config_load;
use anvilml_hardware::detect_all_devices;
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::JobStore;
use anvilml_registry::ModelStore;
use anvilml_registry::create_pool;
use anvilml_registry::trigger_model_scan;
use anvilml_scheduler::JobScheduler;
use anvilml_scheduler::spawn_event_loop;
use anvilml_server::ws::spawn_stats_tick;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

/// Maximum time to wait, after the shutdown signal fires, for in-flight HTTP
/// connections to close on their own before proceeding with shutdown anyway.
///
/// This bounds only the post-signal drain phase — NOT the wait for the
/// signal itself, which is unbounded by design (the server runs until
/// Ctrl-C). Without this bound, a client holding a connection open
/// indefinitely — most notably a `GET /v1/events` WebSocket — would prevent
/// the process from ever exiting. See the call site in `main()` for the
/// full reasoning.
const HTTP_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

/// Entry point for the AnvilML server binary.
///
/// Parses CLI arguments, loads `ServerConfig` through the four-layer
/// precedence chain (defaults → TOML → env vars → CLI flags) via
/// `config_load::load()`, then branches on the parsed subcommand:
///
/// - `hw-probe` — calls `detect_all_devices()`, serialises the result
///   to pretty JSON on stdout, and exits 0.
/// - no subcommand (default) — builds the HTTP router, binds a TCP
///   listener on the loaded host and port, then serves HTTP requests
///   until a shutdown signal (Ctrl+C / SIGINT) is received.
///
/// If config loading fails, prints the error and exits with code 1
/// before binding any socket or running hardware detection.
#[tokio::main]
async fn main() {
    // Parse CLI arguments first — we need the `log_format` value to
    // choose the subscriber output format (plain or json).
    let cli = cli::parse();

    // Initialize the tracing subscriber as the very first startup step.
    // Reads filter from ANVILML_LOG (primary) or RUST_LOG (fallback),
    // defaulting to "info" when neither is set — matching the precedence
    // documented in ENVIRONMENT.md §3.3.
    // Output format is controlled by --log-format (plain or json), not by
    // an environment variable, per ENVIRONMENT.md §3.3.
    // Write to stderr so tracing output does not mix with stdout data
    // (e.g. `hw-probe` JSON output goes to stdout, logs go to stderr).
    let builder = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("ANVILML_LOG")
                .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr);

    // Branch on the parsed log_format value.
    // "plain" keeps the default text formatter; "json" switches to
    // newline-delimited JSON via the tracing-subscriber json feature.
    // The EnvFilter precedence is identical in both branches.
    match cli.log_format.as_str() {
        "json" => builder.json().init(),
        // "plain" — default text formatter.
        _ => builder.init(),
    };

    // Build `CliOverrides` from the parsed CLI fields.
    // `host` and `port` are `Option` — `None` means the caller did not
    // set a CLI flag, so the override is silently skipped and the
    // config value from the prior layers (env var / TOML / default) wins.
    let cli_overrides = CliOverrides {
        host: cli.host,
        port: cli.port,
    };

    // Load the full `ServerConfig` through the four-layer precedence chain.
    // Pass the TOML path (if provided via --config) and CLI overrides.
    let config = config_load::load(cli.config.as_deref().map(Path::new), Some(cli_overrides))
        .map_err(|e| {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        })
        .unwrap();

    // Branch on the parsed subcommand.
    // `hw-probe` runs hardware detection and exits; the default `None`
    // path starts the HTTP server as before.
    match cli.command {
        Some(cli::Commands::HwProbe) => {
            // Detect all hardware devices using the loaded config.
            // This is the same detection path used at server startup,
            // ensuring consistent results between probe and runtime.
            let hw_info = detect_all_devices(&config).await.unwrap();

            // Serialize to pretty-printed JSON for human readability.
            // `HardwareInfo` derives `Serialize` via serde, so this
            // always succeeds for well-formed data.
            let json = serde_json::to_string_pretty(&hw_info)
                .expect("HardwareInfo serialization must succeed");

            // Print to stdout and exit 0 — no server, no socket.
            println!("{json}");
            std::process::exit(0);
        }
        None => {
            // Default path: start the HTTP server.
        }
    }

    // Create the database pool and run migrations.
    // This is called before binding the TCP listener so that a DB failure
    // prevents the server from starting with no database — matching the
    // config-load failure pattern (eprintln + exit 1).
    let pool = create_pool(&config.db_path)
        .await
        .map_err(|e| {
            eprintln!("Failed to create database pool: {e}");
            std::process::exit(1);
        })
        .unwrap();

    // Load device capability seed data from the checked-in SQL file.
    // The seed is hash-gated and idempotent — if the file hasn't changed
    // since last run, this is a no-op. On failure, exit before binding
    // any socket, matching the create_pool() error pattern.
    //
    // The seed path can be overridden via ANVILML_SEED_PATH for testing
    // (e.g. pointing to a temp file or a non-existent path).
    // Resolve the seed file path: use ANVILML_SEED_PATH env var override if set,
    // otherwise fall back to the checked-in seed path relative to CWD.
    // Using PathBuf to own the path data (the env var string must live long enough).
    let seed_path: std::path::PathBuf = match std::env::var("ANVILML_SEED_PATH") {
        Ok(path) => Path::new(&path).to_path_buf(),
        Err(_) => Path::new("database/seeds/devices.sql").to_path_buf(),
    };

    tracing::info!(seed_path = %seed_path.display(), "loading device capabilities seed");

    let loader = anvilml_registry::SeedLoader::new(pool.clone());
    loader
        .run("devices.sql", &seed_path)
        .await
        .map_err(|e| {
            eprintln!("Failed to apply device capabilities seed: {e}");
            std::process::exit(1);
        })
        .unwrap();

    // Reset any stale "ghost" jobs left over from a previous run.
    // Ghost jobs are those in Queued or Running state — they may have been
    // in-flight when the server crashed or was restarted. The reset transitions
    // them to Failed with error = "server_restart" so they are visible to the
    // operator and can be retried or discarded.
    // The pool is cloned for the JobStore; the clone is cheap (shared connection
    // pool, not a new database connection).
    let job_store = JobStore::new(pool.clone());
    let _ghost_count = job_store
        .reset_ghost_jobs()
        .await
        .map_err(|e| {
            eprintln!("Failed to reset ghost jobs: {e}");
            std::process::exit(1);
        })
        .unwrap();

    // Detect all hardware devices (GPU/CPU) using the loaded config.
    // The mock-hardware feature replaces real detectors with MockDetector,
    // which returns synthetic device info driven by ANVILML_MOCK_* env vars.
    // On Linux/Windows, real detectors (Vulkan, DXGI, sysfs) enumerate
    // actual GPUs; CPU detection always returns one CPU device as fallback.
    let hw_info = detect_all_devices(&config)
        .await
        .map_err(|e| {
            eprintln!("Failed to detect hardware devices: {e}");
            std::process::exit(1);
        })
        .unwrap();

    // Log the detected devices for operator visibility.
    // device_count is the number of GPUs/CPU detected; MockDetector
    // returns 1 device (mock GPU) when mock-hardware is active.
    tracing::info!(
        device_count = hw_info.gpus.len(),
        "hardware devices detected"
    );

    // Wrap the hardware snapshot in Arc<RwLock> for sharing with AppState.
    // Clone before wrapping so the original `hw_info` is still available
    // for WorkerPool::spawn_all() below — we need `&hw_info.gpus`.
    // The RwLock allows future VRAM-refresh paths to update the snapshot
    // without reconstructing the entire struct; the scheduler reads it
    // during dispatch.
    let hardware = Arc::new(RwLock::new(hw_info.clone()));

    // Construct the worker pool (empty), spawn workers for each device,
    // then wrap in Arc for sharing with AppState and the dispatch loop.
    // WorkerPool::new() binds a RouterTransport and spawns the bridge;
    // spawn_all() populates the pool with one ManagedWorker per device.
    // Use a distinct name from the `pool` SqlitePool to avoid shadowing.
    let mut worker_pool = WorkerPool::new()
        .await
        .expect("WorkerPool::new() must succeed at startup");

    // Create the shared node registry before spawning workers.
    // Clone the Arc and pass it to spawn_all() so each ManagedWorker
    // registers its node types into this same registry — the scheduler
    // and server handlers query the same Arc, so `is_empty()` returns
    // false once any worker has sent a Ready event.
    let node_registry = Arc::new(NodeTypeRegistry::new());

    // Spawn a Python worker subprocess for each detected device.
    // Each worker connects to the RouterTransport's DEALER socket,
    // registers itself, and enters the message dispatch loop.
    // With mock-hardware: spawns mock workers (no real Python interpreter).
    // Without mock-hardware: spawns real Python workers that import torch.
    worker_pool
        .spawn_all(&hw_info.gpus, &config, Arc::clone(&node_registry))
        .await
        .expect("WorkerPool::spawn_all() must succeed at startup");

    // Log worker count for operator visibility.
    tracing::info!(
        worker_count = worker_pool.handles().len(),
        "workers spawned"
    );

    let workers = Arc::new(worker_pool);

    // Construct the event broadcaster once and share it with both
    // the scheduler's event loop and AppState — two independently
    // constructed broadcasters would silently never see each other's
    // events.
    let broadcaster = Arc::new(EventBroadcaster::new());

    // Construct the job scheduler with the database pool.
    // The scheduler owns the in-memory job queue and dispatch loop;
    // it uses the shared `pool` (SqlitePool) for job persistence via `JobStore`.
    // Construct the artifact store before the scheduler so it can be passed
    // to JobScheduler::new() — the scheduler needs the artifact store for
    // the event_loop module's handle_image_ready() function.
    let artifact_store = Arc::new(ArtifactStore::new(
        config.artifact_dir.clone(),
        pool.clone(),
    ));

    let job_store = JobStore::new(pool.clone());
    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        Arc::clone(&artifact_store),
        Arc::clone(&workers).transport().clone(),
    ));

    // Keep the dispatch loop's JoinHandle (not discarded via `_`) — the
    // graceful shutdown sequence below needs to abort and await it, to
    // release its own Arc<WorkerPool> clone before reclaiming exclusive
    // ownership. start_dispatch_loop consumes Arc<Self>, so we clone the
    // Arc first — the clone is cheap (just an atomic ref-count bump).
    let dispatch_handle = Arc::clone(&scheduler).start_dispatch_loop(Arc::clone(&workers));

    tracing::info!("dispatch loop started");

    // Construct the event loop task that consumes WorkerEvent fan-out from
    // the worker pool's Demux and publishes WsEvent to WebSocket subscribers.
    // The Demux subscription is established synchronously before the task
    // is spawned, so no events can be missed between this call and the
    // return of spawn_event_loop(). The event_loop_handle is kept so it
    // can be aborted and awaited during graceful shutdown, parallel to
    // the dispatch_handle pattern.
    let event_loop_handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(workers.demux()),
        Arc::clone(&broadcaster),
        Arc::clone(&workers),
    );

    // Construct the periodic SystemStats heartbeat task (P16-D1). Interval
    // is the production default of 5 seconds per ANVILML_DESIGN.md §13.1;
    // spawn_stats_tick() itself takes the interval as a parameter
    // specifically so tests can use a millisecond-scale value instead.
    // Holds its own Arc<WorkerPool> clone (via `workers`), so its
    // JoinHandle must be aborted and awaited during graceful shutdown
    // below, the same way dispatch_handle and event_loop_handle already
    // are — otherwise Arc::try_unwrap(workers) would fail with this
    // task's clone still outstanding.
    let stats_tick_handle = spawn_stats_tick(
        Arc::clone(&broadcaster),
        Arc::clone(&workers),
        Duration::from_secs(5),
    );

    // Capture process-start instant once, before binding, so the health
    // handler returns a real elapsed-time measurement.
    let start_time = Instant::now();

    // Best-effort initial EnvReport at startup.
    // A full preflight subsystem is a later concern — this just captures
    // the interpreter path and a conservative preflight status.
    // torch_version is None because Rust cannot import Python modules;
    // the Python worker will populate this on its Ready event later.
    let env_report = Arc::new(RwLock::new(EnvReport {
        python_path: Some(
            config
                .venv_path
                .join("bin/python3")
                .to_string_lossy()
                .into_owned(),
        ),
        python_version: None, // Will be filled by worker Ready event
        torch_version: None,
        provisioning: ProvisioningState::NotStarted,
        preflight_ok: false,
        reason: None,
        node_types: Vec::new(),
    }));

    // Construct `AppState` with the loaded config, a fresh empty node
    // registry (populated later when the Python worker sends Ready),
    // the captured start instant, and the subsystem fields.
    let app_state = AppState {
        config: Arc::new(config),
        node_registry,
        start_time,
        scheduler,
        workers: Arc::clone(&workers),
        db: pool.clone(),
        artifact_store,
        broadcaster: Arc::clone(&broadcaster),
        hardware,
        env_report,
        // Clone the pool so both JobStore and ModelStore share the same
        // connection pool — the scheduler and model store operate on
        // different tables (jobs vs models) but use the same SQLite file.
        model_store: Arc::new(ModelStore::new(pool)),
    };

    // Trigger a background model directory scan at startup.
    // This ensures the model registry is populated before the server starts
    // accepting requests — matching the contract that models must always be
    // scanned on startup (P18-C3). The scan runs in a spawned tokio task,
    // so it does not block the listener bind. Reuses trigger_model_scan()
    // to avoid duplicating the scan logic from the /v1/models/rescan handler.
    let model_dirs = app_state.config.model_dirs.clone();
    let model_scan_depth = app_state.config.model_scan_depth;
    trigger_model_scan(app_state.db.clone(), model_dirs, model_scan_depth);

    // Extract the listen address from the Arc-wrapped config before moving
    // `app_state` into `build_router`. The `Arc` ensures the config data
    // is shared, not copied, so this is zero-cost.
    let addr = format!("{}:{}", app_state.config.host, app_state.config.port);

    let router = build_router(app_state);
    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!(addr = %addr, "listening");
    // NOTE: this is deliberately NOT a plain `tokio::select! { axum::serve(..), signal }`,
    // and NOT a bare `axum::serve(..).with_graceful_shutdown(signal).await` either.
    //
    // axum::serve()'s accept loop spawns an independent tokio task per accepted
    // connection — each carrying its own clone of `router` (and therefore
    // `AppState`, therefore `Arc<WorkerPool>`). `.with_graceful_shutdown(signal)`
    // is what actually stops the accept loop AND waits for those already-spawned
    // connection tasks to finish once `signal` resolves — a plain `select!`
    // dropping the bare `axum::serve(..)` future does neither of those things
    // for tasks already spawned off of it.
    //
    // But `.with_graceful_shutdown(signal).await` on its own waits for that
    // drain *unboundedly* — a client that opens a long-lived connection (a
    // `GET /v1/events` WebSocket is the common case: you're typically watching
    // one specifically because you're expecting to observe something, e.g. a
    // worker respawn) and never disconnects would then block this `.await`
    // forever. An open WebSocket must never be able to prevent this process
    // from exiting.
    //
    // So the two phases are split explicitly: `wait_for_shutdown_signal()`
    // itself is awaited with NO timeout (the server must run indefinitely
    // until Ctrl-C — that wait is supposed to take arbitrarily long); only
    // once it resolves does `HTTP_DRAIN_TIMEOUT`'s clock start, bounding just
    // the post-signal drain. `signal_tx`/`signal_rx` is what lets the second
    // `select!` branch below know when to start that clock, since the signal
    // future itself is consumed by `with_graceful_shutdown` and can't also be
    // awaited a second time out here.
    let (signal_tx, signal_rx) = tokio::sync::oneshot::channel::<()>();
    let mut serve_fut = Box::pin(
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown::wait_for_shutdown_signal().await;
                tracing::info!("shutdown signal received, draining in-flight http connections");
                let _ = signal_tx.send(());
            })
            // `WithGracefulShutdown` implements `IntoFuture`, not `Future`
            // directly — a bare `.await` desugars through `IntoFuture`
            // transparently, but `Box::pin()` needs a concrete `Future`,
            // so that conversion has to happen explicitly here.
            .into_future(),
    );
    tokio::select! {
        result = &mut serve_fut => {
            if let Err(e) = result {
                tracing::error!(error = %e, "http server exited with an error");
            }
        }
        _ = async {
            let _ = signal_rx.await;
            tokio::time::sleep(HTTP_DRAIN_TIMEOUT).await;
        } => {
            tracing::warn!(
                timeout_secs = HTTP_DRAIN_TIMEOUT.as_secs(),
                "not every http connection drained within the timeout (e.g. a still-open \
                 GET /v1/events websocket) — proceeding with shutdown anyway; \
                 Arc::try_unwrap(workers) below may fail as a result, which is expected \
                 here and falls back to non-graceful worker cleanup"
            );
        }
    }
    // `serve_fut` is boxed (not `tokio::pin!`-shadowed) specifically so it
    // can be dropped explicitly here, on both branches — releasing whatever
    // Arc<WorkerPool> clone the accept-loop's own state was holding for
    // itself. `tokio::pin!` would leave that clone alive until `main()`
    // returns regardless of which select! branch won, which would make
    // Arc::try_unwrap(workers) below fail unconditionally on the timeout
    // path rather than only when a connection task is genuinely still open.
    // Any already-spawned, still-open connection task (the stuck WebSocket)
    // is a separate, independent tokio task — dropping serve_fut cannot
    // reach it, and that is the one remaining, expected reason
    // Arc::try_unwrap(workers) can still fail below.
    drop(serve_fut);

    // Graceful shutdown (ANVILML_DESIGN.md §19.3): stop every worker's IPC
    // message loop and subprocess cleanly, rather than relying solely on
    // Drop (JobObjectGuard's kill-on-drop only terminates the OS
    // processes — it does not stop this process's own supervisor tasks).
    // Previously nothing called this at all: wait_for_shutdown_signal()
    // just returned, main() returned, and Tokio's runtime then blocked
    // waiting for every still-running spawned task — including every
    // ManagedWorker::run() supervisor loop and this dispatch loop, none
    // of which anything ever signalled to stop — to finish, which they
    // never would on their own. That's what previously required a
    // second, harsher Ctrl+C (STATUS_CONTROL_C_EXIT) to actually exit.
    //
    // axum::serve(...)'s future — and everything it captured, including
    // its own Arc<WorkerPool> clone via app_state/router — was already
    // dropped by tokio::select!'s branch-cancellation semantics above,
    // the instant the shutdown-signal branch won. Aborting and awaiting
    // the dispatch loop's task releases its own separate clone the same
    // way. After both, this function's own `workers` binding should be
    // the last live Arc<WorkerPool> clone, making Arc::try_unwrap()
    // below succeed.
    //
    // shutdown_all() needs owned (&mut) access specifically because each
    // WorkerHandle's shutdown_tx/force_shutdown_tx are plain, non-Clone
    // oneshot::Sender by design (see WorkerHandle's own Clone impl) —
    // only the original handles stored inside WorkerPool.handles can
    // trigger shutdown at all, so no clone-based workaround is possible.
    dispatch_handle.abort();
    let _ = dispatch_handle.await;

    // Abort and await the event loop task — same pattern as dispatch_handle.
    // The event loop holds its own Arc<WorkerPool> clone via the demux
    // subscription and the workers Arc passed to spawn_event_loop();
    // aborting it releases that clone before the workers shutdown_all().
    event_loop_handle.abort();
    let _ = event_loop_handle.await;

    // Abort and await the stats tick task (P16-D1) — same pattern and same
    // reason as event_loop_handle immediately above: it holds its own
    // Arc<WorkerPool> clone that must be released before
    // Arc::try_unwrap(workers) below can succeed.
    stats_tick_handle.abort();
    let _ = stats_tick_handle.await;

    match Arc::try_unwrap(workers) {
        Ok(mut workers) => {
            workers.shutdown_all(Duration::from_secs(10)).await;
            tracing::info!("all workers shut down gracefully");
        }
        Err(_) => {
            // Two possible causes now, not one:
            // 1. HTTP_DRAIN_TIMEOUT above elapsed with a connection still
            //    open (e.g. a client that never closes its GET /v1/events
            //    WebSocket) — its per-connection task is still alive and
            //    still holds its own Arc<WorkerPool> clone via AppState.
            //    This is an EXPECTED, handled outcome of that timeout, not
            //    a bug.
            // 2. Something else entirely is still holding a clone — the
            //    ordering assumption for dispatch_handle/event_loop_handle/
            //    stats_tick_handle above no longer holds and needs
            //    re-investigating.
            // Either way, workers still get terminated via Drop
            // (JobObjectGuard's kill-on-drop) when this function returns,
            // just not gracefully — logged at error level so case 2 stays
            // visible, even though case 1 alone is not itself a defect.
            tracing::error!(
                "could not reclaim exclusive WorkerPool ownership for \
                 graceful shutdown_all() — an additional Arc<WorkerPool> \
                 clone is still alive (a still-open http connection past \
                 HTTP_DRAIN_TIMEOUT is one known, expected cause; anything \
                 else warrants investigation); workers will still be \
                 terminated via Drop (JobObjectGuard), just not gracefully"
            );
        }
    }
}
