mod cli;

/// Entry point for the AnvilML server binary.
///
/// Parses CLI arguments, prints a scaffold message to confirm
/// the binary is operational, and returns. Later phases convert
/// this to an async main and wire the HTTP server.
fn main() {
    let cli = cli::parse();
    // Print scaffold message to confirm binary is operational.
    println!("AnvilML scaffold");
    // TODO: Wire HTTP server and shutdown signal handler (P1-A3, P1-D1).
    let _ = cli;
}
