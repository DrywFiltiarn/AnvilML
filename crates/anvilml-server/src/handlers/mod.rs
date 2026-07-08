//! HTTP request handlers for the AnvilML server.
//!
//! Each submodule implements a single handler function that processes
//! one or more HTTP routes registered by `build_router()` in the
//! parent `lib.rs`.

pub mod artifacts;
pub mod health;
pub mod jobs;
pub mod nodes;
