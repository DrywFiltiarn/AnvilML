//! IPC-specific error types for the `anvilml-ipc` crate.
//!
//! Defines the `IpcError` enum used by all IPC operations (bind, send, recv,
//! serialization, payload size, worker lookup). Every variant maps to
//! `AnvilError::Ipc(String)` for callers outside this crate.

use anvilml_core::AnvilError;

/// IPC-specific error enum covering the six failure modes of the transport layer.
///
/// Each variant carries the context needed to diagnose the failure (e.g. the address
/// that failed to bind, the payload size that exceeded the limit). All variants
/// convert to `AnvilError::Ipc(String)` via the `From` implementation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum IpcError {
    /// The bind operation failed (e.g. address already in use, permission denied).
    #[error("bind failed: {0}")]
    BindFailed(String),

    /// A send operation failed (e.g. connection closed, socket error).
    #[error("send failed: {0}")]
    SendFailed(String),

    /// A receive operation failed (e.g. timeout, malformed frame).
    #[error("recv failed: {0}")]
    RecvFailed(String),

    /// Serialization of the message payload failed (e.g. unsupported type).
    #[error("serialization failed: {0}")]
    SerializationFailed(String),

    /// The message payload exceeded the configured maximum size.
    ///
    /// `actual` is the size of the payload in bytes; `max` is the configured limit
    /// in bytes (derived from `max_ipc_payload_mib` in `ServerConfig`).
    #[error("payload too large: {actual} > {max}")]
    PayloadTooLarge { actual: usize, max: usize },

    /// A message referenced a worker that is not registered.
    ///
    /// The string identifies the unknown worker (e.g. `"gpu:3"`).
    #[error("unknown worker: {0}")]
    UnknownWorker(String),
}

impl From<IpcError> for AnvilError {
    fn from(err: IpcError) -> Self {
        // Map every IpcError variant to AnvilError::Ipc using the error's own
        // Display output. This gives callers outside anvilml-ipc a single domain-level
        // error type without needing to know about IPC-specific error details.
        AnvilError::Ipc(err.to_string())
    }
}
