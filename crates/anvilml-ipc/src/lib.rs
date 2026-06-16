//! ZeroMQ ROUTER transport and message types for AnvilML worker IPC.
//!
//! This crate owns the `RouterTransport` (ZeroMQ ROUTER socket wrapper),
//! the `WorkerMessage` and `WorkerEvent` enums, and msgpack serialisation
//! via `rmp-serde`. It does not contain process management or business logic.
//!
//! **Hard constraints:** ZeroMQ routing is handled automatically — this crate
//! never manipulates identities or performs manual message routing.

pub mod messages;

pub use messages::{decode_event, encode_message, IpcError, WorkerEvent, WorkerMessage};
