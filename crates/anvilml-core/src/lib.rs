//! anvilml-core — Pure domain types, config schema, error enum.
//! Zero I/O. Zero async. No tokio, no sqlx, no network.

mod error;

pub use error::AnvilError;
