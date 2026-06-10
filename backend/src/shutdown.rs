use std::sync::Arc;

use anvilml_registry::SqlitePool;
use anvilml_server::App;

#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
#[cfg(windows)]
use tokio::signal::windows::{ctrl_close, ctrl_shutdown};

/// Returns the SIGTERM / Ctrl-CLOSE / Ctrl-SHUTDOWN future for the current
/// platform, or a never-resolving future on unsupported targets.
#[cfg(unix)]
fn pending_or_terminate() -> impl std::future::Future<Output = ()> {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    async move {
        sigterm.recv().await;
    }
}

#[cfg(windows)]
fn pending_or_terminate() -> impl std::future::Future<Output = ()> {
    let mut ctrlclose = ctrl_close().unwrap();
    async move {
        ctrlclose.recv().await;
    }
}

/// Returns the Ctrl-SHUTDOWN future on Windows, or a never-resolving
/// future on other platforms.
#[cfg(windows)]
fn pending_or_ctrl_shutdown() -> impl std::future::Future<Output = ()> {
    let mut ctrlshutdown = ctrl_shutdown().unwrap();
    async move {
        ctrlshutdown.recv().await;
    }
}

#[cfg(not(windows))]
fn pending_or_ctrl_shutdown() -> impl std::future::Future<Output = ()> {
    std::future::pending()
}

/// Cross-platform async shutdown signal handler.
///
/// Waits for a termination signal (SIGINT, SIGTERM, Ctrl-C, Ctrl-CLOSE,
/// or Ctrl-SHUTDOWN), then performs a graceful shutdown sequence:
/// 1. Sets the shutdown flag on `AppState` to reject new job submissions.
/// 2. Drains all workers via `WorkerPool::shutdown_all`.
/// 3. Closes the SQLite connection pool (flushes WAL).
///
/// On Unix: waits for SIGINT (Ctrl-C) or SIGTERM.
/// On Windows: waits for Ctrl-C, Ctrl-CLOSE, or Ctrl-SHUTDOWN.
pub async fn shutdown_signal(state: Arc<App>, pool: SqlitePool) {
    tokio::select! {
        _ = pending_or_terminate() => {
            tracing::info!("Received termination signal, shutting down");
        }
        _ = pending_or_ctrl_shutdown() => {
            tracing::info!("Received Ctrl-SHUTDOWN, shutting down");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT (Ctrl-C), shutting down");
        }
        _ = std::future::pending::<()>() => {
            // Never resolves — keeps the select alive on unsupported
            // platforms.
        }
    }

    // 1. Set the shutdown flag to reject new submissions.
    state.set_shutdown();
    tracing::info!("submissions closed — rejecting new job submissions");

    // 2. Drain workers.
    if let Some(workers) = &state.workers {
        tracing::info!("draining workers");
        workers.shutdown_all().await;
        tracing::info!("all workers drained");
    } else {
        tracing::warn!("no worker pool configured, skipping drain");
    }

    // 3. Close the SQLx pool (flushes WAL).
    drop(pool);
    tracing::info!("database connection pool closed");

    tracing::info!("graceful shutdown complete");
}
