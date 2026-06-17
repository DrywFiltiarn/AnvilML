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
///   by the ROUTER's recv. This identity is used only for `transport.send()`
///   and the misrouting cross-check described below — it is never logged
///   directly, since raw routing bytes (or their hex encoding) are not a
///   useful key for an operator to grep on. See `display_id` for the
///   identity used in all log output.
/// * `display_id` — The human-readable display label (e.g. `"worker-0"`)
///   used in every log statement emitted by this module. This is the same
///   `"worker-N"` string stored on `ManagedWorker` and shown in `WorkerInfo`
///   and WebSocket broadcasts elsewhere in the worker subsystem — passing
///   the same string here ensures a single worker is referred to identically
///   across every log line, regardless of which task or module emitted it.
///   This string has no meaning to ZeroMQ and must never be used for routing.
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
///
/// On every successfully received event, the reader also compares the wire
/// identity ZeroMQ actually delivered the frame under against this bridge's
/// own `worker_id`. A mismatch means the ROUTER socket handed this bridge an
/// event that originated from a different peer than the one it was given at
/// construction — i.e. cross-worker misrouting. This should never happen in
/// correct operation, so a mismatch is logged at WARN rather than silently
/// broadcast as if it were normal; the event is still broadcast either way,
/// since dropping it would leave the worker pool in an undetermined state.
pub fn start(
    transport: Arc<RouterTransport>,
    worker_id: Vec<u8>,
    display_id: String,
    msg_rx: mpsc::Receiver<WorkerMessage>,
    event_tx: broadcast::Sender<(String, WorkerEvent)>,
) -> (JoinHandle<()>, JoinHandle<()>) {
    // Clone transport, worker_id, and display_id for the writer task. The
    // original transport and worker_id values are moved into the reader's
    // async closure below, so the writer gets its own clones (cheap: Arc
    // refcount bump for transport, byte/string copies for the identities).
    let transport_writer = transport.clone();
    let worker_id_writer = worker_id.clone();
    let display_id_writer = display_id.clone();

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
                tracing::warn!(
                    worker_id = %display_id_writer,
                    error = %e,
                    "writer send failed"
                );
            }
            tracing::debug!(
                worker_id = %display_id_writer,
                msg_type = ?msg,
                "message sent to worker"
            );
        }
        // The mpsc channel sender was dropped — all messages have been
        // processed. This is a normal shutdown path, not an error.
        tracing::debug!(worker_id = %display_id_writer, "writer task ended (channel closed)");
    });

    // Spawn the reader task second. It runs independently of the writer —
    // events flow from transport → broadcast. The reader terminates when
    // the transport returns an error (e.g. socket closed).
    //
    // The reader does not use worker_id for routing — it receives the
    // identity from transport.recv() for each event, which is the correct
    // identity for the ROUTER socket. worker_id is retained here only to
    // cross-check against that received identity for misrouting detection
    // (see the WARN below); it is never logged directly.
    let reader_handle = tokio::spawn(async move {
        // Precompute the expected wire identity as a string once, outside
        // the loop, so the comparison below is a cheap string equality
        // check per event rather than re-decoding worker_id every iteration.
        // `String::from_utf8_lossy` degrades gracefully for the rare
        // non-UTF8 test identities instead of panicking; in production
        // worker_id is always the bare device index, which is valid UTF-8.
        let expected_wire_id = String::from_utf8_lossy(&worker_id).into_owned();

        // The reader uses `loop { match ... }` instead of `while let` because
        // transport.recv() returns Result, not Option. On Ok, we broadcast
        // the event; on Err, we log and break (terminal error for the reader).
        loop {
            match transport.recv().await {
                Ok((id, event)) => {
                    tracing::debug!(
                        worker_id = %display_id,
                        event_type = ?event,
                        "event received from worker"
                    );

                    // Cross-check: the wire identity ZeroMQ delivered this
                    // frame under should always match this bridge's own
                    // routing identity. A mismatch indicates the ROUTER
                    // socket associated this event with the wrong peer —
                    // a misrouting condition that should never occur in
                    // correct operation and is otherwise invisible, since
                    // the event is still broadcast and processed normally.
                    if id != expected_wire_id {
                        tracing::warn!(
                            worker_id = %display_id,
                            expected_wire_id = %expected_wire_id,
                            actual_wire_id = %id,
                            event_type = ?event,
                            "received event under unexpected wire identity — possible IPC misrouting"
                        );
                    }

                    // Broadcast to all listeners. If no receivers are
                    // subscribed, send() returns Err(RecvError::Closed)
                    // which we ignore — there is nothing to deliver.
                    let _ = event_tx.send((id, event));
                }
                Err(e) => {
                    // Transport recv error is terminal for the reader —
                    // the socket is closed or unreachable, so there is no
                    // point retrying. Log at WARN and break out of the loop.
                    tracing::warn!(
                        worker_id = %display_id,
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
