//! anvilml-core — Pure domain types, config schema, error enum.
//! Zero I/O. Zero async. No tokio, no sqlx, no network.

mod config;
mod error;

pub use config::ServerConfig;
pub use error::AnvilError;
