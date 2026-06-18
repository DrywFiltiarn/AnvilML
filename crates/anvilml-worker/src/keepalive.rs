//! Keepalive heartbeat — periodic Ping/Pong liveness detection for a worker subprocess.
//!
//! This module implements a background tokio task that periodically sends `Ping{seq}`
//! messages to a worker via the IPC bridge and waits for matching `Pong{seq}` responses.
//! If a pong is not received within the configured `pong_timeout`, the task invokes an
//! `on_timeout` callback to signal that the worker may be unresponsive.
//!
//! The heartbeat loop uses `select!` with a per-ping deadline sleep rather than
//! `tokio::time::interval` because the deadline is relative to each ping send time,
//! not a fixed clock interval.
//!
//! **Ready gate:** The loop does not send its first ping until the caller's
//! `ready_rx` resolves — see `start()`'s `ready_rx` parameter. This keeps
//! pings from being sent (and pong timeouts from being logged) against a
//! worker that hasn't finished initializing yet.
//!
//! **Shutdown:** The caller receives a `HeartbeatHandle` alongside the `JoinHandle`.
//! Calling `shutdown()` causes the loop to exit promptly — at either of its
//! two wait points (the ping-interval sleep, or the pong-timeout wait) —
//! rather than only at a cycle boundary.

use std::time::Duration;

use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing;

use anvilml_ipc::{WorkerEvent, WorkerMessage};

/// A handle for signalling the heartbeat loop to shut down.
///
/// The caller (typically `ManagedWorker`) stores this handle alongside the
/// `JoinHandle` returned by `start()`. Calling `shutdown()` causes the loop
/// to exit cleanly and promptly — including while it's in the middle of
/// either of its two wait points (the `ping_interval` sleep between
/// cycles, or the `pong_timeout` wait for a matching pong) — not just at a
/// cycle boundary.
///
/// Backed by a `tokio::sync::watch` channel rather than a bare
/// `Mutex<bool>` plus a separate wakeup primitive: the loop has two
/// sequential wait points that each need to observe a single `shutdown()`
/// call, and a one-shot wakeup (e.g. `Notify`'s single stored permit)
/// would be consumed by whichever wait point reaches it first, leaving the
/// other to block for its full duration regardless. A `watch` value does
/// not have this problem — `*rx.borrow()` can be checked any number of
/// times at any number of separate `.await` points and always reflects
/// the latest sent value, with no "consumption" semantics to race.
#[derive(Debug)]
pub struct HeartbeatHandle {
    /// Sends `true` once, on `shutdown()`. The loop holds the matching
    /// `watch::Receiver` and checks/awaits it at both of its wait points.
    shutdown_tx: watch::Sender<bool>,
}

impl HeartbeatHandle {
    /// Signal the heartbeat loop to shut down immediately.
    ///
    /// Sends `true` on the underlying watch channel. Whichever wait point
    /// the loop is currently at (or the next one it reaches) observes this
    /// and the loop exits without waiting out the rest of its current
    /// `ping_interval` or `pong_timeout` window. A send error here only
    /// means the loop's `watch::Receiver` was already dropped — i.e. the
    /// task has already exited on its own for some other reason (broadcast
    /// channel closed, bridge writer gone) — which isn't a failure to
    /// react to.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

/// Waits until `rx`'s value becomes `true`, or returns immediately if it
/// already is. Used at each of the heartbeat loop's wait points so a
/// single `shutdown()` call is observable at any/all of them, regardless
/// of which one is current when it's sent — see discussion on
/// `HeartbeatHandle` for why a one-shot wakeup primitive doesn't have this
/// property across multiple sequential wait points.
///
/// Checking `*rx.borrow()` first (rather than only ever calling
/// `changed()`) is what makes this safe to call repeatedly on the same
/// receiver at separate, later wait points: `changed()` alone marks the
/// value "seen" on first observation, which would make a second call on
/// the same receiver block waiting for a value that will never change
/// again (the value is sent at most once, `false → true`). Checking the
/// current value directly has no such one-time consumption.
async fn wait_for_shutdown(rx: &mut watch::Receiver<bool>) {
    while !*rx.borrow() {
        if rx.changed().await.is_err() {
            // Sender dropped without ever sending `true` — treat the same
            // as shutdown, since there is no one left to ask for graceful
            // continuation either way.
            return;
        }
    }
}

/// Spawn the keepalive heartbeat task.
///
/// Creates a background tokio task that periodically sends `Ping{seq}` messages
/// to a worker and waits for matching `Pong{seq}` responses. If a pong is not
/// received within `pong_timeout`, the `on_timeout` callback is invoked.
///
/// The function returns a tuple of the task's `JoinHandle` (for awaiting shutdown)
/// and a `HeartbeatHandle` (for signalling early shutdown).
///
/// # Arguments
///
/// * `worker_id` — The logical worker identity (e.g. `"worker-0"`). Used only
///   for structured logging to distinguish heartbeat logs from other workers.
/// * `tx` — The mpsc sender for `WorkerMessage`. This is the same channel used
///   by the bridge writer task — pings are sent through the shared ROUTER socket.
/// * `event_rx` — The broadcast receiver for `(String, WorkerEvent)` tuples.
///   This is the same channel populated by the bridge reader task — pongs
///   flow through this channel from the ROUTER socket.
/// * `ready_rx` — Resolves once the worker has reported `Ready` (fired by
///   `ManagedWorker::run()`'s `Initializing → Idle` transition). The ping
///   loop below does not start until this resolves, so no ping is ever
///   sent to a worker that hasn't finished initializing yet. If the sender
///   is dropped without firing — the worker hit the ready timeout or
///   exited before ever reporting `Ready` — this resolves to `Err` and the
///   task returns immediately without ever pinging: there is no live
///   worker left to heartbeat.
/// * `ping_interval` — How often to send a Ping message. Default 30 seconds.
///   The interval starts after the pong is received (or timeout fires), not
///   after the ping is sent. This prevents ping storms when the worker is slow.
/// * `pong_timeout` — Maximum time to wait for a matching Pong after sending
///   a Ping. Default 10 seconds. If no matching pong arrives within this
///   window, `on_timeout` is invoked.
/// * `on_timeout` — Callback invoked when a pong timeout fires. The callback
///   is wrapped in `catch_unwind` to prevent a panic from aborting the
///   entire heartbeat task.
///
/// # Returns
///
/// A tuple of `(JoinHandle<()>, HeartbeatHandle)`. The caller should:
/// - Store the `JoinHandle` and await it during shutdown (or drop it if the
///   caller is being dropped and wants the task to be abandoned).
/// - Store the `HeartbeatHandle` and call `shutdown()` when the worker
///   transitions to a terminal state (e.g. `Dead`).
pub fn start(
    worker_id: String,
    tx: mpsc::Sender<WorkerMessage>,
    mut event_rx: broadcast::Receiver<(String, WorkerEvent)>,
    ready_rx: tokio::sync::oneshot::Receiver<()>,
    ping_interval: Duration,
    pong_timeout: Duration,
    on_timeout: impl Fn() + Send + 'static,
) -> (JoinHandle<()>, HeartbeatHandle) {
    // Initially false — the heartbeat is active. See HeartbeatHandle's doc
    // comment for why a watch channel rather than a bare flag: the loop
    // has two sequential wait points that each independently need to
    // observe a single shutdown() call.
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let handle = HeartbeatHandle { shutdown_tx };

    // Spawn the heartbeat task. The task runs an infinite loop that sends
    // pings and waits for pongs, using select! with a per-ping deadline.
    // The task terminates when shutdown is requested or the broadcast
    // channel is closed.
    let worker_id_clone = worker_id.clone();
    let task_handle = tokio::spawn(async move {
        // Wait for the worker to report Ready before sending a single
        // ping. Starting immediately at spawn time — before the Python
        // worker has even imported its node registry — produces pings
        // (and eventually pong-timeout warnings) against a worker that
        // was never given a chance to respond in the first place.
        match ready_rx.await {
            Ok(()) => {
                tracing::debug!(
                    worker_id = %worker_id_clone,
                    "ready signal received, starting heartbeat"
                );
            }
            Err(_) => {
                // The sender was dropped without firing — the worker hit
                // the ready timeout or exited before ever reporting Ready.
                // There is no live worker to ping, so exit now rather than
                // entering the loop below at all.
                tracing::debug!(
                    worker_id = %worker_id_clone,
                    "ready signal never sent, skipping heartbeat"
                );
                return;
            }
        }

        let mut seq: u64 = 0; // Monotonically increasing sequence number for ping/pong matching.

        loop {
            // Check for shutdown at the start of each iteration. This
            // allows ManagedWorker to stop the heartbeat when the worker
            // transitions to Dead or is dropped. A direct borrow check
            // (not changed()) since this call site doesn't want to mark
            // the value seen — wait_for_shutdown() (used at the two wait
            // points below) needs to make its own independent check later
            // in the same iteration.
            if *shutdown_rx.borrow() {
                // Shutdown requested — exit the loop cleanly. The current
                // ping/pong cycle has already completed (or was never
                // started), so no ping is orphaned.
                tracing::debug!(
                    worker_id = %worker_id_clone,
                    "keepalive shutdown requested"
                );
                return;
            }

            seq += 1; // Increment sequence number for this ping cycle.

            // Send the Ping message via the shared mpsc channel. The bridge
            // writer task forwards this to the ROUTER socket, which routes
            // it to the worker. Errors here are non-fatal — the bridge writer
            // may be temporarily unavailable during worker respawn.
            let ping_msg = WorkerMessage::Ping { seq };
            if let Err(e) = tx.send(ping_msg).await {
                // The mpsc channel sender was dropped — the bridge writer
                // has exited. This is a terminal condition for the heartbeat;
                // there is no point sending more pings if the transport is gone.
                tracing::warn!(
                    worker_id = %worker_id_clone,
                    seq = %seq,
                    error = %e,
                    "ping send failed, bridge writer may be down"
                );
                return;
            }

            tracing::debug!(
                worker_id = %worker_id_clone,
                seq = %seq,
                "sending ping"
            );

            // Set the deadline for this ping's pong response. The deadline
            // is computed immediately after the ping is sent, so it represents
            // the maximum time from ping send to pong receive.
            let pong_deadline = Instant::now() + pong_timeout;

            // Enter the select! loop waiting for either a matching pong
            // or a timeout. The deadline sleep is relative to the ping send
            // time, not a fixed interval — this is why we use sleep_until
            // instead of tokio::time::interval.
            loop {
                tokio::select! {
                    // Wait for the next event from the broadcast channel.
                    // The bridge reader task populates this channel with
                    // events received from the ROUTER socket.
                    result = event_rx.recv() => {
                        match result {
                            // Got a valid event — check if it's a matching Pong.
                            Ok((_id, event)) => {
                                // Check if this event is a Pong matching our
                                // current sequence number. Only the exact seq
                                // match counts — older pongs from previous
                                // cycles must be ignored.
                                if let WorkerEvent::Pong { seq: received_seq } = event {
                                    if received_seq == seq {
                                        // Pong matches our current ping — the
                                        // worker is alive. Break out of the
                                        // inner select loop to start the next
                                        // ping cycle.
                                        tracing::debug!(
                                            worker_id = %worker_id_clone,
                                            seq = %seq,
                                            "received pong"
                                        );
                                        break; // Break inner loop, continue to next ping cycle.
                                    }
                                    // Pong seq doesn't match — this is either
                                    // a late pong from a previous cycle or
                                    // a spurious event. Ignore it.
                                }
                                // Non-Pong event (Ready, Dying, etc.) —
                                // not relevant for heartbeat matching.
                            }
                            // Broadcast channel had lagged events — drain them
                            // and continue waiting. This happens when the
                            // heartbeat task falls behind on recv() calls
                            // (e.g. during a long timeout wait).
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                tracing::debug!(
                                    worker_id = %worker_id_clone,
                                    dropped = %n,
                                    "heartbeat dropped lagged events"
                                );
                                // Continue the select loop to recv the next event.
                                continue;
                            }
                            // Broadcast channel closed — no more events will
                            // arrive. This is a terminal condition.
                            Err(broadcast::error::RecvError::Closed) => {
                                // The bridge reader has exited — the worker
                                // is gone. Exit the heartbeat loop cleanly.
                                tracing::debug!(
                                    worker_id = %worker_id_clone,
                                    "broadcast channel closed, stopping heartbeat"
                                );
                                return;
                            }
                        }
                    }
                    // Deadline passed — no pong received in time.
                    // Call the on_timeout callback and break out of the inner
                    // loop so the outer loop's sleep(ping_interval) can execute
                    // before the next ping cycle. Without this break, the inner
                    // loop would spin in a tight loop of immediate timeouts.
                    _ = tokio::time::sleep_until(pong_deadline) => {
                        // Timeout — the worker did not respond to this ping.
                        // Invoke the callback (wrapped in catch_unwind to
                        // prevent a panic from aborting the heartbeat task).
                        tracing::warn!(
                            worker_id = %worker_id_clone,
                            seq = %seq,
                            "pong timeout — worker may be unresponsive"
                        );
                        // Wrap the callback in catch_unwind. A panic in
                        // the timeout handler (e.g. from a callback that
                        // calls panic! or drops_unchecked) should not
                        // abort the entire heartbeat task — we log and
                        // continue to the next ping cycle.
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&on_timeout));
                        if result.is_err() {
                            tracing::error!(
                                worker_id = %worker_id_clone,
                                seq = %seq,
                                "on_timeout callback panicked"
                            );
                        }
                        // Break to the outer loop so the ping_interval sleep
                        // executes before the next ping. This prevents a tight
                        // loop of immediate timeouts (the deadline is still in
                        // the past, so sleep_until would fire again immediately).
                        break;
                    }
                    // Shutdown requested while waiting for a pong — break
                    // out immediately rather than waiting up to
                    // pong_timeout for this inner select to resolve on its
                    // own. Breaking (not returning) is deliberate: it sends
                    // control to the outer loop's top, where the shutdown
                    // check is the single place that actually decides to
                    // exit — consistent with how every other path out of
                    // this inner select already works.
                    _ = wait_for_shutdown(&mut shutdown_rx) => {
                        tracing::debug!(
                            worker_id = %worker_id_clone,
                            seq = %seq,
                            "pong wait interrupted by shutdown signal"
                        );
                        break;
                    }
                }
            }

            // Wait for the next ping interval before sending the next ping,
            // but don't wait out the full interval if shutdown() is called
            // in the meantime — race the sleep against shutdown_rx so the
            // loop wakes immediately and re-checks at the top, rather than
            // potentially sitting in this sleep for the remainder of
            // ping_interval (up to 30s in production) after shutdown was
            // already requested. The interval still starts after the pong
            // is received (or timeout fires), not after the ping is sent,
            // for the same reason as before: to avoid ping storms when the
            // worker is slow to respond.
            tokio::select! {
                _ = tokio::time::sleep(ping_interval) => {}
                _ = wait_for_shutdown(&mut shutdown_rx) => {
                    tracing::debug!(
                        worker_id = %worker_id_clone,
                        "ping_interval sleep interrupted by shutdown signal"
                    );
                }
            }
        }
    });

    (task_handle, handle)
}
