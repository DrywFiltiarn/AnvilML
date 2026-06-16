//! ZeroMQ ROUTER socket transport for AnvilML worker IPC.
//!
//! This module provides `RouterTransport`, a wrapper around a tokio-async
//! ZeroMQ ROUTER socket. The ROUTER socket is the server-side socket type
//! in ZeroMQ — it accepts connections from clients (DEALER sockets on the
//! worker side) and routes messages to them by identity.
//!
//! **Routing contract:** The ROUTER socket automatically routes messages
//! to the correct peer based on the identity frame. The caller must provide
//! the worker identity as the first frame of every `send()` call; the socket
//! pops it internally and uses it to look up the connected peer.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::Mutex;
use tracing;
use zeromq::{Endpoint, RouterSocket, Socket, SocketRecv, SocketSend, ZmqMessage};

/// The socket type used internally by `RouterTransport`.
///
/// `Arc<Mutex<RouterSocket>>` allows the transport to be cloned and shared
/// across async tasks while keeping the underlying socket access serialised.
type InnerSocket = Arc<Mutex<RouterSocket>>;

use crate::TransportError;
use crate::{decode_event, encode_message, WorkerEvent, WorkerMessage};
use anvilml_core::AnvilError;

/// A ZeroMQ ROUTER socket wrapper for sending messages to AnvilML workers.
///
/// The ROUTER socket is the server-side socket type in ZeroMQ. It accepts
/// connections from worker DEALER sockets and routes messages to them by
/// identity.
///
/// # Thread safety
///
/// The inner socket is protected by `tokio::sync::Mutex` (not `std::sync::Mutex`)
/// because all socket operations are async. The `Arc` wrapper allows the transport
/// to be cloned and shared across async tasks.
///
/// # Examples
///
/// ```no_run
/// use anvilml_ipc::{RouterTransport, WorkerMessage};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let transport = RouterTransport::bind().await?;
/// println!("Bound to port {}", transport.port);
///
/// // The worker_id is a byte slice — the identity frame used by
/// // the ZeroMQ ROUTER socket for routing. For human workers
/// // (e.g., from environment variables), use the worker ID string
/// // as bytes. For auto-generated identities (from DEALER sockets),
/// // use the raw identity bytes discovered via ROUTER::recv.
/// transport
///     .send(b"worker-0", &WorkerMessage::Ping { seq: 1 })
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct RouterTransport {
    /// The underlying ZeroMQ ROUTER socket, protected by a tokio async mutex
    /// inside an `Arc` for shared ownership across async tasks.
    ///
    /// `Arc<Mutex<RouterSocket>>` allows the transport to be cloned and shared
    /// across async tasks while keeping the underlying socket access serialised.
    /// `tokio::sync::Mutex` is required because all socket methods (`bind`,
    /// `send`, `recv`) are async and must be awaitable without blocking the
    /// tokio runtime thread.
    ///
    /// The underlying ZeroMQ ROUTER socket wrapped in `Arc<Mutex<>>`.
    ///
    /// This field is `pub` to allow integration tests to discover the DEALER
    /// socket's auto-generated identity via `recv`. In production, the `send()`
    /// method is the only public interface.
    pub socket: InnerSocket,

    /// The TCP port the ROUTER socket is bound to.
    ///
    /// This is set by `bind()` when the socket binds to `tcp://127.0.0.1:0`,
    /// which causes the OS to assign an available port. The port number is
    /// extracted from the returned `Endpoint::Tcp(_, port)` and stored here
    /// for the caller (e.g., to pass to workers via environment variables).
    pub port: u16,
}

impl RouterTransport {
    /// Create a new ROUTER socket and bind it to `tcp://127.0.0.1:0`.
    ///
    /// Binding to port 0 lets the OS pick an available port, which avoids
    /// port conflicts when multiple server instances run concurrently.
    /// The assigned port is stored in `self.port` for later use.
    ///
    /// # Errors
    ///
    /// Returns `TransportError::Bind` if the socket cannot bind to the
    /// requested address (e.g., port already in use, permission denied).
    ///
    /// # Logging
    ///
    /// Logs at INFO level: `tracing::info!(port = %port, "ROUTER socket bound")`.
    pub async fn bind() -> Result<Self, TransportError> {
        let mut socket = RouterSocket::new();

        // Bind to port 0 so the OS assigns an available port, avoiding
        // conflicts when multiple server instances run concurrently.
        let endpoint = socket
            .bind("tcp://127.0.0.1:0")
            .await
            .map_err(|e| TransportError::Bind(e.to_string()))?;

        // Extract the OS-assigned port from the TCP endpoint.
        // The bind address "tcp://127.0.0.1:0" guarantees a Tcp variant,
        // but we pattern-match explicitly for safety.
        let port = match &endpoint {
            Endpoint::Tcp(_, port) => *port,
            // The #[non_exhaustive] attribute requires a wildcard arm.
            // In practice, only Tcp and Ipc variants exist.
            _ => {
                return Err(TransportError::Bind(
                    "bind returned non-TCP endpoint (expected Tcp)".to_string(),
                ));
            }
        };

        tracing::info!(port = %port, "ROUTER socket bound");

        // Wrap the socket in Arc<Mutex<>> so it can be shared across async
        // tasks. The Arc allows cloning the transport, and the Mutex ensures
        // only one task accesses the socket at a time.
        Ok(Self {
            socket: Arc::new(Mutex::new(socket)),
            port,
        })
    }

    /// Send a `WorkerMessage` to a worker identified by `worker_id`.
    ///
    /// The message is encoded to msgpack bytes via `encode_message()`, then
    /// wrapped in a ZeroMQ multipart message with two frames:
    /// 1. The worker identity (first frame, popped internally by ROUTER for routing)
    /// 2. The encoded message payload (second frame, delivered to the worker)
    ///
    /// # Errors
    ///
    /// Returns `TransportError::Encode` if message encoding fails.
    /// Returns `TransportError::Zmq` if the socket encounters an error,
    /// including `ZmqError::Other("Destination client not found by identity")`
    /// when no worker with the given identity is connected.
    ///
    /// # Logging
    ///
    /// Logs at DEBUG level: `tracing::debug!(worker_id = %worker_id, "message sent to worker")`.
    pub async fn send(&self, worker_id: &[u8], msg: &WorkerMessage) -> Result<(), TransportError> {
        // Encode the message to msgpack bytes. This is the data-level
        // serialization step; failures here indicate a bug in the message
        // type definitions or a corrupted message value.
        let encoded = encode_message(msg).map_err(|e| TransportError::Encode(e.to_string()))?;

        // Construct a ZeroMQ multipart message with two frames:
        // Frame 1: worker identity — the ROUTER socket pops this internally
        //          to route the remaining frames to the correct peer.
        // Frame 2: encoded payload — the msgpack-serialized WorkerMessage.
        //
        // The ROUTER socket's send API requires at least 2 frames. Fewer
        // frames will return ZmqError::Socket("ROUTER send requires at least 2 frames").
        // Construct the message with two frames in order: [identity, payload].
        // The ROUTER socket pops the first frame (identity) internally to
        // route the remaining frames to the correct peer.
        let frames: Vec<Bytes> = vec![
            Bytes::from(worker_id.to_vec()), // identity frame
            Bytes::from(encoded),            // payload frame
        ];
        let message = ZmqMessage::try_from(frames).expect("frames should be non-empty");

        // Acquire the mutex lock to send. The tokio mutex ensures that only
        // one task at a time can modify the socket state, preventing races
        // between concurrent send() calls.
        let mut socket = self.socket.lock().await;
        // The ROUTER socket pops the first frame (identity) internally and
        // routes the remaining frames to the matching peer. If no peer with
        // that identity is connected, it returns ZmqError::Other.
        socket.send(message).await?;

        // Log the worker identity as a hex string for readability.
        let hex_id: String = worker_id.iter().map(|b| format!("{b:02x}")).collect();
        tracing::debug!(worker_id = %hex_id, "message sent to worker");

        Ok(())
    }

    /// Receive a `WorkerEvent` from a connected worker via the ZeroMQ ROUTER socket.
    ///
    /// The ROUTER socket delivers messages as multipart frames: the first frame is the
    /// peer's identity (used for routing replies back to the correct worker), and the
    /// remaining frame(s) contain the encoded message payload.
    ///
    /// This method extracts the identity as a UTF-8 string, decodes the msgpack payload
    /// into a `WorkerEvent`, and returns the `(worker_id, event)` tuple.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Ipc` if the socket encounters a ZeroMQ error (e.g.
    /// connection lost, socket closed), if the identity frame is missing (protocol
    /// violation), if the payload frame is missing, or if the msgpack payload cannot
    /// be decoded into a `WorkerEvent`.
    ///
    /// # Logging
    ///
    /// Logs at DEBUG level: `tracing::debug!(worker_id = %worker_id, event_type = ?event,
    /// "event received from worker")`.
    pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError> {
        // Acquire the mutex lock to receive. The tokio mutex ensures that only
        // one task at a time can access the socket, preventing races between
        // concurrent recv() calls.
        let mut socket = self.socket.lock().await;

        // Receive the multipart message from the ROUTER socket. The ROUTER
        // returns a ZmqMessage where frame 0 is the peer identity and frame 1+
        // are the message payload. A ZmqError here indicates a socket-level
        // failure (connection lost, closed socket). Map to AnvilError::Ipc
        // since the design doc specifies recv() returns AnvilError.
        let msg = socket
            .recv()
            .await
            .map_err(|e| AnvilError::Ipc(format!("ROUTER recv failed: {e}")))?;

        // Extract the identity frame (frame 0). The ROUTER socket always
        // prepends the peer's identity as the first frame. If this frame
        // is missing, the message violates the ROUTER protocol contract.
        let identity_bytes = msg
            .get(0)
            .ok_or_else(|| AnvilError::Ipc("ROUTER recv returned no identity frame".to_string()))?;

        // Extract the payload frame (frame 1). This contains the msgpack-encoded
        // WorkerEvent. If missing, the message is incomplete.
        let payload_bytes = msg
            .get(1)
            .ok_or_else(|| AnvilError::Ipc("ROUTER recv returned no payload frame".to_string()))?;

        // Convert the identity bytes to a UTF-8 string. Worker identities
        // are typically ASCII strings (e.g. "worker-0", "test-worker-0").
        // Auto-generated zeromq identities are raw bytes, so we fall back
        // to hex encoding when the identity is not valid UTF-8.
        let worker_id = match String::from_utf8(identity_bytes.to_vec()) {
            Ok(s) => s,
            // Non-UTF8 identity: represent as hex string for log readability.
            // This handles auto-generated zeromq identities which are raw bytes.
            Err(_) => identity_bytes.iter().map(|b| format!("{b:02x}")).collect(),
        };

        // Decode the msgpack payload into a WorkerEvent. This uses the
        // `_type` discriminator field to select the correct enum variant.
        // IpcError is mapped to AnvilError::Ipc per the design doc's
        // specification that recv() returns AnvilError.
        let event =
            decode_event(payload_bytes.as_ref()).map_err(|e| AnvilError::Ipc(e.to_string()))?;

        // Log the received event with structured fields for log aggregation.
        // The event_type field captures the WorkerEvent variant for indexing.
        tracing::debug!(
            worker_id = %worker_id,
            event_type = ?event,
            "event received from worker"
        );

        Ok((worker_id, event))
    }
}
