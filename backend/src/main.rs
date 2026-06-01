mod cli;

use anvilml_core::load_config;
use anvilml_server::{build_router, AppState};

#[tokio::main]
async fn main() {
    let args = cli::parse();
    let overrides = args.to_overrides();
    let _log_format = args.log_format;

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

    println!("Listening on http://{bind_addr}");
    let _ = axum::serve(listener, router).await;
}
