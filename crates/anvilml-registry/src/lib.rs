//! Model directory scanning and SQLite persistence for AnvilML.
//!
//! This crate owns the model scanner (directory walk, `ModelMeta` derivation),
//! the model store (CRUD for `ModelMeta` in SQLite), the device capability store,
//! and the SHA256-gated SQL seed loader.
//!
//! **Hard constraints:** Never cache model file contents in memory.
//! All model metadata is persisted to SQLite and re-read on startup.

#[allow(dead_code)]
pub fn stub() {}
