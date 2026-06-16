//! EventBroadcaster re-export.
//!
//! `EventBroadcaster` is defined in `anvilml-ipc` to avoid a cyclic dependency:
//! `anvilml-worker` needs this type for `WorkerPool`, but `anvilml-server`
//! transitively depends on `anvilml-worker`. Placing it in `anvilml-ipc`
//! (which `anvilml-worker` already depends on) breaks the cycle.
//!
//! This module re-exports `EventBroadcaster` from `anvilml-ipc` for
//! backward compatibility with code that imports via `anvilml_server::ws`.

pub use anvilml_ipc::EventBroadcaster;
