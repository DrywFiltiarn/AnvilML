use clap::{Parser, Subcommand};

/// Subcommands for the AnvilML binary.
///
/// Each variant represents a distinct operation the binary can perform
/// (currently only hardware probing; the default `None` path runs the server).
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Probe the system hardware and print detected devices as JSON.
    HwProbe,
}

/// Command-line interface for the AnvilML server.
///
/// Parses and validates all CLI arguments using clap derive macros.
/// Host and port defaults come from `ServerConfig::default()` via
/// `config_load::load()` (layer 1 of the four-layer config precedence),
/// not from clap defaults. The `--config` flag points to an optional
/// TOML file (layer 2).
///
/// The optional `command` subcommand field enables non-server operations
/// (e.g., `hw-probe`) without requiring separate binary targets.
#[derive(Parser, Debug)]
#[command(name = "anvilml", about = "AnvilML — ML model serving platform")]
pub struct Cli {
    /// Subcommand to execute.
    ///
    /// If `None` (the default), the binary runs the HTTP server.
    /// If `Some(Commands::HwProbe)`, the binary probes hardware and exits.
    #[command(subcommand)]
    pub command: Option<Commands>,

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

    /// Log output format: "plain" for human-readable text or "json" for
    /// newline-delimited JSON.
    ///
    /// Defaults to "plain". Any other value causes clap to exit with usage
    /// information and exit code 2, matching the existing CLI error convention.
    #[arg(long, default_value = "plain", value_parser = validate_log_format)]
    pub log_format: String,
}

/// Validate that the log format string is one of the supported values.
///
/// Returns the input string unchanged if it is "plain" or "json";
/// otherwise returns an error, which clap converts to an exit-with-code-2 message.
fn validate_log_format(s: &str) -> Result<String, String> {
    match s {
        // Both "plain" and "json" are valid — return the string unchanged.
        "plain" | "json" => Ok(s.to_owned()),
        // Invalid value: clap will print this error + usage and exit with code 2.
        other => Err(format!(
            "invalid log format '{other}': expected 'plain' or 'json'"
        )),
    }
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
