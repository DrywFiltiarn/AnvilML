//! Model scanner + SQLite persistence. Never caches model file contents in memory.

pub mod db;

pub use db::create_pool;
