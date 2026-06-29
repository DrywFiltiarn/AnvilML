//! Model scanner + SQLite persistence. Never caches model file contents in memory.

pub mod db;
pub mod store;

pub use db::create_pool;
pub use store::ModelStore;
