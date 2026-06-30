//! ZeroMQ ROUTER transport + message types. No process management.

pub mod error;
pub mod ws;

pub use error::IpcError;
pub use ws::broadcaster::EventBroadcaster;
