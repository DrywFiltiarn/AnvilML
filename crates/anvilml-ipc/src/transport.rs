//! The ZeroMQ ROUTER socket transport wrapper.
//!
//! Provides `RouterTransport` — an `Arc`-shareable wrapper around a ZeroMQ ROUTER socket
//! whose send and receive halves are split into independent `tokio::sync::Mutex` guards
//! at construction time. This is the structural fix for a v3 shutdown deadlock where a
//! blocked `recv()` held the same lock a concurrent `send()` needed.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::Mutex;
use zeromq::prelude::*;
use zeromq::{Endpoint, RouterRecvHalf, RouterSendHalf, RouterSocket, ZmqMessage};

use crate::IpcError;
use crate::messages::{WorkerEvent, WorkerMessage};

/// The Rust-side ZeroMQ ROUTER socket wrapper.
///
/// Binds on construction. Ownership rule: constructed exactly once by `WorkerPool`
/// and shared via `Arc<RouterTransport>`. No other code holds the socket directly.
///
/// The send and receive halves are split into independent `tokio::sync::Mutex` guards
/// at construction time — this is the fix for a v3 shutdown deadlock where a blocked
/// `recv()` held the same lock a concurrent `send()` needed.
pub struct RouterTransport {
    /// The send half of the ROUTER socket, protected by its own `Arc<Mutex<>>`.
    ///
    /// This is a separate mutex from `receiver` so that a blocked `recv()` on the
    /// receive half cannot prevent `send()` from acquiring the lock — the structural
    /// fix for the v3 shutdown deadlock.
    sender: Arc<Mutex<RouterSendHalf>>,

    /// The receive half of the ROUTER socket, protected by its own `Arc<Mutex<>>`.
    ///
    /// This is a separate mutex from `sender` so that a blocked `send()` on the
    /// send half cannot prevent `recv()` from acquiring the lock.
    receiver: Arc<Mutex<RouterRecvHalf>>,

    /// The TCP port the ROUTER socket is bound on.
    ///
    /// Set by `bind()` when the OS assigns the port from `tcp://127.0.0.1:0`.
    /// Workers use this port to connect via `tcp://127.0.0.1:{port}` using their
    /// worker_id as the ZeroMQ identity.
    pub port: u16,
}

impl RouterTransport {
    /// Bind a ROUTER socket on `tcp://127.0.0.1:0` (OS-assigned port),
    /// split into independent send/recv halves, and return the transport.
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

        // Split the socket into independent send/recv halves.
        // split(self: Self) consumes the original socket and returns
        // (RouterSendHalf, RouterRecvHalf). This is the structural fix
        // for the v3 shutdown deadlock — each half is wrapped in its own
        // Arc<Mutex<>> so concurrent send and recv never contend on the same lock.
        let (send_half, recv_half) = socket.split();

        Ok(RouterTransport {
            sender: Arc::new(Mutex::new(send_half)),
            receiver: Arc::new(Mutex::new(recv_half)),
            port,
        })
    }

    /// Send a `WorkerMessage` to a worker identified by `worker_id`.
    ///
    /// Serializes the message via msgpack (`rmp_serde::to_vec_named`), builds a
    /// 3-frame ZeroMQ ROUTER multipart message (`[worker_id, "", payload]`), and
    /// sends it over the locked send half.
    ///
    /// This method acquires only `self.sender` — it never touches `self.receiver`,
    /// which is the structural fix for the v3 shutdown deadlock.
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

        // Acquire only the sender lock — never touches receiver.
        // This is the structural deadlock fix: recv() holds a separate lock.
        let mut send_half = self.sender.lock().await;

        // Send the 3-frame message over the ROUTER socket. The SocketSend trait
        // is provided by zeromq::prelude::* and implemented on RouterSendHalf.
        send_half
            .send(message)
            .await
            .map_err(|e| IpcError::SendFailed(e.to_string()))?;

        tracing::debug!(worker_id = %worker_id, "message sent");
        Ok(())
    }

    /// Send raw bytes to a worker identified by `worker_id`.
    ///
    /// Builds a 3-frame ZeroMQ ROUTER multipart message (`[worker_id, "", payload]`)
    /// and sends it over the locked send half. This is used by tests to send
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

        // Acquire only the sender lock — never touches receiver.
        let mut send_half = self.sender.lock().await;

        // Send the 3-frame message over the ROUTER socket.
        send_half
            .send(message)
            .await
            .map_err(|e| IpcError::SendFailed(e.to_string()))?;

        tracing::debug!(worker_id = %worker_id, "raw message sent");
        Ok(())
    }

    /// Receive a `WorkerEvent` from a worker, returning its identity and the event.
    ///
    /// Receives a 3-frame ROUTER multipart message, validates the frame count,
    /// extracts the worker identity (frame 0) and payload (frame 2), and
    /// deserializes the payload via msgpack into a `WorkerEvent`.
    ///
    /// This method acquires only `self.receiver` — it never touches `self.sender`,
    /// which is the structural fix for the v3 shutdown deadlock.
    ///
    /// # Errors
    ///
    /// Returns `IpcError::RecvFailed` if the socket receive fails, the frame count
    /// is not exactly 3, the identity frame is not valid UTF-8, or the payload
    /// fails msgpack deserialization.
    #[tracing::instrument(skip(self))]
    pub async fn recv(&self) -> Result<(String, WorkerEvent), IpcError> {
        // Acquire only the receiver lock — never touches sender.
        // This is the structural deadlock fix: send() holds a separate lock.
        let mut recv_half = self.receiver.lock().await;

        // Receive a 3-frame ROUTER multipart message. The SocketRecv trait
        // is provided by zeromq::prelude::* and implemented on RouterRecvHalf.
        let message = recv_half
            .recv()
            .await
            .map_err(|e| IpcError::RecvFailed(e.to_string()))?;

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
