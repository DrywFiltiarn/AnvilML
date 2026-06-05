//! AnvilML Registry — SQLite-backed job / model / artifact persistence layer.

pub mod db;
pub mod device_store;
pub mod scanner;
pub mod seed_loader;
pub mod store;

pub use db::{open, open_in_memory};
pub use device_store::{DeviceCapabilityRow, DeviceCapabilityStore};
pub use scanner::scan_dirs;
pub use seed_loader::run;
pub use sqlx::sqlite::SqlitePool;
pub use store::ModelRegistry;
