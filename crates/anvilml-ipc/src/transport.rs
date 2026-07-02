//! The ZeroMQ ROUTER socket transport wrapper.
//!
//! Provides `RouterTransport` — an `Arc`-shareable wrapper around a ZeroMQ ROUTER socket.
//! The socket is stored behind a mutex so that `send()` and `recv()` can both access it.
//!
//! The transport can be closed via `close()`, which causes the next `recv()` to return
//! an error — this is used by tests to exercise the worker crash path. `close()`'s
//! signal is delivered via a `watch` channel, deliberately independent of the socket's
//! own mutex — see `close()`'s doc comment for why.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::{Mutex, watch};
use zeromq::prelude::*;
use zeromq::{Endpoint, RouterSocket, ZmqMessage};

use crate::IpcError;
use crate::messages::{WorkerEvent, WorkerMessage};

/// The Rust-side ZeroMQ ROUTER socket wrapper.
///
/// Binds on construction. Ownership rule: constructed exactly once by `WorkerPool`
/// and shared via `Arc<RouterTransport>`. No other code holds the socket directly.
///
/// The socket is stored behind an `Arc<Mutex<Option<RouterSocket>>>`. The `close()`
/// method replaces the socket with `None`, causing the next `recv()` to return an error.
pub struct RouterTransport {
    /// The ROUTER socket, protected by a mutex.
    ///
    /// Stored in an `Option` so that `close()` can replace it with `None`,
    /// which drops the socket and causes the next `recv()` to fail.
    socket: Arc<Mutex<Option<RouterSocket>>>,

    /// Closed signal, deliberately independent of `socket`'s mutex.
    ///
    /// `recv()` holds `socket`'s lock for the entire duration of the blocking
    /// inner socket read (it needs `&mut RouterSocket` for that whole span).
    /// If `close()` used the same lock to signal closure, it would block
    /// forever whenever a `recv()` call is genuinely in flight and waiting for
    /// a network message that will never arrive - the lock would never be
    /// released and `close()` could never acquire it. This `watch` channel
    /// lets `close()` signal instantly, lock-free, and lets `recv()` race that
    /// signal against the real socket read via `tokio::select!`, so a blocked
    /// `recv()` is interrupted rather than permanently starving `close()`.
    closed_tx: watch::Sender<bool>,

    /// The TCP port the ROUTER socket is bound on.
    ///
    /// Set by `bind()` when the OS assigns the port from `tcp://127.0.0.1:0`.
    /// Workers use this port to connect via `tcp://127.0.0.1:{port}` using their
    /// worker_id as the ZeroMQ identity.
    pub port: u16,
}

impl RouterTransport {
    /// Bind a ROUTER socket on `tcp://127.0.0.1:0` (OS-assigned port)
    /// and return the transport.
    ///
    /// The socket is bound on the loopback interface only — workers connect
    /// via `tcp://127.0.0.1:{port}` using their worker_id as the ZeroMQ identity.
    ///
    /// # Errors
    ///
    /// Returns `IpcError::BindFailed` if the bind operation fails (e.g. address
    /// already in use, permission denied) or if the returned endpoint is not a
    /// TCP endpoint (which would indicate a zeromq crate regression).
    pub async fn bind() -> Result<Self, IpcError> {
        // Create a new ROUTER socket. RouterSocket::new() is a synchronous
        // constructor that produces an unbound socket ready for bind().
        let mut socket = RouterSocket::new();

        // Bind to tcp://127.0.0.1:0 — the OS assigns an available port.
        // The bind() method is provided by the zeromq::Socket trait, which
        // RouterSocket implements. It returns ZmqResult<Endpoint>.
        let endpoint = socket
            .bind("tcp://127.0.0.1:0")
            .await
            .map_err(|e| IpcError::BindFailed(e.to_string()))?;

        // Extract the port number from the returned Endpoint.
        // The endpoint is Tcp(Host, u16) — pattern match to get the port.
        // We expect Tcp because we bound to a tcp:// URL; Ipc would only
        // appear if we bound to an ipc:// URL.
        let port = match endpoint {
            Endpoint::Tcp(_, p) => p,
            _ => {
                return Err(IpcError::BindFailed(format!(
                    "unexpected endpoint type: {endpoint:?}"
                )));
            }
        };

        Ok(RouterTransport {
            socket: Arc::new(Mutex::new(Some(socket))),
            closed_tx: watch::channel(false).0,
            port,
        })
    }

    /// Send a `WorkerMessage` to a worker identified by `worker_id`.
    ///
    /// Serializes the message via msgpack (`rmp_serde::to_vec_named`), builds a
    /// 3-frame ZeroMQ ROUTER multipart message (`[worker_id, "", payload]`), and
    /// sends it over the ROUTER socket.
    ///
    /// # Errors
    ///
    /// Returns `IpcError::SerializationFailed` if msgpack serialization fails, or
    /// `IpcError::SendFailed` if the socket send operation fails.
    #[tracing::instrument(skip(self, msg), fields(worker_id = %worker_id))]
    pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), IpcError> {
        // Serialize the message to msgpack bytes. to_vec_named produces a flat
        // dict with a "_type" discriminator, matching the Python msgpack decoder.
        let payload = rmp_serde::to_vec_named(msg)
            .map_err(|e| IpcError::SerializationFailed(e.to_string()))?;

        // Build a 3-frame ROUTER multipart message:
        //   Frame 0: worker_id (identity — tells ROUTER which DEALER to route to)
        //   Frame 1: empty delimiter (ROUTER protocol marker)
        //   Frame 2: msgpack payload (the actual message)
        //
        // ZmqMessage::from(worker_id) creates a 1-frame message with worker_id
        // as frame 0. Then push_back adds frames to the back.
        let mut message = ZmqMessage::from(worker_id);
        message.push_back(Bytes::from("")); // frame 1: empty delimiter
        message.push_back(Bytes::from(payload)); // frame 2: payload

        // Acquire the socket lock and send the message.
        let mut socket = self.socket.lock().await;
        let socket = socket
            .as_mut()
            .ok_or_else(|| IpcError::SendFailed("transport is closed".to_string()))?;

        // Send the 3-frame message over the ROUTER socket. The SocketSend trait
        // is provided by zeromq::prelude::* and implemented on RouterSocket.
        socket
            .send(message)
            .await
            .map_err(|e| IpcError::SendFailed(e.to_string()))?;

        tracing::debug!(worker_id = %worker_id, "message sent");
        Ok(())
    }

    /// Send raw bytes to a worker identified by `worker_id`.
    ///
    /// Builds a 3-frame ZeroMQ ROUTER multipart message (`[worker_id, "", payload]`)
    /// and sends it over the ROUTER socket. This is used by tests to send
    /// `WorkerEvent` payloads directly without going through `WorkerMessage` serialization.
    ///
    /// # Arguments
    ///
    /// * `worker_id` — The worker identity to route to.
    /// * `payload` — Raw msgpack bytes to send as frame 2.
    ///
    /// # Errors
    ///
    /// Returns `IpcError::SendFailed` if the socket send operation fails.
    pub async fn send_raw(&self, worker_id: &str, payload: &[u8]) -> Result<(), IpcError> {
        // Build a 3-frame ROUTER multipart message:
        //   Frame 0: worker_id (identity)
        //   Frame 1: empty delimiter
        //   Frame 2: raw payload bytes
        let mut message = ZmqMessage::from(worker_id);
        message.push_back(Bytes::from("")); // frame 1: empty delimiter
        // copy_from_slice copies the bytes into a new Bytes allocation (static lifetime).
        message.push_back(Bytes::copy_from_slice(payload)); // frame 2: payload

        // Acquire the socket lock and send the message.
        let mut socket = self.socket.lock().await;
        let socket = socket
            .as_mut()
            .ok_or_else(|| IpcError::SendFailed("transport is closed".to_string()))?;

        // Send the 3-frame message over the ROUTER socket.
        socket
            .send(message)
            .await
            .map_err(|e| IpcError::SendFailed(e.to_string()))?;

        tracing::debug!(worker_id = %worker_id, "raw message sent");
        Ok(())
    }

    /// Close the transport, causing the next `recv()` to return an error.
    ///
    /// This is used by tests to exercise the worker crash path (transport
    /// recv error).
    ///
    /// Signals via the lock-free `closed_tx` watch channel *first*, before
    /// touching `socket`'s mutex. This ordering matters: if a `recv()` call is
    /// currently in flight, holding `socket`'s lock while blocked on a network
    /// read that will never arrive, this method's own attempt to acquire that
    /// same lock (below) would otherwise block forever. Signaling first
    /// guarantees the in-flight `recv()`'s `tokio::select!` (see `recv()`)
    /// observes the signal and returns promptly, releasing the lock, before
    /// this method's own lock acquisition is reached.
    pub async fn close(&self) {
        // Signal first (lock-free) — see the doc comment above for why this
        // ordering is required, not just convenient.
        let _ = self.closed_tx.send(true);

        // Now replace the socket with None. By this point any in-flight
        // recv() has already been signaled and will release the lock on its
        // own; this acquisition is not competing with a recv() that can never
        // finish on its own.
        let mut socket = self.socket.lock().await;
        *socket = None;
    }

    /// Receive a `WorkerEvent` from a worker, returning its identity and the event.
    ///
    /// Receives a 3-frame ROUTER multipart message, validates the frame count,
    /// extracts the worker identity (frame 0) and payload (frame 2), and
    /// deserializes the payload via msgpack into a `WorkerEvent`.
    ///
    /// Races the blocking socket read against `closed_tx`'s signal via
    /// `tokio::select!` — see `close()`'s doc comment for why this is required
    /// rather than merely checking `socket.is_none()` before starting the read.
    /// If the close signal wins, the losing `socket.recv()` future (and the
    /// `MutexGuard` borrowed through it) is dropped by `select!`, releasing the
    /// lock for `close()`'s own pending acquisition — this is the mechanism
    /// that actually breaks the standoff, not merely detects it.
    ///
    /// # Errors
    ///
    /// Returns `IpcError::RecvFailed` if the transport is already closed, the
    /// socket receive fails or is interrupted by `close()`, the frame count is
    /// not exactly 3, the identity frame is not valid UTF-8, or the payload
    /// fails msgpack deserialization.
    #[tracing::instrument(skip(self))]
    pub async fn recv(&self) -> Result<(String, WorkerEvent), IpcError> {
        // Fast path: already closed. Avoids acquiring the socket lock at all
        // for any recv() call made after close() has already completed.
        if *self.closed_tx.subscribe().borrow() {
            return Err(IpcError::RecvFailed("transport is closed".to_string()));
        }

        let mut closed_rx = self.closed_tx.subscribe();
        let mut socket_guard = self.socket.lock().await;
        let socket = socket_guard
            .as_mut()
            .ok_or_else(|| IpcError::RecvFailed("transport is closed".to_string()))?;

        // Receive a 3-frame ROUTER multipart message, or bail out if close()
        // signals while this read is still pending. The SocketRecv trait is
        // provided by zeromq::prelude::* and implemented on RouterSocket.
        let message = tokio::select! {
            result = socket.recv() => {
                result.map_err(|e| IpcError::RecvFailed(e.to_string()))?
            }
            _ = closed_rx.changed() => {
                return Err(IpcError::RecvFailed("transport is closed".to_string()));
            }
        };

        // Convert the message into individual frames.
        // ROUTER always returns: [identity, delimiter, payload].
        let frames = message.into_vec();

        // Validate frame count — ROUTER multipart messages must have exactly 3
        // frames: worker identity, empty delimiter, msgpack payload.
        // A wrong count indicates a protocol violation or a partial message.
        if frames.len() != 3 {
            return Err(IpcError::RecvFailed(format!(
                "expected 3 frames, got {}",
                frames.len()
            )));
        }

        // Extract the worker identity from frame 0.
        // This is the string the worker registered with as its ZeroMQ DEALER identity.
        let identity = String::from_utf8(frames[0].to_vec())
            .map_err(|e| IpcError::RecvFailed(format!("invalid UTF-8 identity: {e}")))?;

        // Extract the msgpack payload from frame 2, skipping frame 1 (empty delimiter).
        // The delimiter is a ROUTER protocol marker with no semantic information.
        let payload = &frames[2];

        // Deserialize the msgpack payload into a WorkerEvent.
        // from_slice reads the flat dict and dispatches on the "_type" field
        // to construct the correct enum variant.
        let event = rmp_serde::from_slice(payload)
            .map_err(|e| IpcError::RecvFailed(format!("deserialization failed: {e}")))?;

        tracing::debug!(worker_id = %identity, event_type = ?event, "message received");
        Ok((identity, event))
    }
}
