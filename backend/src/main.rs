mod cli;

use anvilml_core::load_config;
use anvilml_server::{build_router, AppState};
use tracing_subscriber::fmt::layer as fmt_layer;
use tracing_subscriber::Layer;

#[tokio::main]
async fn main() {
    let args = cli::parse();

    // Initialise the tracing subscriber before any server logic.
    let env_filter = std::env::var("ANVILML_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());
    let filter = tracing_subscriber::EnvFilter::try_new(env_filter)
        .unwrap_or_else(|e| {
            eprintln!("Invalid RUST_LOG/ANVILML_LOG value: {e}, falling back to info");
            tracing_subscriber::EnvFilter::new("info")
        });

    // Build the formatter layer.  Boxing via `dyn Layer` unifies the
    // plain and JSON variants which have incompatible concrete types.
    let fmt_layer: Box<
        dyn Layer<tracing_subscriber::Registry> + Send + Sync,
    > = match args.log_format {
        cli::LogFormat::Plain => {
            Box::new(fmt_layer().with_filter(filter.clone()))
        }
        cli::LogFormat::Json => {
            Box::new(fmt_layer().json().with_filter(filter))
        }
    };

    use tracing_subscriber::prelude::*;
    let subscriber = tracing_subscriber::registry()
        .with(fmt_layer)
        .try_init();
    let _ = subscriber;

    let overrides = args.to_overrides();

    // Resolve config path from CLI or use the default.
    let toml_path = if args.config.as_os_str().is_empty() {
        None
    } else {
        Some(args.config.as_path())
    };

    let cfg = load_config(toml_path, overrides).expect("Failed to load config");

    let state = AppState::new(env!("CARGO_PKG_VERSION"));
    let router = build_router(state);

    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {bind_addr}: {e}"));

    tracing::info!("Listening on http://{bind_addr}");
    let _ = axum::serve(listener, router).await;
}
