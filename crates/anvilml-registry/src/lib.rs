//! Model directory scanning and SQLite persistence for AnvilML.
//!
//! This crate owns the model scanner (directory walk, `ModelMeta` derivation),
//! the model store (CRUD for `ModelMeta` in SQLite), the device capability store,
//! and the SHA256-gated SQL seed loader.
//!
//! **Hard constraints:** Never cache model file contents in memory.
//! All model metadata is persisted to SQLite and re-read on startup.

pub mod db;
pub mod device_store;
pub mod scanner;
pub mod seed_loader;
pub mod store;

pub use db::{open, open_in_memory};
pub use device_store::{DeviceCapabilityStore, DeviceRow};
pub use scanner::ModelScanner;
pub use seed_loader::run;
pub use store::ModelStore;
