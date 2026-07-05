//! The ZeroMQ ROUTER socket transport wrapper.
//!
//! Provides `RouterTransport` — an `Arc`-shareable wrapper around a ZeroMQ ROUTER socket.
//! `send()` and `recv()` operate on independent locks (`sender`/`receiver`), each guarding
//! only its own direction — see `ANVILML_DESIGN.md §8.3`. This is not a style choice: a
//! single shared lock around both directions is the documented root cause of v3's shutdown
//! deadlock, and P8-E5 (`KeepaliveWatchdog` spawned as a task concurrent with
//! `ManagedWorker::run()`, sharing this same transport) is proof the risk is real, not
//! theoretical — it is the first code to exercise genuine concurrent send+recv from two
//! separate tasks, and previously would have blocked a `send()` for as long as a concurrent
//! `recv()` was waiting on a message that might never arrive.
//!
//! The transport can be closed via `close()`, which causes the next `send()`/`recv()` to
//! return an error — this is used by tests to exercise the worker crash path. `close()`'s
//! signal is delivered via a `watch` channel, deliberately independent of both locks — see
//! `close()`'s doc comment for why.

use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::{Mutex, watch};
use zeromq::prelude::*;
use zeromq::{Endpoint, RouterRecvHalf, RouterSendHalf, RouterSocket, ZmqMessage};

use crate::IpcError;
use crate::messages::{WorkerEvent, WorkerMessage};

/// The Rust-side ZeroMQ ROUTER socket wrapper.
///
/// Binds on construction. Ownership rule: constructed exactly once by `WorkerPool`
/// and shared via `Arc<RouterTransport>`. No other code holds the socket directly.
///
/// `send()` and `recv()` are backed by independent locks (`sender`/`receiver`) —
/// see `ANVILML_DESIGN.md §8.3`. The `close()` method replaces both halves with
/// `None`, causing the next `send()`/`recv()` to return an error.
pub struct RouterTransport {
    /// The send half of the ROUTER socket, protected by its own mutex.
    ///
    /// Entirely independent from `receiver`'s lock — a `send()` call never
    /// waits on anything a concurrent `recv()` is doing, and vice versa.
    /// Stored in an `Option` so `close()` can replace it with `None`,
    /// causing the next `send()` to fail.
    sender: Arc<Mutex<Option<RouterSendHalf>>>,

    /// The recv half of the ROUTER socket, protected by its own mutex.
    ///
    /// Entirely independent from `sender`'s lock — see `sender`'s doc comment.
    /// `recv()` holds this lock for the entire duration of the blocking inner
    /// socket read (it needs `&mut RouterRecvHalf` for that whole span), but
    /// since `sender` is a separate lock, this no longer blocks a concurrent
    /// `send()` the way a single shared lock did in v3 (see module docs).
    /// Stored in an `Option` so `close()` can replace it with `None`, causing
    /// the next `recv()` to fail.
    receiver: Arc<Mutex<Option<RouterRecvHalf>>>,

    /// Closed signal, deliberately independent of both `sender`'s and
    /// `receiver`'s locks.
    ///
    /// `recv()` holds `receiver`'s lock for the entire duration of the
    /// blocking inner socket read. If `close()` used that same lock to
    /// signal closure, it would block forever whenever a `recv()` call is
    /// genuinely in flight and waiting for a network message that will never
    /// arrive — the lock would never be released and `close()` could never
    /// acquire it. This `watch` channel lets `close()` signal instantly,
    /// lock-free, and lets `recv()` race that signal against the real socket
    /// read via `tokio::select!`, so a blocked `recv()` is interrupted rather
    /// than permanently starving `close()`.
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

        // Split into independent send/recv halves per §8.3 — see the struct's
        // field docs for why this must never be collapsed back into one lock.
        // zeromq 0.6.0's RouterSocket::split() is exactly this: it consumes
        // the socket and returns (RouterSendHalf, RouterRecvHalf), each
        // usable from a different task with no shared internal lock between
        // them (RouterSendHalf is even Clone, backed by its own Arc).
        let (send_half, recv_half) = socket.split();

        Ok(RouterTransport {
            sender: Arc::new(Mutex::new(Some(send_half))),
            receiver: Arc::new(Mutex::new(Some(recv_half))),
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

        // Acquire the sender lock (independent from receiver's — see struct
        // docs) and send the message.
        let mut sender = self.sender.lock().await;
        let sender = sender
            .as_mut()
            .ok_or_else(|| IpcError::SendFailed("transport is closed".to_string()))?;

        // Send the 3-frame message over the ROUTER socket's send half. The
        // SocketSend trait is provided by zeromq::prelude::* and implemented
        // on RouterSendHalf.
        sender
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

        // Acquire the sender lock (independent from receiver's — see struct
        // docs) and send the message.
        let mut sender = self.sender.lock().await;
        let sender = sender
            .as_mut()
            .ok_or_else(|| IpcError::SendFailed("transport is closed".to_string()))?;

        // Send the 3-frame message over the ROUTER socket's send half.
        sender
            .send(message)
            .await
            .map_err(|e| IpcError::SendFailed(e.to_string()))?;

        tracing::debug!(worker_id = %worker_id, "raw message sent");
        Ok(())
    }

    /// Close the transport, causing the next `send()`/`recv()` to return an error.
    ///
    /// This is used by tests to exercise the worker crash path (transport
    /// recv error).
    ///
    /// Signals via the lock-free `closed_tx` watch channel *first*, before
    /// touching either lock. This ordering matters for `receiver`'s lock
    /// specifically: if a `recv()` call is currently in flight, holding
    /// `receiver`'s lock while blocked on a network read that will never
    /// arrive, this method's own attempt to acquire that same lock (below)
    /// would otherwise block forever. Signaling first guarantees the
    /// in-flight `recv()`'s `tokio::select!` (see `recv()`) observes the
    /// signal and returns promptly, releasing the lock, before this method's
    /// own lock acquisition is reached. (`sender`'s lock has no equivalent
    /// hazard — `send()` never blocks indefinitely the way `recv()` does — but
    /// it's still cleared here so a `send()` after `close()` correctly fails.)
    pub async fn close(&self) {
        // Signal first (lock-free) — see the doc comment above for why this
        // ordering is required, not just convenient.
        let _ = self.closed_tx.send(true);

        // Clear both halves. By this point any in-flight recv() has already
        // been signaled and will release receiver's lock on its own; this
        // acquisition is not competing with a recv() that can never finish
        // on its own. sender and receiver are independent locks, so there's
        // no ordering hazard between clearing the two.
        let mut sender = self.sender.lock().await;
        *sender = None;
        let mut receiver = self.receiver.lock().await;
        *receiver = None;
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
        // Fast path: already closed. Avoids acquiring the receiver lock at
        // all for any recv() call made after close() has already completed.
        if *self.closed_tx.subscribe().borrow() {
            return Err(IpcError::RecvFailed("transport is closed".to_string()));
        }

        let mut closed_rx = self.closed_tx.subscribe();
        let mut receiver_guard = self.receiver.lock().await;
        let receiver = receiver_guard
            .as_mut()
            .ok_or_else(|| IpcError::RecvFailed("transport is closed".to_string()))?;

        // Receive a 3-frame ROUTER multipart message, or bail out if close()
        // signals while this read is still pending. The SocketRecv trait is
        // provided by zeromq::prelude::* and implemented on RouterRecvHalf.
        // This lock is independent from sender's (see struct docs) — a
        // concurrent send() is never blocked by this call being in flight.
        let message = tokio::select! {
            result = receiver.recv() => {
                result.map_err(|e| IpcError::RecvFailed(e.to_string()))?
            }
            _ = closed_rx.changed() => {
                return Err(IpcError::RecvFailed("transport is closed".to_string()));
            }
        };

        // Convert the message into individual frames.
        // ROUTER delivers:
        //   - [identity, delimiter, payload] when the sender sent a 2-frame message
        //     (the ROUTER prepends identity + delimiter before the 2 original frames)
        //   - [identity, payload] when the sender sent a 1-frame message
        //     (the ROUTER prepends only identity, no delimiter for single-frame)
        // This difference exists because the ROUTER protocol only inserts the
        // delimiter between the identity and the *first original frame* when there
        // are multiple original frames — for a single-frame message there's nothing
        // to delimit, so the delimiter is omitted. Both pyzmq and the Rust zeromq
        // crate follow this behavior; the Rust test in bridge_tests.rs sends a
        // 2-frame ZmqMessage which triggers the delimiter, but the Python worker
        // sends a 1-frame msgpack message which does not.
        let frames = message.into_vec();

        // Validate frame count — ROUTER multipart messages have either 2 or 3
        // frames depending on whether the sender used a 1-frame or 2-frame
        // message. Both are valid.
        if frames.len() < 2 {
            return Err(IpcError::RecvFailed(format!(
                "expected 2 or 3 frames, got {}",
                frames.len()
            )));
        }

        // Extract the worker identity from frame 0.
        // This is the string the worker registered with as its ZeroMQ DEALER identity.
        let identity = String::from_utf8(frames[0].to_vec())
            .map_err(|e| IpcError::RecvFailed(format!("invalid UTF-8 identity: {e}")))?;

        // Extract the msgpack payload from frame 1 (2-frame message) or
        // frame 2 (3-frame message with delimiter). The payload is always
        // the last frame.
        let payload = &frames[frames.len() - 1];

        // Deserialize the msgpack payload into a WorkerEvent.
        // from_slice reads the flat dict and dispatches on the "_type" field
        // to construct the correct enum variant.
        let event = rmp_serde::from_slice(payload)
            .map_err(|e| IpcError::RecvFailed(format!("deserialization failed: {e}")))?;

        tracing::debug!(worker_id = %identity, event_type = ?event, "message received");
        Ok((identity, event))
    }
}
