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
/// On Unix: waits for SIGINT (Ctrl-C) or SIGTERM.
/// On Windows: waits for Ctrl-C, Ctrl-CLOSE, or Ctrl-SHUTDOWN.
/// Uses `std::future::pending` for inactive-platform arms.
pub async fn shutdown_signal() {
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
}
