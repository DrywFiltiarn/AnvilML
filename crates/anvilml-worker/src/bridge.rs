//! The IPC bridge: independent reader/writer tasks against the split
//! `RouterTransport` (P7-B2, `ANVILML_DESIGN.md §8.3`), per `§9.6`.
//!
//! Two separate `tokio::spawn` tasks, each locking only its own half of the
//! already-split transport — no combined `select!` loop, no shared-lock
//! deadlock risk, because nothing here needs to clone a socket. The writer
//! task drains an `mpsc::Receiver`, sending each message via
//! `RouterTransport::send()`. The reader task loops
//! `RouterTransport::recv()`, routing each incoming `(worker_id, event)`
//! pair via `Demux::route()`.
//!
//! # Scope: one bridge for the whole pool, not one per worker
//!
//! `spawn_bridge()` is called exactly once, for the entire worker pool, not
//! once per worker. This was a genuine ambiguity in this task's own spec —
//! `ANVILML_DESIGN.md §9.6`'s illustrative code snippet uses a `worker_id`
//! variable inside the writer task without showing where it comes from,
//! which could as easily read as "one bridge per worker, `worker_id`
//! captured per instance." That reading doesn't hold up: `RouterTransport`
//! is a single, pool-wide ROUTER socket (constructed once, shared via
//! `Arc`) — if `spawn_bridge()` were called once per worker, every
//! resulting reader task would call `transport.recv()` concurrently
//! against the *same* shared socket, and `recv()` returns whatever message
//! arrives next regardless of source identity. Any reader task could
//! "steal" a message meant for a different worker's reader task entirely —
//! not a narrowing of an existing race, a *new* one this task would have
//! introduced. This task's own acceptance text — "the SOLE production
//! caller of transport.recv(), prerequisite for P8-F2's multi-worker-race
//! fix" — only makes sense under the pool-wide reading: it explicitly
//! names a multi-worker race as real and defers *fixing* it to P8-F2
//! (`ManagedWorker consumes its own demux channel`), not something this
//! task introduces or needs to solve itself.
//!
//! # Deviation from this task's literal stated signature
//!
//! The task's own text states `spawn_bridge(...) -> (Sender<WorkerMessage>,
//! JoinHandle<()>, JoinHandle<()>)`. That signature cannot work under the
//! confirmed pool-wide scope above: `RouterTransport::send()` requires a
//! `worker_id: &str` for every call, and a pool-wide writer task has no
//! other way to know which worker each outgoing `WorkerMessage` targets.
//! The channel item type is `(String, WorkerMessage)` here instead — a
//! direct, necessary mechanical consequence of the pool-wide scope
//! decision above, not an independent design choice. `WorkerMessage`
//! itself was not extended with an identity field: it's defined in
//! `anvilml-ipc` and serialized directly over the wire to the Python
//! worker process, so adding a field would be a wire-protocol change output
//! of this task's scope, not a fix contained to this new module.
//!
//! # Error handling: exit on the first error, matching §9.6's own `?` usage
//!
//! `IpcError` has no variant distinguishing a permanent condition (the
//! transport was closed) from a transient one (e.g. a single malformed
//! frame) — both collapse into `RecvFailed(String)`, distinguishable only
//! by string content, which would be fragile to match on. `§9.6`'s own
//! code example uses `?` on both `transport.send()` and `transport.recv()`
//! — explicit propagate-and-exit semantics, not a retry or backoff scheme.
//! Both tasks here log the error and then exit (return) on the first one,
//! matching that intent exactly; literal `?` isn't used because the tasks
//! return `()` (matching this task's own stated `JoinHandle<()>`), not a
//! `Result`, so there's nothing for `?` to propagate into.

use std::sync::Arc;

use anvilml_ipc::{RouterTransport, WorkerMessage};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::demux::Demux;

/// Channel capacity for the writer task's inbound queue. Matches this
/// crate's existing convention for bounded internal channels (e.g.
/// `managed.rs`'s `pong_tx`/`registered_tx`), not a value `§9.6` itself
/// specifies.
const WRITER_CHANNEL_CAPACITY: usize = 16;

/// Spawn the IPC bridge's reader and writer tasks.
///
/// One call for the entire worker pool — see this module's own doc
/// comment for why, and for why the returned sender's item type is
/// `(String, WorkerMessage)` rather than this task's literally stated
/// `WorkerMessage`.
///
/// Returns:
/// - A sender for queuing `(worker_id, message)` pairs; the writer task
///   drains this and calls `transport.send(&worker_id, &message)` for
///   each one.
/// - The writer task's `JoinHandle`. The task exits once every sender
///   clone is dropped (its `mpsc::Receiver::recv()` returns `None`) or
///   once a `transport.send()` call fails — see this module's own doc
///   comment on error handling.
/// - The reader task's `JoinHandle`. The task loops
///   `transport.recv()`, routing each `(worker_id, event)` pair via
///   `demux.route()`, until a `transport.recv()` call fails.
pub fn spawn_bridge(
    transport: Arc<RouterTransport>,
    demux: Arc<Demux>,
) -> (
    mpsc::Sender<(String, WorkerMessage)>,
    JoinHandle<()>,
    JoinHandle<()>,
) {
    let (tx, mut rx) = mpsc::channel::<(String, WorkerMessage)>(WRITER_CHANNEL_CAPACITY);

    // writer_task: drains the mpsc channel, sends via RouterTransport::send().
    // Locks only the transport's send half (see anvilml-ipc/src/transport.rs)
    // — never contends with reader_task's recv() below.
    let writer_transport = Arc::clone(&transport);
    let writer_handle = tokio::spawn(async move {
        while let Some((worker_id, msg)) = rx.recv().await {
            if let Err(e) = writer_transport.send(&worker_id, &msg).await {
                tracing::error!(
                    worker_id = %worker_id,
                    error = %e,
                    "bridge writer_task: send failed, exiting"
                );
                return;
            }
        }
        tracing::info!("bridge writer_task: channel closed, exiting");
    });

    // reader_task: pumps RouterTransport::recv(), routes via demux.
    // Locks only the transport's recv half — never contends with
    // writer_task's send() above.
    let reader_transport = transport;
    let reader_handle = tokio::spawn(async move {
        loop {
            let (worker_id, event) = match reader_transport.recv().await {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::error!(error = %e, "bridge reader_task: recv failed, exiting");
                    return;
                }
            };
            if let Err(e) = demux.route(&worker_id, event).await {
                // A routing failure (e.g. no worker registered under this
                // ID, or the paired receiver was dropped) does not mean
                // the transport itself is broken — unlike a recv()
                // failure, this is scoped to a single message for a
                // single worker, so the reader loop continues rather
                // than exiting.
                tracing::warn!(
                    worker_id = %worker_id,
                    error = %e,
                    "bridge reader_task: route failed, continuing"
                );
            }
        }
    });

    (tx, writer_handle, reader_handle)
}
