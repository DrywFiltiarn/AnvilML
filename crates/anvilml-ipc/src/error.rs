//! Transport-level errors for the ZeroMQ ROUTER socket.
//!
//! This module defines `TransportError`, the error type returned by
//! `RouterTransport` methods (`bind`, `send`). It is separate from `IpcError`
//! because transport errors (network-level, socket lifecycle) are conceptually
//! distinct from serialization errors (data-level, msgpack encoding).

use thiserror::Error;
use zeromq::ZmqError;

/// Errors that can occur during ZeroMQ ROUTER socket operations.
///
/// This error type is distinct from [`IpcError`](crate::IpcError) because it
/// represents transport-layer failures (socket binding, message routing) rather
/// than serialization failures.
#[derive(Debug, Error)]
pub enum TransportError {
    /// The ROUTER socket failed to bind to the requested address.
    ///
    /// This typically occurs when the port is already in use or the process
    /// lacks permission to bind to the address.
    #[error("failed to bind ROUTER socket: {0}")]
    Bind(String),

    /// A ZeroMQ socket error during message send or other socket operations.
    ///
    /// For `send()`, this includes `ZmqError::Other("Destination client not
    /// found by identity")` when the worker identity is not connected to the
    /// ROUTER socket.
    #[error("ZeroMQ socket error: {0}")]
    Zmq(#[from] ZmqError),

    /// Message encoding failed — the `WorkerMessage` could not be serialised
    /// to msgpack bytes for transport.
    #[error("failed to encode message for transport: {0}")]
    Encode(String),
}
