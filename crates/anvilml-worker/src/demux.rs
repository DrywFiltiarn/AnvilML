//! Single shared reader for the ROUTER socket.
//!
//! Exactly one task in the whole process calls `RouterTransport::recv()`.
//! That constraint is the entire point of this module: a ROUTER socket has
//! no notion of "this event belongs to caller X" beyond the identity frame
//! it returns, so any second concurrent reader can win the next `recv()`
//! and walk off with an event meant for someone else. Centralizing the read
//! here, then dispatching by identity into per-worker channels, makes that
//! failure mode structurally impossible rather than merely unlikely.
//!
//! The routing table is shared and growable (`Arc<Mutex<HashMap<...>>>`),
//! not a one-shot table fixed at task start. `ManagedWorker::spawn()`
//! starts a worker's keepalive — and so its first ping — before returning,
//! so a table built only after every device has spawned would leave every
//! worker's first ping/pong racing the demux task's own startup, the same
//! class of bug this module exists to eliminate. Registering each route
//! the instant its worker's `spawn()` call returns, against a demux task
//! already running, closes that gap to the time of one lock acquisition
//! rather than the time to spawn every device in the pool.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tracing;

use anvilml_ipc::{RecvError, RouterTransport, WorkerEvent};

/// Destination for events addressed to one worker: its display label (for
/// logging) and the broadcast channel its `ManagedWorker` subscribes to.
pub type Route = (String, broadcast::Sender<(String, WorkerEvent)>);

/// Shared, growable routing table. Callers register routes via the same
/// `Arc` passed to `start()`, so a route added after the demux task is
/// already running is visible on its very next loop iteration.
pub type RouteTable = Arc<Mutex<HashMap<String, Route>>>;

/// Register one worker's route, keyed exactly as `RouterTransport::recv()`
/// would render that worker's IPC identity.
///
/// Calling this before `start()` pre-seeds the table; calling it after is
/// equally valid, since `start()`'s loop re-locks the table on every
/// iteration rather than capturing a snapshot.
pub async fn register(routes: &RouteTable, key: String, route: Route) {
    routes.lock().await.insert(key, route);
}

/// Remove one worker's route from the table, keyed the same way as
/// `register()`.
///
/// Without this, a crashed or shut-down worker's entry stays in the table
/// forever — the broadcast channel its `Route` holds has no live receiver,
/// so every subsequent event for that identity is a wasted lookup and a
/// `send()` into the void, and the table itself grows without bound across
/// repeated respawns. Calling this during a worker's own shutdown keeps the
/// table's size bounded by the number of *currently live* workers, not the
/// number that have ever existed.
///
/// A no-op if `key` isn't present — deregistering twice, or deregistering a
/// worker that never successfully registered (e.g. it crashed before
/// `spawn_all` reached the registration call), is not an error.
pub async fn deregister(routes: &RouteTable, key: &str) {
    routes.lock().await.remove(key);
}

/// Spawn the demux task and return its `JoinHandle`.
///
/// `routes` must be keyed using `anvilml_ipc::render_identity()` — the same
/// function `RouterTransport::recv()` uses internally to render the wire
/// identity it returns. Building keys any other way risks the two sides
/// disagreeing on non-UTF8 identities and every lookup silently missing.
/// Each received event is forwarded to the matching route's channel; an
/// identity with no route is logged at WARN rather than silently dropped,
/// since it means a peer is talking to the ROUTER that was never
/// registered — not necessarily a misconfiguration, since a worker whose
/// `spawn()` call hasn't returned yet is indistinguishable from one that
/// will never register at all from inside this loop.
///
/// The task runs until `transport.recv()` returns
/// [`RecvError::SocketClosed`](anvilml_ipc::RecvError::SocketClosed) — the
/// one variant that means the transport itself is gone. Every other
/// `RecvError` variant (missing identity frame, missing payload frame,
/// failed decode) is a problem with one single message from one peer, not
/// with the socket as a whole; this loop logs those at WARN and continues,
/// since this is the only demux task for the entire process — stopping it
/// over one malformed message would silently kill event delivery for every
/// other worker sharing the same ROUTER socket, not just the one that sent
/// the bad message.
pub fn start(transport: Arc<RouterTransport>, routes: RouteTable) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match transport.recv().await {
                Ok((id, event)) => {
                    // Re-locked every iteration rather than once outside
                    // the loop, specifically so routes registered after
                    // this task started are visible immediately.
                    let table = routes.lock().await;
                    match table.get(&id) {
                        Some((display_id, event_tx)) => {
                            tracing::debug!(
                                worker_id = %display_id,
                                event_type = ?event,
                                "event received from worker"
                            );
                            // A `Closed` error here just means the worker's
                            // ManagedWorker has no active subscriber right now
                            // (e.g. mid-shutdown) — not a reason to stop demuxing
                            // for every other worker still running.
                            let _ = event_tx.send((id, event));
                        }
                        None => {
                            tracing::warn!(
                                wire_id = %id,
                                event_type = ?event,
                                "event received from unregistered identity, dropping"
                            );
                        }
                    }
                }
                Err(RecvError::SocketClosed(e)) => {
                    // The socket itself is gone — every worker loses its
                    // event feed simultaneously, so this is fatal for the
                    // demux task as a whole, not per-worker recoverable.
                    tracing::warn!(error = %e, "demux recv failed, stopping");
                    break;
                }
                Err(
                    e @ (RecvError::MissingIdentityFrame
                    | RecvError::MissingPayloadFrame
                    | RecvError::DecodeFailed(_)),
                ) => {
                    // A problem with this one message from one peer — the
                    // socket itself is still alive, and every other worker's
                    // events are unaffected. Logging and falling through to
                    // the next loop iteration is what keeps one malformed
                    // message (from a worker, or from any other peer that
                    // connects and sends garbage) from silently killing
                    // event delivery for the entire pool.
                    tracing::warn!(error = %e, "demux recv failed for one message, continuing");
                }
            }
        }
    })
}
