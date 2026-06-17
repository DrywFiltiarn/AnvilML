//! Managed worker — a state machine that supervises a Python worker subprocess.
//!
//! `ManagedWorker` owns the lifecycle of a Python worker subprocess, including:
//! - Spawning the subprocess with proper environment variables and stdio piping
//! - Sending IPC messages via the bridge writer task (see `crate::bridge`)
//! - Heartbeat keepalive with timeout watchdog
//! - State machine transitions driven by events from the worker
//! - Clean shutdown of all spawned tasks
//!
//! The state machine tracks the worker's lifecycle state (`WorkerStatus`) and
//! transitions between states based on events received from the worker:
//! `Initializing` → `Idle` (on Ready), `Idle` → `Dead` (on Dying),
//! `Busy` → `Idle` (on Completed/Failed/Cancelled), `Busy` → `Dead` (on Dying).
//!
//! `ManagedWorker` does not read events off the transport itself — a single
//! pool-wide demux task (`crate::demux`) owns the only `RouterTransport::recv()`
//! call and forwards each worker's events into that worker's `event_tx`. See
//! `crate::demux` for why a per-worker reader was unsound.
//!
//! Join handles are `Option<JoinHandle>` so `shutdown()` can take ownership
//! and drop them to abort the task, since `tokio::JoinHandle` has no `Clone`.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::{AnvilError, GpuDevice, ServerConfig, WorkerStatus};
use anvilml_ipc::{RouterTransport, WorkerEvent};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing;

use crate::bridge;
use crate::keepalive::{self, HeartbeatHandle};
use crate::respawn::RespawnPolicy;
use crate::spawn::build_command;

/// A message sent from the Rust supervisor to a Python worker subprocess.
///
/// Re-exported from `anvilml_ipc` for use in the bridge channel type.
use anvilml_ipc::WorkerMessage;

/// Everything `WorkerPool::spawn_all` needs to register a freshly spawned
/// worker in the pool-wide demux routing table.
///
/// Returned alongside `ManagedWorker` from `spawn()` rather than stored as a
/// field on `ManagedWorker` itself, since nothing past the moment of
/// construction needs the IPC identity bytes or a spare `event_tx` clone
/// again — keeping them out of the struct avoids carrying state for the
/// worker's entire lifetime to serve a single post-construction read.
pub struct RouteInfo {
    /// The raw ZMQ routing identity, matching `ANVILML_WORKER_ID`.
    pub ipc_identity: Vec<u8>,
    /// The human-readable display label (e.g. `"worker-0"`).
    pub display_id: String,
    /// A clone of this worker's event broadcast sender. The demux task
    /// forwards events addressed to `ipc_identity` into this channel.
    pub event_tx: broadcast::Sender<(String, WorkerEvent)>,
}

/// A managed worker subprocess with state machine supervision.
///
/// Created via `spawn()` (launches the subprocess) or `new()` (pre-built
/// channels, for testing). Consumed by `run()` (the event loop) or
/// `shutdown()` (clean termination).
///
/// # Shutdown sequence
///
/// `shutdown()` performs the following steps in order:
/// 1. Signal the heartbeat to stop via `HeartbeatHandle::shutdown()`
/// 2. Send a `Shutdown` message to the bridge (best-effort)
/// 3. Drop `msg_tx` to signal the bridge writer to exit
/// 4. Drop the bridge handle to abort the writer task if it hasn't exited
/// 5. Drop the keepalive handle to abort the keepalive task
#[allow(dead_code)] // child, device_index, and respawn_policy are for future respawn logic (P10-A1)
#[derive(Debug)]
pub struct ManagedWorker {
    /// Read-write locked so the run loop can write transitions while other
    /// tasks (e.g. the pool's status-change monitor) read concurrently.
    #[cfg_attr(test, allow(dead_code))] // tests need access to verify state transitions
    pub(crate) status: Arc<tokio::sync::RwLock<WorkerStatus>>,

    /// Sender for the message channel. The bridge writer task receives from
    /// the corresponding receiver and forwards messages to the transport.
    msg_tx: mpsc::Sender<WorkerMessage>,

    /// Broadcast sender for events addressed to this worker. The demux task
    /// holds the matching clone it forwards into; this struct's own copy is
    /// dropped at the start of `run()` so the channel can close once the
    /// demux task's clone is the only one left.
    event_tx: broadcast::Sender<(String, WorkerEvent)>,

    /// `None` when constructed via `new()` for testing without a real
    /// subprocess.
    child: Option<tokio::process::Child>,

    /// `Option`-wrapped so `shutdown()` can take ownership and await it.
    /// There is no reader handle here — see the module docs for why a
    /// single pool-wide demux task replaced the per-worker reader.
    bridge_handle: Option<JoinHandle<()>>,

    /// Handle for the keepalive heartbeat task. `Option`-wrapped for shutdown.
    keepalive_handle: Option<JoinHandle<()>>,

    /// Handle for signalling the heartbeat loop to shut down.
    heartbeat_handle: Option<HeartbeatHandle>,

    /// Controls how dead workers are respawned (logic deferred to P10-A1).
    respawn_policy: RespawnPolicy,

    /// The worker's stable identity string (e.g. `"worker-0"`).
    /// Used for structured logging throughout the worker's lifecycle.
    worker_id: String,

    /// The GPU device name. Updated from the `Ready` event in `run()`,
    /// since the worker's actual reported name may differ from the
    /// initial value from `GpuDevice` (e.g. auto-detected vs. configured).
    device_name: String,

    /// The GPU device index. Updated from the `Ready` event in `run()`,
    /// for the same reason as `device_name`.
    device_index: u32,
}
impl ManagedWorker {
    /// Returns a clone of the status `Arc` so callers can observe state
    /// transitions without holding the worker itself — used by the pool's
    /// status-change monitor and by tests asserting on transitions.
    pub fn get_status(&self) -> Arc<tokio::sync::RwLock<WorkerStatus>> {
        self.status.clone()
    }

    /// Construct a `ManagedWorker` from pre-built channels and handles,
    /// bypassing subprocess spawning. Used by tests; production code uses
    /// `spawn()`.
    ///
    /// # Arguments
    ///
    /// * `status` — The worker's initial state (typically `Initializing`).
    /// * `msg_tx` — The message channel sender.
    /// * `event_tx` — The event broadcast sender.
    /// * `child` — The subprocess child, if spawned. `None` for tests.
    /// * `bridge_handle` — Handle for the bridge writer task.
    /// * `keepalive_handle` — Handle for the keepalive heartbeat task.
    /// * `heartbeat_handle` — Handle for signalling heartbeat shutdown.
    /// * `worker_id` — The worker's stable identity string.
    /// * `device_name` — The GPU device name (populated from `Ready` event in production).
    /// * `device_index` — The GPU device index (populated from `Ready` event in production).
    #[allow(clippy::too_many_arguments)] // test constructor with many parameters
    pub fn new(
        status: WorkerStatus,
        msg_tx: mpsc::Sender<WorkerMessage>,
        event_tx: broadcast::Sender<(String, WorkerEvent)>,
        child: Option<tokio::process::Child>,
        bridge_handle: Option<JoinHandle<()>>,
        keepalive_handle: Option<JoinHandle<()>>,
        heartbeat_handle: Option<HeartbeatHandle>,
        worker_id: String,
        device_name: String,
        device_index: u32,
    ) -> Self {
        Self {
            status: Arc::new(tokio::sync::RwLock::new(status)),
            msg_tx,
            event_tx,
            child,
            bridge_handle,
            keepalive_handle,
            heartbeat_handle,
            respawn_policy: RespawnPolicy::default(),
            worker_id,
            device_name,
            device_index,
        }
    }
    /// Spawn a Python worker subprocess and return a `ManagedWorker` plus
    /// the `RouteInfo` its caller must register with the pool's demux task.
    ///
    /// # Arguments
    ///
    /// * `cfg` — The server configuration (provides venv path and IPC payload cap).
    /// * `device` — The GPU device this worker will operate on.
    /// * `transport` — The shared `RouterTransport` for IPC communication.
    /// * `worker_id` — The worker's stable display identity (e.g.
    ///   `"worker-0"`), used for logging, `WorkerInfo`, and WebSocket
    ///   broadcasts — never the IPC routing key, since it won't match the
    ///   ZMQ identity the Python worker actually registers (see
    ///   `ANVILML_WORKER_ID` in `build_worker_env()`).
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the subprocess fails to spawn.
    ///
    /// # Subprocess lifecycle
    ///
    /// On Linux, `PR_SET_PDEATHSIG` is set so the worker is killed if the
    /// parent supervisor dies.
    #[tracing::instrument(skip(cfg, device, transport), fields(worker_id, device_index = %device.index))]
    pub async fn spawn(
        cfg: &ServerConfig,
        device: &GpuDevice,
        transport: Arc<RouterTransport>,
        worker_id: String,
    ) -> Result<(Self, RouteInfo), AnvilError> {
        // The port is the transport's bound port, not a separately
        // configured value — every worker shares the one ROUTER socket.
        let port = transport.port;
        let mut cmd = build_command(cfg, device, port);

        let child = cmd.spawn().map_err(AnvilError::Io)?;

        // Starts Initializing; only the Ready event (handled in run())
        // advances it, so a worker that never reports Ready stays here
        // until the run loop's ready-timeout kills it.
        let status = Arc::new(tokio::sync::RwLock::new(WorkerStatus::Initializing));

        // Bounded at 16 so a wedged bridge applies backpressure to senders
        // (keepalive, shutdown) rather than growing without limit.
        let (msg_tx, msg_rx) = mpsc::channel(16);

        // Bounded at 16 so a slow state machine can lag briefly without
        // the demux task blocking on a full channel.
        let (event_tx, _event_rx) = broadcast::channel(16);

        // The routing identity must be the bare device index, matching
        // ANVILML_WORKER_ID — not worker_id, the display label. Using the
        // display label here would make every send() fail with
        // "Destination client not found by identity", since ZMQ never
        // registered that string. bridge::start is writer-only; the demux
        // task (started later by WorkerPool, using RouteInfo below) is the
        // only reader of this worker's events — see crate::demux.
        let ipc_identity_bytes = device.index.to_string().into_bytes();
        let bridge_handle = Some(bridge::start(
            transport.clone(),
            ipc_identity_bytes.clone(),
            worker_id.clone(),
            msg_rx,
        ));

        // Weak reference avoids a borrow cycle: status is owned by
        // ManagedWorker, and this callback is stored inside the keepalive
        // task — a strong reference would mean neither could ever be
        // dropped while the other lives.
        let status_weak = Arc::downgrade(&status);
        let worker_id_for_callback = worker_id.clone();
        let on_timeout = move || {
            let status_weak = status_weak.clone();
            let worker_id = worker_id_for_callback.clone();
            tracing::info!(
                worker_id = %worker_id,
                "keepalive timeout — spawning status transition task"
            );
            // on_timeout is Fn(), but transitioning status needs an async
            // write lock — spawning a task is the only way to bridge that.
            tokio::spawn(async move {
                if let Some(s) = status_weak.upgrade() {
                    *s.write().await = WorkerStatus::Dead;
                }
            });
        };

        let (keepalive_handle, heartbeat_handle) = keepalive::start(
            worker_id.clone(),
            msg_tx.clone(),
            _event_rx,
            Duration::from_secs(30), // ping_interval
            Duration::from_secs(10), // pong_timeout
            on_timeout,
        );

        let device_name = device.name.clone();

        // Mandatory INFO log point per ENVIRONMENT.md §9; worker_id,
        // device_index, and pid are indexed by log aggregators.
        tracing::info!(
            worker_id = %worker_id,
            device_index = %device.index,
            pid = %child.id().unwrap_or(0),
            "worker spawned"
        );

        // event_tx is cloned a third time here (bridge::start no longer
        // takes a clone, but the struct field and RouteInfo each still
        // need their own) so WorkerPool can hand RouteInfo's clone to the
        // demux task without reaching into the struct after construction.
        let route_info = RouteInfo {
            ipc_identity: ipc_identity_bytes,
            display_id: worker_id.clone(),
            event_tx: event_tx.clone(),
        };

        Ok((
            Self {
                status,
                msg_tx,
                event_tx,
                child: Some(child),
                bridge_handle,
                keepalive_handle: Some(keepalive_handle),
                heartbeat_handle: Some(heartbeat_handle),
                respawn_policy: RespawnPolicy::default(),
                worker_id,
                device_name,
                device_index: device.index,
            },
            route_info,
        ))
    }
    /// Run the managed worker's state machine event loop, consuming `self`.
    ///
    /// # State transitions
    ///
    /// | Current State | Event              | New State  |
    /// |---------------|--------------------|------------|
    /// | Initializing  | Ready              | Idle       |
    /// | Initializing  | (any other)        | Initializing |
    /// | Idle          | Dying              | Dead       |
    /// | Idle          | (any other)        | Idle       |
    /// | Busy          | Completed          | Idle       |
    /// | Busy          | Failed             | Idle       |
    /// | Busy          | Cancelled          | Idle       |
    /// | Busy          | Dying              | Dead       |
    /// | Busy          | (any other)        | Busy       |
    /// | Dead          | (any)              | Dead       |
    /// | Respawning    | (any)              | Dead       |
    ///
    /// # Ready timeout
    ///
    /// If no `Ready` event is received within 60 seconds, the worker is
    /// considered unresponsive and transitions to `Dead`. This prevents
    /// workers from hanging indefinitely in the `Initializing` state.
    #[tracing::instrument(skip(self), fields(worker_id = %self.worker_id))]
    pub async fn run(mut self) {
        let mut event_rx = self.event_tx.subscribe();

        // In production, the pool's demux task (crate::demux) holds the
        // only remaining clone of this sender and forwards this worker's
        // events into it — when the demux task exits, the channel closes
        // and the loop below exits. In tests constructing ManagedWorker
        // directly, the test itself holds the only remaining sender.
        drop(self.event_tx);

        // The loop's only exit condition is the broadcast channel closing
        // (or an unexpected child exit, handled in the select below) —
        // ready_timeout is re-armed each iteration rather than once, since
        // it must stop firing the moment the worker leaves Initializing.
        loop {
            tracing::debug!(worker_id = %self.worker_id, "run loop iteration");

            // Re-read status each iteration: once it's no longer
            // Initializing, the timeout must never fire again, or a worker
            // sitting Idle for hours would be killed by a stale 60s timer.
            let mut ready_timeout = if *self.status.read().await == WorkerStatus::Initializing {
                Some(tokio::time::sleep(Duration::from_secs(60)))
            } else {
                None
            };

            tokio::select! {
                // Both branches of this async block must be the same
                // future type for tokio::select! — Duration::MAX sleep is
                // the "effectively never" placeholder for the None case,
                // not a real timeout (≈292,471 years).
                _ = async {
                    if let Some(sleep) = ready_timeout.take() {
                        // tokio::time::Sleep is !Unpin, so pin! is required
                        // before this can be awaited by value.
                        std::pin::pin!(sleep).as_mut().await;
                    } else {
                        tokio::time::sleep(std::time::Duration::MAX).await;
                    }
                } => {
                    tracing::warn!(
                        worker_id = %self.worker_id,
                        "ready timeout, worker dead"
                    );
                    *self.status.write().await = WorkerStatus::Dead;
                }

                result = event_rx.recv() => {
                    match result {
                        Ok((_id, event)) => {
                            tracing::debug!(
                                worker_id = %self.worker_id,
                                event_type = ?event,
                                "processing event in run loop"
                            );
                            let _current_status = *self.status.read().await;
                            let _device_name = match &event {
                                WorkerEvent::Ready {
                                    device_name,
                                    device_index,
                                    ..
                                } => {
                                    // The worker's actual device_name/index
                                    // may differ from GpuDevice's initial
                                    // values (e.g. auto-detected hardware),
                                    // so Ready's reported values take over
                                    // for all subsequent logging.
                                    self.device_name = device_name.clone();
                                    self.device_index = *device_index;
                                    tracing::debug!(
                                        worker_id = %self.worker_id,
                                        event_type = ?event,
                                        "processing event"
                                    );
                                    let mut s = self.status.write().await;
                                    match *s {
                                        WorkerStatus::Initializing => {
                                            // This is the Rust/Python sync
                                            // point: the worker can now
                                            // accept jobs.
                                            *s = WorkerStatus::Idle;
                                            tracing::info!(
                                                worker_id = %self.worker_id,
                                                device = %device_name,
                                                "worker reached Ready"
                                            );
                                            Some(device_name.clone())
                                        }
                                        _ => {
                                            Some(device_name.clone())
                                        }
                                    }
                                }
                                _ => {
                                    tracing::debug!(
                                        worker_id = %self.worker_id,
                                        event_type = ?event,
                                        "processing event"
                                    );
                                    let mut s = self.status.write().await;
                                    match *s {
                                        WorkerStatus::Idle => {
                                            if matches!(&event, WorkerEvent::Dying { .. }) {
                                                *s = WorkerStatus::Dead;
                                            }
                                        }
                                        WorkerStatus::Busy => {
                                            match &event {
                                                WorkerEvent::Completed { .. } |
                                                WorkerEvent::Failed { .. } |
                                                WorkerEvent::Cancelled { .. } => {
                                                    *s = WorkerStatus::Idle;
                                                }
                                                WorkerEvent::Dying { .. } => {
                                                    *s = WorkerStatus::Dead;
                                                }
                                                _ => {}
                                            }
                                        }
                                        WorkerStatus::Dead | WorkerStatus::Respawning => {
                                            // Terminal: nothing transitions
                                            // a worker back out of these.
                                        }
                                        WorkerStatus::Initializing => {
                                            // Only Ready (handled above)
                                            // moves out of this state.
                                        }
                                    }
                                    None
                                }
                            };
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // The demux task exited, so no more events for
                            // any worker can ever arrive on this channel —
                            // terminal, not retryable.
                            tracing::info!(
                                worker_id = %self.worker_id,
                                "broadcast channel closed, worker gone"
                            );
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            // Dropped events aren't reconstructable, but
                            // state-machine correctness only depends on the
                            // worker's current status, not on having seen
                            // every intermediate event.
                            tracing::debug!(
                                worker_id = %self.worker_id,
                                dropped = %n,
                                "managed dropped lagged events"
                            );
                        }
                    }
                }

                // child is None in tests, where this arm must never win —
                // a never-firing sleep keeps it inert without needing a
                // separate code path per child-present/absent case.
                _ = async {
                    match self.child.as_mut() {
                        Some(child) => {
                            let _ = child.wait().await;
                        }
                        None => {
                            tokio::time::sleep(std::time::Duration::MAX).await;
                        }
                    }
                } => {
                    // TODO(P14): notify JobScheduler of in-flight job failure
                    // once it exists. The scheduler is introduced in P13-A3 and
                    // wired to dispatch in P14-A1.
                    let exit_code = match self.child.as_mut().and_then(|c| c.try_wait().ok()).flatten() {
                        Some(status) => status.code(),
                        None => None,
                    };
                    *self.status.write().await = WorkerStatus::Dead;
                    tracing::info!(
                        worker_id = %self.worker_id,
                        exit_code = ?exit_code,
                        "worker exited unexpectedly"
                    );
                    // Can't broadcast a Dying event here: self.event_tx was
                    // already dropped above, and the demux task holds the
                    // only remaining clone. The next GET /v1/workers poll
                    // will observe the Dead status instead.
                    break;
                }
            }
        }

        // The child subprocess isn't waited on here — if it's still alive,
        // it's either exiting on its own or will be reaped when self drops.
        drop(self.bridge_handle);
        drop(self.keepalive_handle);
        drop(self.heartbeat_handle);

        tracing::info!(
            worker_id = %self.worker_id,
            "worker run loop ended"
        );
    }
    /// Shut down the managed worker cleanly, consuming `self`.
    ///
    /// 1. Signal the heartbeat to stop via `HeartbeatHandle::shutdown()`
    /// 2. Send a `Shutdown` message to the bridge (best-effort, may fail
    ///    if the channel is already closed)
    /// 3. Drop `msg_tx` to signal the bridge writer to exit
    /// 4. Await the bridge writer (bounded), so `Shutdown` is actually
    ///    transmitted before this function returns
    /// 5. Drop the keepalive handle to abort the keepalive task
    /// 6. Wait up to 5 seconds for the subprocess to exit on its own, then
    ///    force-kill it — dropping a `tokio::process::Child` does not
    ///    terminate the OS process, only the Rust handle to it
    #[tracing::instrument(skip(self), fields(worker_id = %self.worker_id))]
    pub async fn shutdown(mut self) {
        // Stop pinging before the worker is told to shut down, so no ping
        // races the Shutdown message sent below.
        if let Some(handle) = &self.heartbeat_handle {
            handle.shutdown().await;
        }

        // Best-effort: the writer drains this from the channel below
        // regardless of whether this particular send() succeeds.
        self.msg_tx.send(WorkerMessage::Shutdown).await.ok();

        // The writer's `while let Some(msg) = msg_rx.recv().await` only
        // exits once every sender is dropped AND the buffer is drained —
        // so dropping here cannot discard the Shutdown message just sent.
        drop(self.msg_tx);

        // A bare send().await only guarantees the message reached the
        // channel buffer, not that the writer task has since been
        // scheduled to pull it off and call transport.send() — tokio's
        // scheduler is cooperative, and nothing else in this function
        // naturally yields long enough for that to happen on its own.
        // Awaiting the handle (bounded, in case the writer is itself
        // stuck on a bad transport) is what actually guarantees delivery
        // before shutdown proceeds; previously this just dropped the
        // handle immediately, which aborts nothing and guarantees nothing.
        if let Some(writer_handle) = self.bridge_handle.take() {
            if tokio::time::timeout(Duration::from_secs(2), writer_handle)
                .await
                .is_err()
            {
                tracing::warn!(
                    worker_id = %self.worker_id,
                    "bridge writer task did not finish within grace period during shutdown"
                );
            }
            // No reader handle to abort here — the demux task is
            // pool-wide and outlives any single worker's shutdown.
            // WorkerPool aborts it once, after every worker has stopped.
        }

        drop(self.keepalive_handle);

        // Without this wait, the OS process survives the supervisor
        // exiting: dropping tokio::process::Child only drops Rust's
        // handle, never the underlying process. This is also why
        // with_graceful_shutdown alone was insufficient — it drains HTTP
        // connections but has no awareness of worker subprocesses at all.
        if let Some(mut child) = self.child.take() {
            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => {
                    tracing::debug!(
                        worker_id = %self.worker_id,
                        exit_code = ?status.code(),
                        "worker subprocess exited cleanly during shutdown"
                    );
                }
                Ok(Err(err)) => {
                    tracing::warn!(
                        worker_id = %self.worker_id,
                        error = %err,
                        "error waiting on worker subprocess during shutdown"
                    );
                }
                Err(_) => {
                    // Grace period elapsed: force-kill so the process
                    // can't outlive the supervisor that's exiting.
                    tracing::warn!(
                        worker_id = %self.worker_id,
                        "worker subprocess did not exit within grace period, killing"
                    );
                    if let Err(err) = child.kill().await {
                        tracing::warn!(
                            worker_id = %self.worker_id,
                            error = %err,
                            "failed to kill worker subprocess"
                        );
                    }
                }
            }
        }

        tracing::debug!(
            worker_id = %self.worker_id,
            "worker shutdown"
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the spawned task in the keepalive callback successfully
    /// updates the worker status. This is a regression test for the
    /// mechanism where the synchronous on_timeout callback spawns an async
    /// task that acquires the write lock and sets the status to Dead.
    #[tokio::test]
    async fn test_spawned_task_updates_status() {
        let status = Arc::new(tokio::sync::RwLock::new(WorkerStatus::Idle));
        let weak = Arc::downgrade(&status);

        let callback_fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback_fired_clone = Arc::clone(&callback_fired);

        let on_timeout = move || {
            let weak = weak.clone();
            callback_fired.store(true, std::sync::atomic::Ordering::SeqCst);
            tokio::spawn(async move {
                if let Some(s) = weak.upgrade() {
                    *s.write().await = WorkerStatus::Dead;
                }
            });
        };

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
        });

        on_timeout();

        let _ = handle.await;

        assert!(
            callback_fired_clone.load(std::sync::atomic::Ordering::SeqCst),
            "callback should have fired"
        );

        let final_status = *status.read().await;
        assert_eq!(
            final_status,
            WorkerStatus::Dead,
            "status should be Dead, got {:?}",
            final_status
        );
    }

    /// Verify that the ManagedWorker processes events from the broadcast channel.
    ///
    /// This test creates a ManagedWorker in Initializing status, spawns run(),
    /// sends a Ready event, and verifies the status transitions to Idle.
    #[tokio::test]
    async fn test_managed_worker_processes_ready_event() {
        let (msg_tx, _msg_rx) = mpsc::channel(16);
        let (event_tx, _event_rx) = broadcast::channel(16);

        let worker = ManagedWorker::new(
            WorkerStatus::Initializing,
            msg_tx,
            event_tx.clone(),
            None,
            None, // bridge_handle
            None,
            None,
            "test-worker".to_string(),
            "test-device".to_string(),
            0, // device_index
        );

        let status = worker.get_status();

        let run_handle = tokio::spawn(worker.run());

        // run() must reach event_rx.subscribe() before this send, or the
        // event is published with no subscriber yet and lost.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let ready_event = WorkerEvent::Ready {
            worker_id: "test-worker".to_string(),
            device_index: 0,
            device_name: "test-device".to_string(),
            device_type: "cpu".to_string(),
            vram_total_mib: 8192,
            vram_free_mib: 8000,
            torch_version: "2.4.0".to_string(),
            fp16: true,
            bf16: true,
            fp8: false,
            flash_attention: false,
            node_types: Vec::new(),
        };
        let _ = event_tx.send(("test-worker".to_string(), ready_event));

        tokio::time::sleep(Duration::from_millis(200)).await;

        let final_status = *status.read().await;
        assert_eq!(
            final_status,
            WorkerStatus::Idle,
            "status should be Idle after Ready event, got {:?}",
            final_status
        );

        // Dropping the only remaining sender is what lets run()'s loop
        // observe RecvError::Closed and exit.
        drop(event_tx);
        let _ = run_handle.await;
    }

    /// Verify that the ManagedWorker processes Completed events from Busy state.
    #[tokio::test]
    async fn test_managed_worker_processes_completed_event() {
        let (msg_tx, _msg_rx) = mpsc::channel(16);
        let (event_tx, _event_rx) = broadcast::channel(16);

        let worker = ManagedWorker::new(
            WorkerStatus::Idle,
            msg_tx,
            event_tx.clone(),
            None,
            None, // bridge_handle
            None,
            None,
            "test-worker".to_string(),
            "test-device".to_string(),
            0, // device_index
        );

        let status = worker.get_status();

        let run_handle = tokio::spawn(worker.run());

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Simulates job dispatch, which normally happens via the
        // scheduler — there is no dispatch path to call directly here.
        {
            let mut s = status.write().await;
            *s = WorkerStatus::Busy;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        let completed_event = WorkerEvent::Completed {
            job_id: uuid::Uuid::new_v4(),
            elapsed_ms: 5000,
        };
        let _ = event_tx.send(("test-worker".to_string(), completed_event));

        tokio::time::sleep(Duration::from_millis(500)).await;

        let final_status = *status.read().await;
        assert_eq!(
            final_status,
            WorkerStatus::Idle,
            "status should be Idle after Completed event, got {:?}",
            final_status
        );

        drop(event_tx);
        let _ = run_handle.await;
    }
}
