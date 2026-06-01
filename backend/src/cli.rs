//! CLI argument parsing for the anvilml backend.
//!
//! Defines the `Args` struct and `LogFormat` enum used to parse command-line
//! arguments via clap, then resolve them into `ConfigOverrides` for the
//! config loader pipeline.

use std::net::IpAddr;
use std::path::PathBuf;

use anvilml_core::ConfigOverrides;
use clap::{Parser, ValueEnum};

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// Plain-text human-readable logs.
    #[default]
    Plain,
    /// Machine-parseable JSON logs.
    Json,
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain => write!(f, "plain"),
            Self::Json => write!(f, "json"),
        }
    }
}

impl ValueEnum for LogFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Plain, Self::Json]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        match self {
            Self::Plain => Some(clap::builder::PossibleValue::new("plain")),
            Self::Json => Some(clap::builder::PossibleValue::new("json")),
        }
    }
}

/// CLI argument parser.
///
/// Uses clap derive to define all supported flags with their default values
/// and help text. Call `parse()` to resolve from `std::env::args_os()`.
#[derive(Parser)]
#[command(name = "anvilml", about = "AnvilML server — configurable CLI")]
pub struct Args {
    /// Path to the TOML configuration file.
    #[arg(long, default_value = "./anvilml.toml")]
    pub config: PathBuf,

    /// Bind host address for the HTTP server.
    #[arg(long)]
    pub host: Option<IpAddr>,

    /// Bind port number for the HTTP server.
    #[arg(long)]
    pub port: Option<u16>,

    /// Do not open a browser window on startup.
    #[arg(long)]
    pub no_browser: bool,

    /// Log output format: "plain" or "json".
    #[arg(long, value_enum, default_value = "plain")]
    pub log_format: LogFormat,
}

impl Args {
    /// Resolve CLI fields into `ConfigOverrides`.
    ///
    /// Only `host` and `port` produce overrides; the remaining flags are
    /// returned directly from the struct.
    pub fn to_overrides(&self) -> ConfigOverrides {
        ConfigOverrides {
            host: self.host,
            port: self.port,
        }
    }
}

/// Parse CLI arguments from `std::env::args_os()` and return a fully
/// constructed `Args` instance.
pub fn parse() -> Args {
    Args::parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── LogFormat tests ────────────────────────────────────────────────

    #[test]
    fn test_log_format_default_is_plain() {
        assert_eq!(LogFormat::default(), LogFormat::Plain);
    }

    #[test]
    fn test_log_format_value_enum_variants() {
        let variants = <LogFormat as ValueEnum>::value_variants();
        assert_eq!(variants.len(), 2);
        assert!(variants.contains(&LogFormat::Plain));
        assert!(variants.contains(&LogFormat::Json));
    }

    #[test]
    fn test_log_format_to_string() {
        assert_eq!(format!("{}", LogFormat::Plain), "plain");
        assert_eq!(format!("{}", LogFormat::Json), "json");
    }

    #[test]
    fn test_log_format_possible_values() {
        let plain = LogFormat::Plain.to_possible_value();
        assert!(plain.is_some());
        assert_eq!(plain.unwrap().get_name(), "plain");

        let json = LogFormat::Json.to_possible_value();
        assert!(json.is_some());
        assert_eq!(json.unwrap().get_name(), "json");
    }

    // ── Args struct tests ──────────────────────────────────────────────

    #[test]
    fn test_args_to_overrides_all_none() {
        let args = Args {
            config: PathBuf::from("./anvilml.toml"),
            host: None,
            port: None,
            no_browser: false,
            log_format: LogFormat::Plain,
        };
        let overrides = args.to_overrides();
        assert!(overrides.host.is_none());
        assert!(overrides.port.is_none());
    }

    #[test]
    fn test_args_to_overrides_with_values() {
        use std::str::FromStr;
        let args = Args {
            config: PathBuf::from("/tmp/test.toml"),
            host: Some(IpAddr::from_str("0.0.0.0").unwrap()),
            port: Some(9090),
            no_browser: true,
            log_format: LogFormat::Json,
        };
        let overrides = args.to_overrides();
        assert_eq!(overrides.host, Some(IpAddr::from_str("0.0.0.0").unwrap()));
        assert_eq!(overrides.port, Some(9090));
    }

    #[test]
    fn test_args_to_overrides_ipv6() {
        let args = Args {
            config: PathBuf::from("./anvilml.toml"),
            host: Some("::1".parse().unwrap()),
            port: None,
            no_browser: false,
            log_format: LogFormat::Plain,
        };
        let overrides = args.to_overrides();
        assert_eq!(overrides.host, Some("::1".parse().unwrap()));
    }

    #[test]
    fn test_args_to_overrides_port_edge() {
        let args = Args {
            config: PathBuf::from("./anvilml.toml"),
            host: None,
            port: Some(1),
            no_browser: false,
            log_format: LogFormat::Plain,
        };
        let overrides = args.to_overrides();
        assert_eq!(overrides.port, Some(1));
    }
}
