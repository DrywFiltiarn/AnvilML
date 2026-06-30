mod cli;

use tracing_subscriber::EnvFilter;

use anvilml::shutdown;
use anvilml_core::CliOverrides;
use anvilml_core::config_load;
use anvilml_hardware::detect_all_devices;
use anvilml_registry::create_pool;
use anvilml_server::build_router;
use std::path::Path;
use std::time::Instant;
use tokio::net::TcpListener;

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
    let _pool = create_pool(&config.db_path)
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

    let loader = anvilml_registry::SeedLoader::new(_pool.clone());
    loader
        .run("devices.sql", &seed_path)
        .await
        .map_err(|e| {
            eprintln!("Failed to apply device capabilities seed: {e}");
            std::process::exit(1);
        })
        .unwrap();

    // Capture process-start instant once, before binding, so the health
    // handler returns a real elapsed-time measurement.
    let start_time = Instant::now();
    let router = build_router(start_time);
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port))
        .await
        .unwrap();
    tracing::info!(
        addr = %format!("{}:{}", config.host, config.port),
        "listening"
    );
    tokio::select! {
        _ = axum::serve(listener, router) => {},
        _ = shutdown::wait_for_shutdown_signal() => {
            tracing::info!("shutdown signal received");
        }
    }
}
