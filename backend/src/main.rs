mod cli;

use anvilml::shutdown;

/// Entry point for the AnvilML server binary.
///
/// Parses CLI arguments, prints a scaffold message to confirm
/// the binary is operational, then awaits a shutdown signal.
/// Converted to async main in P1-A3 to support tokio-based
/// signal handling and future HTTP server integration.
#[tokio::main]
async fn main() {
    let _cli = cli::parse();
    // Print scaffold message to confirm binary is operational.
    println!("AnvilML scaffold");
    // Await cross-platform shutdown signal (Ctrl+C / SIGINT).
    // Full graceful shutdown with worker drain is a later phase.
    shutdown::wait_for_shutdown_signal().await;
}
