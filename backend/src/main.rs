mod cli;

use anvilml::shutdown;
use anvilml_server::build_router;
use tokio::net::TcpListener;

/// Entry point for the AnvilML server binary.
///
/// Parses CLI arguments, builds the HTTP router, binds a TCP listener
/// on the configured host and port, then serves HTTP requests until a
/// shutdown signal (Ctrl+C / SIGINT) is received.
#[tokio::main]
async fn main() {
    let cli = cli::parse();
    let router = build_router();
    let listener = TcpListener::bind(format!("{}:{}", cli.host, cli.port))
        .await
        .unwrap();
    tracing::info!(addr = %format!("{}:{}", cli.host, cli.port), "listening");
    tokio::select! {
        _ = axum::serve(listener, router) => {},
        _ = shutdown::wait_for_shutdown_signal() => {
            tracing::info!("shutdown signal received");
        }
    }
}
