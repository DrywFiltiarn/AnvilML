//! AnvilML Registry — SQLite-backed job / model / artifact persistence layer.

pub mod db;
pub mod scanner;

pub use db::open;
pub use scanner::scan_dirs;
