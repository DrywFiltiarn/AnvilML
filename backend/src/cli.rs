use clap::Parser;

/// Command-line interface for the AnvilML server.
///
/// Parses and validates all CLI arguments using clap derive macros.
/// Default values match the compiled-in defaults in ServerConfig
/// so the binary works correctly with defaults before config loading.
#[derive(Parser, Debug)]
#[command(name = "anvilml", about = "AnvilML — ML model serving platform")]
pub struct Cli {
    /// Bind address for the HTTP server.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// TCP port for the HTTP server.
    #[arg(long, default_value = "8488")]
    pub port: u16,

    /// Path to the TOML configuration file.
    #[arg(long)]
    pub config: Option<String>,
}

/// Parse CLI arguments from the process environment.
///
/// Returns a `Cli` struct with all fields populated from
/// command-line flags or their compiled-in defaults.
///
/// On unrecognized flags or missing required arguments,
/// clap prints usage and exits the process.
pub fn parse() -> Cli {
    Cli::parse()
}
