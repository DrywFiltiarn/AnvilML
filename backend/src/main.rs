//! AnvilML server binary — binds and serves the HTTP/WebSocket router.
//!
//! This binary is the entry point for the AnvilML server. It creates shared
//! application state, builds the axum router, binds a TCP listener on the
//! configured address, and runs the server until it is terminated.

use anvilml_server::{build_router, AppState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // Create shared application state with the workspace version string.
    // env!("CARGO_PKG_VERSION") is a compile-time literal that implements
    // Into<String>, matching AppState::new's `impl Into<String>` parameter.
    let state = AppState::new(env!("CARGO_PKG_VERSION"));

    // Build the axum router with all registered handlers wired to their routes.
    let router = build_router(state);

    // Bind a TCP listener on the configured address.
    // tokio::net::TcpListener::bind is async and must be awaited.
    let addr = "127.0.0.1:8488";
    let listener = TcpListener::bind(addr)
        .await
        .expect("failed to bind listener");

    // Log the bind address at INFO level (mandatory log point per ENVIRONMENT.md §9.2).
    tracing::info!(addr = %addr, "listening");

    // Run the server until a fatal error occurs. The .expect() provides a
    // user-visible error message if the server encounters a fatal error during serving.
    axum::serve(listener, router).await.expect("server error");
}
