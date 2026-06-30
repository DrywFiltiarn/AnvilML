//! The ZeroMQ ROUTER socket transport wrapper.
//!
//! Provides `RouterTransport` — an `Arc`-shareable wrapper around a ZeroMQ ROUTER socket
//! whose send and receive halves are split into independent `tokio::sync::Mutex` guards
//! at construction time. This is the structural fix for a v3 shutdown deadlock where a
//! blocked `recv()` held the same lock a concurrent `send()` needed.

use std::sync::Arc;

use tokio::sync::Mutex;
use zeromq::prelude::*;
use zeromq::{Endpoint, RouterRecvHalf, RouterSendHalf, RouterSocket};

use crate::IpcError;

/// The Rust-side ZeroMQ ROUTER socket wrapper.
///
/// Binds on construction. Ownership rule: constructed exactly once by `WorkerPool`
/// and shared via `Arc<RouterTransport>`. No other code holds the socket directly.
///
/// The send and receive halves are split into independent `tokio::sync::Mutex` guards
/// at construction time — this is the fix for a v3 shutdown deadlock where a blocked
/// `recv()` held the same lock a concurrent `send()` needed.
///
/// `send()` and `recv()` methods are deferred to task P7-B2, which implements the
/// split-lock send/recv methods that operate on these guards.
#[allow(dead_code)]
// Fields `sender` and `receiver` are not read by this task's code. They are
// populated by bind() and consumed by the send()/recv() methods deferred to
// task P7-B2. The compiler cannot see the deferred code, so it warns about
// the unused fields here. This suppression is legitimate because the fields
// are genuinely used by a future task that adds the methods.
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
    #[allow(dead_code)]
    // `receiver` is not read by this task's code. It is consumed by the recv()
    // method deferred to task P7-B2. Same justification as the `dead_code`
    // suppression on the struct itself.
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
}
