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
//! **Shutdown:** The caller receives a `HeartbeatHandle` alongside the `JoinHandle`.
//! Setting the shutdown flag on the handle causes the loop to exit cleanly on the
//! next ping cycle iteration.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing;

use anvilml_ipc::{WorkerEvent, WorkerMessage};

/// A handle for signalling the heartbeat loop to shut down.
///
/// The caller (typically `ManagedWorker`) stores this handle alongside the
/// `JoinHandle` returned by `start()`. Setting the shutdown flag causes the
/// loop to exit cleanly at the next iteration boundary — the current ping/pong
/// cycle completes before shutdown, ensuring no in-flight ping is orphaned.
///
/// The flag is protected by a `tokio::sync::Mutex` so it can be set from
/// any async context without blocking the tokio runtime.
#[derive(Debug)]
pub struct HeartbeatHandle {
    /// Shutdown flag — when `true`, the heartbeat loop exits after the current
    /// ping/pong cycle completes. The mutex ensures safe concurrent access
    /// from any async task (e.g. `ManagedWorker::shutdown`).
    shutdown: Arc<Mutex<bool>>,
}

impl HeartbeatHandle {
    /// Signal the heartbeat loop to shut down after the current ping/pong cycle.
    ///
    /// This is a non-blocking operation — it sets the flag and returns
    /// immediately. The loop checks the flag at the start of each iteration,
    /// so shutdown takes effect at the next ping cycle boundary.
    pub async fn shutdown(&self) {
        let mut flag = self.shutdown.lock().await;
        *flag = true; // Mark shutdown to stop the heartbeat loop on next cycle.
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
    ping_interval: Duration,
    pong_timeout: Duration,
    on_timeout: impl Fn() + Send + 'static,
) -> (JoinHandle<()>, HeartbeatHandle) {
    // Create the shared shutdown flag. Initially false — the heartbeat is
    // active. The mutex allows setting the flag from any async context
    // (e.g. ManagedWorker::shutdown) without blocking the runtime.
    let shutdown = Arc::new(Mutex::new(false));
    let handle = HeartbeatHandle {
        shutdown: Arc::clone(&shutdown),
    };

    // Spawn the heartbeat task. The task runs an infinite loop that sends
    // pings and waits for pongs, using select! with a per-ping deadline.
    // The task terminates when the shutdown flag is set or the broadcast
    // channel is closed.
    let worker_id_clone = worker_id.clone();
    let handle_clone = shutdown.clone();
    let task_handle = tokio::spawn(async move {
        let mut seq: u64 = 0; // Monotonically increasing sequence number for ping/pong matching.

        loop {
            // Check for shutdown signal at the start of each iteration.
            // This allows ManagedWorker to stop the heartbeat when the
            // worker transitions to Dead or is dropped.
            {
                let flag = handle_clone.lock().await;
                if *flag {
                    // Shutdown requested — exit the loop cleanly. The current
                    // ping/pong cycle has already completed (or was never
                    // started), so no ping is orphaned.
                    tracing::debug!(
                        worker_id = %worker_id_clone,
                        "keepalive shutdown requested"
                    );
                    return;
                }
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
                }
            }

            // Wait for the next ping interval before sending the next ping.
            // The interval starts after the pong is received (or timeout fires),
            // not after the ping is sent. This prevents ping storms when the
            // worker is slow to respond.
            tokio::time::sleep(ping_interval).await;
        }
    });

    (task_handle, handle)
}
