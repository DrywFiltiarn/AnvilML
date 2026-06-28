use clap::Parser;

/// Command-line interface for the AnvilML server.
///
/// Parses and validates all CLI arguments using clap derive macros.
/// Host and port defaults come from `ServerConfig::default()` via
/// `config_load::load()` (layer 1 of the four-layer config precedence),
/// not from clap defaults. The `--config` flag points to an optional
/// TOML file (layer 2).
#[derive(Parser, Debug)]
#[command(name = "anvilml", about = "AnvilML — ML model serving platform")]
pub struct Cli {
    /// Bind address for the HTTP server.
    ///
    /// If not provided, the value from the config precedence chain
    /// (defaults → TOML → env vars) is used.
    #[arg(long)]
    pub host: Option<String>,

    /// TCP port for the HTTP server.
    ///
    /// If not provided, the value from the config precedence chain
    /// (defaults → TOML → env vars) is used.
    #[arg(long)]
    pub port: Option<u16>,

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
