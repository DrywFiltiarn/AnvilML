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

use anvilml_hardware::detect_all_devices;
use anvilml_server::{build_router, AppState};
use std::sync::Arc;
use tokio::net::TcpListener;

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

    // Load the full configuration: defaults → TOML file → env vars → CLI overrides.
    // The config::load() function implements the four-level precedence chain.
    // If the TOML file does not exist, defaults are used silently.
    let cfg = config::load(&args.config, &overrides).expect("failed to load config");

    // Log the resolved bind address and port at INFO level (mandatory log point
    // per ENVIRONMENT.md §9 — Server subsystem, "config loaded" event).
    // This is the first operational log line after config resolution.
    tracing::info!(host = %cfg.host, port = %cfg.port, "config loaded");

    // Initialise the tracing subscriber based on the selected log format.
    // The tracing::Level::INFO filter matches the default log level from
    // ENVIRONMENT.md §3.1 (ANVILML_LOG defaults to `info`).
    // We use fmt::Subscriber for both formats — the json() builder method
    // on fmt::Subscriber is the standard approach and avoids an additional
    // dependency on a separate JSON crate.
    match args.log_format {
        cli::LogFormat::Plain => {
            tracing_subscriber::fmt::Subscriber::builder()
                .with_max_level(tracing::Level::INFO)
                .init();
        }
        cli::LogFormat::Json => {
            tracing_subscriber::fmt::Subscriber::builder()
                .with_max_level(tracing::Level::INFO)
                .json()
                .init();
        }
    }

    // Create an in-memory SQLite pool as a placeholder for the real database
    // connection (Phase 005 will wire the actual pool). This is sufficient
    // because detect_all_devices only needs a pool reference — it does not
    // execute SQL against it in Phase 004.
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory pool for hardware detection");

    // Detect all hardware devices at startup. The pool is a placeholder
    // (in-memory) until Phase 005 wires the real database connection.
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

    // Create shared application state with hardware detection results.
    // env!("CARGO_PKG_VERSION") is a compile-time literal that implements
    // Into<String>, matching AppState::new_with_hardware's parameter type.
    let state = AppState::new_with_hardware(
        env!("CARGO_PKG_VERSION"),
        Arc::new(tokio::sync::RwLock::new(hardware_info)),
    );

    // Build the axum router with all registered handlers wired to their routes.
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

    // Run the server until a fatal error occurs. The .expect() provides a
    // user-visible error message if the server encounters a fatal error during serving.
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown::shutdown_signal())
        .await
        .expect("server error");
}
