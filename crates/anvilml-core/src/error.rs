//! Error types for `anvilml-core`.
//!
//! This module provides `AnvilError`, the unified error enum for the crate.
//! It covers I/O errors from file operations, TOML deserialisation failures,
//! and invalid environment variable values.
//!
//! This is a minimal version expanded from P3-B1 will add additional variants
//! (`Db`, `Serde`, `Ipc`, `PayloadTooLarge`, etc.) per `ANVILML_DESIGN.md §5.2`.

/// Errors that can occur during config loading and core operations.
///
/// This enum covers the three error categories needed by the config loading
/// pipeline: filesystem I/O, TOML deserialisation, and environment variable
/// parsing. Additional variants will be added in Phase 003 (P3-B1).
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    /// I/O error reading or writing a config file.
    ///
    /// Produced when `std::fs::read_to_string` or similar operations fail —
    /// e.g. the file exists but is not readable, or the path is a directory.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML deserialisation error.
    ///
    /// Produced when `toml::from_str` fails to parse a config file into
    /// `ServerConfig` — e.g. invalid TOML syntax or type mismatch on a field.
    #[error("TOML deserialisation error: {0}")]
    Toml(#[from] toml::de::Error),

    /// Invalid environment variable value.
    ///
    /// Produced when an `ANVILML_*` environment variable is set to a value
    /// that cannot be parsed into the expected type (e.g. `ANVILML_PORT=abc`).
    /// The `name` field identifies the variable; `value` holds the raw string.
    #[error("Invalid env var {name}: {value}")]
    EnvVar { name: String, value: String },
}
