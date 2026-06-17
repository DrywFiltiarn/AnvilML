//! IPC bridge — two independent tokio tasks for routing messages between
//! `ManagedWorker` and the shared ZeroMQ ROUTER socket.
//!
//! The bridge connects the worker's message channel and event broadcast to the
//! transport layer. It spawns two tokio tasks:
//!
//! - **Writer task:** Receives `WorkerMessage` from an `mpsc::Receiver` and
//!   sends each via `RouterTransport::send()`. Terminates when the channel
//!   sender is dropped (e.g. `ManagedWorker` shutdown).
//! - **Reader task:** Receives `(String, WorkerEvent)` from the transport and
//!   broadcasts each via a `broadcast::Sender`. Terminates when the transport
//!   returns an error (e.g. socket closed).
//!
//! Both tasks are returned as `JoinHandle`s so the caller can store them in
//! `ManagedWorker` and await them during shutdown.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing;

use anvilml_ipc::{RouterTransport, WorkerEvent, WorkerMessage};

/// Format a worker identity as a hex string for logging.
///
/// Worker identities from ZeroMQ ROUTER sockets are raw bytes (often UUIDs).
/// Converting to hex produces a readable, stable representation for log
/// aggregation and debugging.
fn format_worker_id(id: &[u8]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

/// Spawn the IPC bridge: two independent tokio tasks for message routing.
///
/// The bridge connects a worker's message channel and event broadcast to the
/// shared ZeroMQ ROUTER socket. It creates a writer task that forwards messages
/// from the `mpsc::Receiver` to the transport, and a reader task that forwards
/// events from the transport to the broadcast channel.
///
/// # Arguments
///
/// * `transport` — The shared `RouterTransport` wrapping the ROUTER socket.
///   Cloned (Arc inner) for each task.
/// * `worker_id` — The IPC routing identity used as the key for
///   `transport.send()`. This must match the ZMQ identity the Python
///   worker actually registered, not a human-readable display label.
///   For workers launched via `build_command()`, this is the bare device
///   index as a UTF-8 string (e.g. `"0"`), matching `ANVILML_WORKER_ID`
///   set by `build_worker_env()`. For auto-generated identities from
///   DEALER sockets (e.g. in tests), it is the raw byte sequence returned
///   by the ROUTER's recv. The `"worker-N"` display label used elsewhere
///   for logging and WebSocket broadcasts is a separate, unrelated string
///   and must never be passed here.
/// * `msg_rx` — The receive half of the mpsc channel. The caller owns the
///   sender and drops it during shutdown to signal the writer task to exit.
/// * `event_tx` — The broadcast sender for events received from the transport.
///
/// # Returns
///
/// A tuple of `(writer_handle, reader_handle)`. The caller should store both
/// handles (e.g. in `ManagedWorker`) and await them during shutdown.
///
/// # Writer task
///
/// The writer receives messages from `msg_rx` and sends each via the transport.
/// It uses `while let Some(msg) = msg_rx.recv().await` — when the channel sender
/// is dropped, `recv()` returns `None` and the loop exits cleanly. Transport
/// send errors are logged at WARN but do not abort the task, because the
/// transport may temporarily be unavailable during worker respawn.
///
/// # Reader task
///
/// The reader loops receiving events from the transport and broadcasting each.
/// It uses `loop { match ... }` because `transport.recv()` returns `Result`,
/// not `Option`. On error (e.g. socket closed), the reader logs at WARN and
/// breaks out of the loop. Transport errors are terminal for the reader — if
/// the socket is closed, there is no point retrying.
pub fn start(
    transport: Arc<RouterTransport>,
    worker_id: Vec<u8>,
    msg_rx: mpsc::Receiver<WorkerMessage>,
    event_tx: broadcast::Sender<(String, WorkerEvent)>,
) -> (JoinHandle<()>, JoinHandle<()>) {
    // Clone transport and worker_id for the writer task. The original values
    // are moved into the async closure, so we clone the Arc (cheap reference
    // count bump) for the reader, and clone worker_id for the writer.
    let transport_writer = transport.clone();
    let worker_id_writer = worker_id.clone();

    // Spawn the writer task first. It runs independently of the reader —
    // messages flow from mpsc → transport, while events flow from transport
    // → broadcast. The writer terminates when the mpsc sender is dropped.
    let writer_handle = tokio::spawn(async move {
        // The writer uses `while let Some(msg) = msg_rx.recv().await` because
        // the mpsc Receiver returns Option — None when all senders are dropped.
        // This is the natural termination signal for the writer task.
        let mut msg_rx = msg_rx;
        while let Some(msg) = msg_rx.recv().await {
            // Send the message to the worker via the transport. Errors here
            // are non-fatal — the transport may be temporarily unavailable
            // during worker respawn, so we log at WARN and continue rather
            // than aborting the task.
            if let Err(e) = transport_writer.send(&worker_id_writer, &msg).await {
                let hex_id = format_worker_id(&worker_id_writer);
                tracing::warn!(
                    worker_id = %hex_id,
                    error = %e,
                    "writer send failed"
                );
            }
            let hex_id = format_worker_id(&worker_id_writer);
            tracing::debug!(
                worker_id = %hex_id,
                msg_type = ?msg,
                "message sent to worker"
            );
        }
        // The mpsc channel sender was dropped — all messages have been
        // processed. This is a normal shutdown path, not an error.
        let hex_id = format_worker_id(&worker_id_writer);
        tracing::debug!(worker_id = %hex_id, "writer task ended (channel closed)");
    });

    // Spawn the reader task second. It runs independently of the writer —
    // events flow from transport → broadcast. The reader terminates when
    // the transport returns an error (e.g. socket closed).
    //
    // The reader does not use worker_id for routing — it receives the
    // identity from transport.recv() for each event, which is the correct
    // identity for the ROUTER socket.
    let reader_handle = tokio::spawn(async move {
        // The reader uses `loop { match ... }` instead of `while let` because
        // transport.recv() returns Result, not Option. On Ok, we broadcast
        // the event; on Err, we log and break (terminal error for the reader).
        loop {
            match transport.recv().await {
                Ok((id, event)) => {
                    tracing::debug!(
                        worker_id = %id,
                        event_type = ?event,
                        "event received from worker"
                    );
                    // Broadcast to all listeners. If no receivers are
                    // subscribed, send() returns Err(RecvError::Closed)
                    // which we ignore — there is nothing to deliver.
                    let _ = event_tx.send((id, event));
                }
                Err(e) => {
                    // Transport recv error is terminal for the reader —
                    // the socket is closed or unreachable, so there is no
                    // point retrying. Log at WARN and break out of the loop.
                    let hex_id = format_worker_id(&worker_id);
                    tracing::warn!(
                        worker_id = %hex_id,
                        error = %e,
                        "reader recv failed, stopping"
                    );
                    break;
                }
            }
        }
    });

    (writer_handle, reader_handle)
}
