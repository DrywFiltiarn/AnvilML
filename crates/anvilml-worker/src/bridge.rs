//! IPC bridge — the writer half of worker message routing.
//!
//! `ManagedWorker` uses this module to forward outgoing `WorkerMessage`s to
//! its worker subprocess via the shared ZeroMQ ROUTER socket.
//!
//! This module previously also owned a reader task that called
//! `RouterTransport::recv()` directly, one per worker. That was unsound:
//! `RouterTransport` wraps a single ROUTER socket shared by every worker, so
//! N independent reader tasks were racing to consume the same event stream,
//! with no guarantee a given `recv()` call returned that task's own worker's
//! event. See `crate::demux` for the single shared reader that replaced it.
//! `send()` was never subject to this hazard, since each call addresses a
//! specific identity directly — there is no shared consumption to race.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing;

use anvilml_ipc::{RouterTransport, WorkerMessage};

/// Spawn the IPC bridge writer task: forwards outgoing messages to a worker.
///
/// Receives `WorkerMessage` values from `msg_rx` and sends each via
/// `RouterTransport::send()`, addressed to `worker_id`. Runs until the
/// corresponding `mpsc::Sender` is dropped (e.g. on `ManagedWorker`
/// shutdown), at which point `msg_rx.recv()` returns `None` and the task
/// exits cleanly. Returns the task's `JoinHandle`.
///
/// # Arguments
///
/// * `transport` — The shared `RouterTransport`. Safe to send from
///   concurrently across workers; see the module docs for why receiving is not.
/// * `worker_id` — The IPC routing identity for `transport.send()`. Must
///   match the ZMQ identity the Python worker registered — the bare device
///   index (e.g. `"0"`), per `ANVILML_WORKER_ID`, not the `"worker-N"`
///   display label. Never logged directly; see `display_id`.
/// * `display_id` — The human-readable label (e.g. `"worker-0"`) used in
///   every log statement here. Pass the same string stored on
///   `ManagedWorker` so a worker is identified consistently across all logs.
/// * `msg_rx` — Receive half of the mpsc channel; the caller holds the sender.
pub fn start(
    transport: Arc<RouterTransport>,
    worker_id: Vec<u8>,
    display_id: String,
    msg_rx: mpsc::Receiver<WorkerMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Returning None (sender dropped) is the intended shutdown signal,
        // not an edge case to special-case separately.
        let mut msg_rx = msg_rx;
        while let Some(msg) = msg_rx.recv().await {
            // Non-fatal: the transport may be temporarily unavailable during
            // worker respawn, so a failed send doesn't stop the loop.
            if let Err(e) = transport.send(&worker_id, &msg).await {
                tracing::warn!(
                    worker_id = %display_id,
                    error = %e,
                    "writer send failed"
                );
            }
            tracing::debug!(
                worker_id = %display_id,
                msg_type = ?msg,
                "message sent to worker"
            );
        }
        // Not an error: a dropped sender is the normal shutdown signal.
        tracing::debug!(worker_id = %display_id, "writer task ended (channel closed)");
    })
}
