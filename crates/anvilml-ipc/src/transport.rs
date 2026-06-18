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
//!
//! **Why send and recv use separate locks:** `RouterSocket::split()` divides
//! the socket into independent send and recv halves rather than wrapping one
//! `RouterSocket` in a single shared mutex. With one shared lock, a `recv()`
//! call has no message waiting parks holding that lock for as long as no
//! traffic arrives — and since `anvilml-worker`'s demux task calls `recv()`
//! in an unbroken loop for the transport's entire lifetime, that meant any
//! concurrent `send()` (e.g. the `Shutdown` message during supervisor exit,
//! with every worker idle and no traffic incoming) could block indefinitely
//! waiting for a lock recv() had no reason to ever release. Splitting once
//! at `bind()` time means send() and recv() contend with nothing but their
//! own kind, and recv()'s lock — held by exactly one caller for the
//! transport's whole lifetime — is never actually contended at all.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::Mutex;
use tracing;
use zeromq::{
    Endpoint, RouterRecvHalf, RouterSendHalf, RouterSocket, Socket, SocketRecv, SocketSend,
    ZmqMessage,
};

use crate::error::RecvError;
use crate::TransportError;
use crate::{decode_event, encode_message, WorkerEvent, WorkerMessage};

/// A ZeroMQ ROUTER socket wrapper for sending messages to and receiving
/// events from AnvilML workers.
///
/// The ROUTER socket is the server-side socket type in ZeroMQ. It accepts
/// connections from worker DEALER sockets and routes messages to them by
/// identity.
///
/// # Thread safety
///
/// The send half and recv half are each behind their own `tokio::sync::Mutex`
/// — see the module docs for why these must be separate locks rather than
/// one shared lock across both operations. Both halves originate from a
/// single `RouterSocket::split()` call in `bind()`, so they share the same
/// underlying ZeroMQ socket and connection state despite being independently
/// lockable.
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
    /// The send half produced by `RouterSocket::split()`. `RouterSendHalf`
    /// is itself `Clone` (backed by an `Arc` internally), but the
    /// `SocketSend` trait still requires `&mut self` per call, so a lock
    /// is needed regardless — `Arc<Mutex<>>` here keeps the door open to
    /// handing out cloned halves per caller later without a structural
    /// change, though nothing currently needs that.
    send_half: Arc<Mutex<RouterSendHalf>>,

    /// The recv half produced by the same `split()` call. Not wrapped in
    /// `Arc` — unlike `send_half`, nothing needs to share ownership of
    /// this, since it's private and only ever reached through
    /// `RouterTransport::recv(&self)`. The `Mutex` exists only to satisfy
    /// `&self`'s borrow rules for the `&mut self` `SocketRecv::recv()`
    /// needs; in practice this lock is never contended, since exactly one
    /// task (the demux task in `anvilml-worker`) ever calls `recv()` for
    /// the transport's entire lifetime.
    recv_half: Mutex<RouterRecvHalf>,

    /// The TCP port the ROUTER socket is bound to.
    ///
    /// This is set by `bind()` when the socket binds to `tcp://127.0.0.1:0`,
    /// which causes the OS to assign an available port. The port number is
    /// extracted from the returned `Endpoint::Tcp(_, port)` and stored here
    /// for the caller (e.g., to pass to workers via environment variables).
    pub port: u16,
}

/// Render a raw ZeroMQ identity as a string for logging and routing.
///
/// Worker identities are UTF-8 strings in production (the bare device index,
/// e.g. `"0"`), but auto-generated DEALER identities (e.g. in tests) may be
/// arbitrary non-UTF8 bytes. This decodes as UTF-8 when valid and falls back
/// to a hex string otherwise. `send()` and `recv()` both use this so the same
/// underlying bytes always render identically regardless of which method
/// observed them — and `anvilml-worker`'s demux task uses the same function
/// to build its routing table keys, since a route only matches an incoming
/// event if both sides rendered the identity the same way.
pub fn render_identity(id: &[u8]) -> String {
    match std::str::from_utf8(id) {
        Ok(s) => s.to_string(),
        Err(_) => id.iter().map(|b| format!("{b:02x}")).collect(),
    }
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

        // Split into independent halves immediately after bind — see the
        // module docs for why send and recv must never share one lock.
        let (send_half, recv_half) = socket.split();

        Ok(Self {
            send_half: Arc::new(Mutex::new(send_half)),
            recv_half: Mutex::new(recv_half),
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

        // Construct the message with two frames in order: [identity, payload].
        // The ROUTER socket pops the first frame (identity) internally to
        // route the remaining frames to the correct peer. Fewer than 2
        // frames returns ZmqError::Socket("ROUTER send requires at least 2 frames").
        let frames: Vec<Bytes> = vec![
            Bytes::from(worker_id.to_vec()), // identity frame
            Bytes::from(encoded),            // payload frame
        ];
        let message = ZmqMessage::try_from(frames).expect("frames should be non-empty");

        // Locks only against other send() callers — never against recv(),
        // since the two now live behind independent mutexes (see module docs).
        let mut send_half = self.send_half.lock().await;
        send_half.send(message).await?;

        // Log the worker identity using the same UTF-8-or-hex rule as recv(),
        // so the same identity bytes always render identically in logs whether
        // they were sent to or received from the ROUTER socket.
        tracing::debug!(worker_id = %render_identity(worker_id), "message sent to worker");

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
    /// Returns [`RecvError`] — see that type's documentation for which of
    /// its four variants is genuinely fatal to the transport
    /// (`SocketClosed`) versus a per-message problem that a caller reading
    /// in a loop should be able to recover from (every other variant):
    /// `SocketClosed` if the socket encounters a ZeroMQ error (e.g.
    /// connection lost, closed socket); `MissingIdentityFrame` if the
    /// identity frame is missing; `MissingPayloadFrame` if the payload
    /// frame is missing; `DecodeFailed` if the msgpack payload cannot be
    /// decoded into a `WorkerEvent`.
    ///
    /// # Logging
    ///
    /// Logs at DEBUG level: `tracing::debug!(worker_id = %worker_id, event_type = ?event,
    /// "event received from worker")`.
    pub async fn recv(&self) -> Result<(String, WorkerEvent), RecvError> {
        // Locks only against other recv() callers — in practice never
        // contended, since exactly one task (anvilml-worker's demux task)
        // calls recv() for the transport's entire lifetime. See module docs.
        let mut recv_half = self.recv_half.lock().await;

        // A ZmqError here indicates a socket-level failure (connection
        // lost, closed socket) — the one genuinely fatal case. `#[from]`
        // on RecvError::SocketClosed handles the conversion.
        let msg = recv_half.recv().await?;

        // Extract the identity frame (frame 0). The ROUTER socket always
        // prepends the peer's identity as the first frame. If this frame
        // is missing, this single message violates the ROUTER protocol
        // contract — the socket itself is still alive.
        let identity_bytes = msg.get(0).ok_or(RecvError::MissingIdentityFrame)?;

        // Extract the payload frame (frame 1). This contains the msgpack-encoded
        // WorkerEvent. If missing, this single message is incomplete — the
        // socket itself is still alive.
        let payload_bytes = msg.get(1).ok_or(RecvError::MissingPayloadFrame)?;

        // Convert the identity bytes to a UTF-8 string. Worker identities
        // are typically ASCII strings (e.g. "worker-0", "test-worker-0").
        // Auto-generated zeromq identities are raw bytes, so we fall back
        // to hex encoding when the identity is not valid UTF-8.
        let worker_id = render_identity(identity_bytes);

        // Decode the msgpack payload into a WorkerEvent. This uses the
        // `_type` discriminator field to select the correct enum variant.
        // A decode failure here is specific to this one message's payload
        // — the socket itself is still alive. `#[from]` on
        // RecvError::DecodeFailed handles the conversion from IpcError.
        let event = decode_event(payload_bytes.as_ref())?;

        // Log the received event with structured fields for log aggregation.
        // The event_type field captures the WorkerEvent variant for indexing.
        tracing::debug!(
            worker_id = %worker_id,
            event_type = ?event,
            "event received from worker"
        );

        Ok((worker_id, event))
    }

    /// Like [`recv`](Self::recv), but also returns the unrendered identity
    /// bytes alongside the rendered string.
    ///
    /// Production code should use [`recv`](Self::recv) and `render_identity`
    /// — every routing table in this codebase (`anvilml-worker`'s demux
    /// task included) is keyed by the rendered string on both the send and
    /// recv side, never by raw bytes, so production never needs this.
    ///
    /// This exists for tests and diagnostics that need to address the same
    /// peer again after receiving from it — e.g. a test DEALER socket has
    /// no way to set a predictable identity in this version of the
    /// `zeromq` crate, so the only way to send a reply back to it is to
    /// recover the exact bytes ZeroMQ assigned, not a rendering of them.
    /// `render_identity` is lossy for non-UTF8 input (hex-encodes it), so
    /// `worker_id.as_bytes()` from `recv()`'s return is NOT a valid
    /// substitute for the original bytes when the identity isn't UTF-8 —
    /// using it as one would address `send()` to the literal ASCII bytes
    /// of a hex string, not to the peer that was actually received from.
    ///
    /// # Errors
    ///
    /// Same as [`recv`](Self::recv).
    pub async fn recv_with_raw_identity(
        &self,
    ) -> Result<(Vec<u8>, String, WorkerEvent), RecvError> {
        let mut recv_half = self.recv_half.lock().await;

        let msg = recv_half.recv().await?;

        let identity_bytes = msg.get(0).ok_or(RecvError::MissingIdentityFrame)?;

        let payload_bytes = msg.get(1).ok_or(RecvError::MissingPayloadFrame)?;

        let raw_identity = identity_bytes.to_vec();
        let worker_id = render_identity(identity_bytes);

        let event = decode_event(payload_bytes.as_ref())?;

        tracing::debug!(
            worker_id = %worker_id,
            event_type = ?event,
            "event received from worker"
        );

        Ok((raw_identity, worker_id, event))
    }
}
