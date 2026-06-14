//! Command-line argument parsing for the AnvilML server binary.
//!
//! Provides the `Args` struct derived from `clap::Parser` and the `LogFormat`
//! enum for selecting log output format. The `parse()` function is a thin
//! wrapper around `Args::parse()` that allows testing without consuming
//! `std::env::args()`.

use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;

/// CLI arguments for the AnvilML server.
///
/// Parsed from `std::env::args()` via `clap`. All fields are optional except
/// `--config` (which defaults to `./anvilml.toml`) and `--log-format`
/// (which defaults to `plain`).
#[derive(clap::Parser, Debug)]
#[clap(name = "anvilml", about = "AnvilML server")]
pub struct Args {
    /// Path to the TOML configuration file.
    ///
    /// The default matches the documented default in `ENVIRONMENT.md §4`.
    #[arg(long, default_value = "./anvilml.toml")]
    pub config: PathBuf,

    /// Optional bind address override.
    ///
    /// When provided, supersedes the `host` field from config files and
    /// environment variables. Clap validates `IpAddr` via its
    /// `ValueParserFactory` impl, rejecting invalid addresses at parse time.
    #[arg(long)]
    pub host: Option<IpAddr>,

    /// Optional bind port override.
    ///
    /// When provided, supersedes the `port` field from config files and
    /// environment variables.
    #[arg(long)]
    pub port: Option<u16>,

    /// Log output format.
    ///
    /// `plain` produces human-readable terminal output; `json` produces
    /// structured JSON lines suitable for log aggregation.
    #[arg(long, value_enum, default_value = "plain")]
    pub log_format: LogFormat,
}

/// Log output format selection.
///
/// Mapped to the `--log-format` CLI flag. Controls whether the tracing
/// subscriber produces plain text or structured JSON output.
#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum LogFormat {
    /// Plain text log output (human-readable terminal format).
    Plain,
    /// JSON log output (structured, machine-parseable lines).
    Json,
}

/// Parse CLI arguments from `std::env::args()`.
///
/// This is a thin wrapper around `Args::parse()` that provides a named
/// function matching the task specification and allows testing without
/// directly consuming `std::env::args()`.
pub fn parse() -> Args {
    Args::parse()
}
