mod cli;

use tracing_subscriber::EnvFilter;

use anvilml::shutdown;
use anvilml_core::CliOverrides;
use anvilml_core::config_load;
use anvilml_hardware::detect_all_devices;
use anvilml_server::build_router;
use std::path::Path;
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
    // Initialize the tracing subscriber as the very first startup step.
    // Reads filter from ANVILML_LOG (primary) or RUST_LOG (fallback),
    // defaulting to "info" when neither is set — matching the precedence
    // documented in ENVIRONMENT.md §3.3.
    // Write to stderr so tracing output does not mix with stdout data
    // (e.g. `hw-probe` JSON output goes to stdout, logs go to stderr).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("ANVILML_LOG")
                .or_else(|_| EnvFilter::try_from_env("RUST_LOG"))
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = cli::parse();

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

    let router = build_router();
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
