//! Model scanner + SQLite persistence. Never caches model file contents in memory.

pub mod db;
pub mod scanner;
pub mod store;

pub use db::create_pool;
pub use scanner::ModelScanner;
pub use store::ModelStore;
