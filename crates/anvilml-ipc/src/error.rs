//! Transport-level errors for the ZeroMQ ROUTER socket.
//!
//! This module defines `TransportError`, the error type returned by
//! `RouterTransport` methods (`bind`, `send`), and `RecvError`, the error
//! type returned by `RouterTransport::recv()` and `recv_with_raw_identity()`
//! specifically. `RecvError` is separate from `TransportError` because only
//! one of its four cases (`SocketClosed`) is actually fatal to the
//! transport as a whole — the other three are per-message problems that a
//! caller reading messages in a loop (e.g. `anvilml-worker`'s demux task)
//! needs to be able to distinguish and recover from, rather than treating
//! every `recv()` failure as "the socket is gone". It is separate from
//! `IpcError` because transport errors (network-level, socket lifecycle)
//! are conceptually distinct from serialization errors (data-level,
//! msgpack encoding) — though `RecvError::DecodeFailed` wraps an `IpcError`
//! for the one case where a `recv()` failure *is* a serialization failure.

use thiserror::Error;
use zeromq::ZmqError;

use crate::IpcError;

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

/// Errors that can occur during [`RouterTransport::recv`](crate::RouterTransport::recv)
/// and [`recv_with_raw_identity`](crate::RouterTransport::recv_with_raw_identity).
///
/// **Only [`SocketClosed`](Self::SocketClosed) is fatal to the transport as
/// a whole.** The other three variants are per-message problems — a single
/// malformed or undecodable message from one peer — and do not mean the
/// underlying ZeroMQ socket has stopped working. A caller reading in a loop
/// (e.g. `anvilml-worker`'s demux task) should `break`/stop only on
/// `SocketClosed`, and log-and-continue on every other variant, so that one
/// bad message from one worker cannot silently stop event delivery for
/// every other worker sharing the same ROUTER socket.
///
/// Converts to [`AnvilError::Ipc`](anvilml_core::AnvilError::Ipc) via `From`
/// for callers that only need to propagate the failure (e.g. via `?`)
/// rather than distinguish its cause — this collapses back to the original,
/// flattened behaviour for anyone who doesn't specifically need the
/// distinction, while `recv()`'s own immediate callers can match on the
/// concrete variant instead.
#[derive(Debug, Error)]
pub enum RecvError {
    /// The underlying ZeroMQ socket itself has failed — connection lost,
    /// socket closed, or some other transport-level error from the
    /// `zeromq` crate. This is the one case that is genuinely fatal:
    /// nothing further will ever arrive on this socket.
    #[error("ROUTER recv failed: {0}")]
    SocketClosed(#[from] ZmqError),

    /// The ROUTER socket delivered a message with no identity frame — a
    /// single message violated the ROUTER protocol contract. The socket
    /// itself is still alive; this says nothing about any other message.
    #[error("ROUTER recv returned no identity frame")]
    MissingIdentityFrame,

    /// The ROUTER socket delivered a message with an identity frame but no
    /// payload frame — the message was incomplete. The socket itself is
    /// still alive; this says nothing about any other message.
    #[error("ROUTER recv returned no payload frame")]
    MissingPayloadFrame,

    /// The payload frame was present but could not be decoded into a
    /// `WorkerEvent` — e.g. the `_type` discriminator was missing or
    /// unrecognized, or a field had an unexpected shape. The socket itself
    /// is still alive; this says nothing about any other message.
    #[error("failed to decode event payload: {0}")]
    DecodeFailed(#[from] IpcError),
}

impl From<RecvError> for anvilml_core::AnvilError {
    /// Flattens `RecvError` back into `AnvilError::Ipc` for callers that
    /// only need to propagate the failure (typically via `?`) without
    /// distinguishing its cause — e.g. HTTP handlers, or test harnesses
    /// that treat any `recv()` failure as a hard error. Callers that do
    /// need the distinction (demux.rs) should match on `RecvError` itself,
    /// before this conversion ever runs.
    fn from(err: RecvError) -> Self {
        anvilml_core::AnvilError::Ipc(err.to_string())
    }
}
