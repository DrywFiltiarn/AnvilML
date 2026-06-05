//! AnvilML Registry — SQLite-backed job / model / artifact persistence layer.

pub mod db;
pub mod device_store;
pub mod scanner;
pub mod store;

pub use db::open;
pub use device_store::{DeviceCapabilityRow, DeviceCapabilityStore};
pub use scanner::scan_dirs;
pub use store::ModelRegistry;
