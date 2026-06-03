//! AnvilML Registry — SQLite-backed job / model / artifact persistence layer.

pub mod db;
pub mod scanner;
pub mod store;

pub use db::open;
pub use scanner::scan_dirs;
pub use store::ModelRegistry;
