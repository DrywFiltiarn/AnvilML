//! Cross-platform graceful shutdown signal handler.
//!
//! This module provides a single public function, `shutdown_signal`, that waits for
//! an OS-level shutdown signal and returns when one is received. On Unix systems
//! it listens for both SIGINT (Ctrl-C) and SIGTERM. On Windows it listens for
//! Ctrl-C, which is the closest equivalent to POSIX signals.
//!
//! The function is designed to be passed to `axum::serve().with_graceful_shutdown()`,
//! which calls `.await` on the returned future when the server is ready to shut down.

/// Wait for a shutdown signal (SIGINT or SIGTERM on Unix, Ctrl-C on Windows)
/// and return when one is received.
///
/// This function is intended to be used with `axum::serve().with_graceful_shutdown()`.
/// It races two signal streams on Unix (SIGINT and SIGTERM) using `tokio::select!`,
/// returning immediately on whichever signal arrives first. On Windows it waits
/// for Ctrl-C only.
///
/// On signal receipt, logs `tracing::info!("shutdown signal received")` and returns,
/// allowing the server to begin graceful shutdown.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        // Race SIGINT and SIGTERM via tokio::select!. Both signals trigger
        // shutdown independently — receiving one must not block waiting for
        // the other. tokio::select! returns on the first ready arm, which
        // is the correct behaviour for signal handling: the server should
        // start shutting down as soon as any recognized signal arrives.
        use tokio::signal::unix::{signal, SignalKind};

        // Signal registration on SIGINT/SIGTERM should never fail in normal
        // operation — these signals are never blocked or forbidden. If it
        // does fail, the process has a fundamentally broken signal setup
        // and panicking is the appropriate response.
        let mut sigint =
            signal(SignalKind::interrupt()).expect("failed to register SIGINT signal handler");
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to register SIGTERM signal handler");

        tokio::select! {
            _ = sigint.recv() => {
                tracing::info!("shutdown signal received");
            }
            _ = sigterm.recv() => {
                tracing::info!("shutdown signal received");
            }
        }
    }

    #[cfg(windows)]
    // Windows does not support POSIX signals (SIGINT/SIGTERM). The closest
    // equivalent is Ctrl-C, which tokio provides via signal::ctrl_c().
    // This is sufficient for local development and for console-hosted
    // Windows services that receive Ctrl-C from the terminal.
    {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to set up Ctrl-C handler");
        tracing::info!("shutdown signal received");
    }
}
