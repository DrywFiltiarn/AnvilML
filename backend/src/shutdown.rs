/// Await a cross-platform shutdown signal (Ctrl+C).
///
/// On Unix this receives SIGINT; on Windows this receives Ctrl+C.
/// tokio normalises both signals into a single awaitable future.
///
/// Full graceful shutdown (SIGTERM handling, worker drain sequence)
/// is implemented in a later phase (`ANVILML_DESIGN.md §19.3`).
pub async fn wait_for_shutdown_signal() {
    // Await Ctrl+C — tokio::signal::ctrl_c() returns () on success
    // or Err on signal-handler setup failure (extremely rare).
    // Discard the result at this stage; error handling is a later phase.
    let _ = tokio::signal::ctrl_c().await;
}
