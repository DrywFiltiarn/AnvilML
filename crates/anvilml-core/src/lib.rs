//! anvilml-core — Pure domain types, config schema, error enum.
//! Zero I/O. Zero async. No tokio, no sqlx, no network.

mod config;
pub mod config_load;
mod error;

mod node_registry;
pub use node_registry::NodeTypeRegistry;

pub mod types;
pub use types::*;

pub use config::ServerConfig;
pub use config_load::CliOverrides;
pub use config_load::load;
pub use error::AnvilError;
