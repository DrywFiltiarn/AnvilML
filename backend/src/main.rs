mod cli;

use anvilml::shutdown;
use anvilml_core::CliOverrides;
use anvilml_core::config_load;
use anvilml_server::build_router;
use std::path::Path;
use tokio::net::TcpListener;

/// Entry point for the AnvilML server binary.
///
/// Parses CLI arguments, loads `ServerConfig` through the four-layer
/// precedence chain (defaults → TOML → env vars → CLI flags) via
/// `config_load::load()`, builds the HTTP router, binds a TCP listener
/// on the loaded host and port, then serves HTTP requests until a
/// shutdown signal (Ctrl+C / SIGINT) is received.
///
/// If config loading fails, prints the error and exits with code 1
/// before binding any socket.
#[tokio::main]
async fn main() {
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
