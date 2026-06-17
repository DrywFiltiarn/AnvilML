//! Managed worker — a state machine that supervises a Python worker subprocess.
//!
//! `ManagedWorker` owns the lifecycle of a Python worker subprocess, including:
//! - Spawning the subprocess with proper environment variables and stdio piping
//! - Routing IPC messages via the bridge (two independent tokio tasks)
//! - Heartbeat keepalive with timeout watchdog
//! - State machine transitions driven by events from the worker
//! - Clean shutdown of all spawned tasks
//!
//! The state machine tracks the worker's lifecycle state (`WorkerStatus`) and
//! transitions between states based on events received from the worker:
//! `Initializing` → `Idle` (on Ready), `Idle` → `Dead` (on Dying),
//! `Busy` → `Idle` (on Completed/Failed/Cancelled), `Busy` → `Dead` (on Dying).
//!
//! **Hard constraints:** All join handles are `Option<JoinHandle>` to allow
//! `shutdown()` to take ownership and abort the task by dropping the handle.
//! `tokio::JoinHandle` does not implement `Clone`, so `Option` wrapping
//! enables ownership transfer without requiring `AbortHandle`.

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

/// A managed worker subprocess with state machine supervision.
///
/// `ManagedWorker` owns the lifecycle of a Python worker subprocess. It manages:
/// - The subprocess child process (`child`)
/// - The IPC bridge (two tokio tasks for message routing)
/// - The keepalive heartbeat task
/// - The heartbeat shutdown handle
/// - The worker's current state
/// - Channels for message passing and event broadcasting
/// - The respawn policy (for future respawn logic)
///
/// The worker is created via `spawn()` (which launches the subprocess) or
/// `new()` (for testing with pre-built channels). It is consumed by `run()`
/// (the main event loop) or `shutdown()` (clean termination).
///
/// # Shutdown sequence
///
/// `shutdown()` performs the following steps in order:
/// 1. Signal the heartbeat to stop via `HeartbeatHandle::shutdown()`
/// 2. Drop `msg_tx` to signal the bridge writer to exit
/// 3. Drop bridge handles to abort the bridge tasks
/// 4. Drop the keepalive handle to abort the keepalive task
/// 5. Send a `Shutdown` message to the bridge (best-effort)
///
/// All fields are `Option`-wrapped join handles to allow `shutdown()` to
/// take ownership and abort tasks by dropping the handle.
#[allow(dead_code)] // child, device_index, and respawn_policy are for future respawn logic (P10-A1)
#[derive(Debug)]
pub struct ManagedWorker {
    /// The worker's current lifecycle state, protected by a read-write lock
    /// to allow concurrent reads and exclusive writes from different tasks.
    #[cfg_attr(test, allow(dead_code))] // tests need access to verify state transitions
    pub(crate) status: Arc<tokio::sync::RwLock<WorkerStatus>>,

    /// Sender for the message channel. The bridge writer task receives from
    /// the corresponding receiver and forwards messages to the transport.
    msg_tx: mpsc::Sender<WorkerMessage>,

    /// Broadcast sender for events received from the transport. Events are
    /// delivered to all subscribers (e.g., the run loop's state machine).
    event_tx: broadcast::Sender<(String, WorkerEvent)>,

    /// The subprocess child process, if spawned. `None` when using `new()`
    /// for testing without subprocess spawning.
    child: Option<tokio::process::Child>,

    /// Handles for the bridge writer and reader tasks. Both are `Option`-wrapped
    /// to allow `shutdown()` to take ownership and abort the tasks.
    bridge_handles: Option<(JoinHandle<()>, JoinHandle<()>)>,

    /// Handle for the keepalive heartbeat task. `Option`-wrapped for shutdown.
    keepalive_handle: Option<JoinHandle<()>>,

    /// Handle for signalling the heartbeat loop to shut down.
    heartbeat_handle: Option<HeartbeatHandle>,

    /// The respawn policy for this worker. Controls how dead workers are respawned.
    respawn_policy: RespawnPolicy,

    /// The worker's stable identity string (e.g. `"worker-0"`).
    /// Used for structured logging throughout the worker's lifecycle.
    worker_id: String,

    /// The GPU device name (e.g. `"NVIDIA RTX 4090"`). Populated from the
    /// `Ready` event during the run loop. Used for logging when the worker
    /// reaches Ready state.
    device_name: String,

    /// The GPU device index (zero-based, as reported by the OS/driver).
    /// Populated from the `Ready` event during the run loop. Used for
    /// structured logging when reporting worker status changes.
    device_index: u32,
}

/// A message sent from the Rust supervisor to a Python worker subprocess.
///
/// Re-exported from `anvilml_ipc` for use in the bridge channel type.
use anvilml_ipc::WorkerMessage;

impl ManagedWorker {
    /// Create a `ManagedWorker` with pre-built channels and handles.
    ///
    /// This constructor is intended for testing, where the caller provides
    /// the channels and handles directly, bypassing subprocess spawning.
    /// The worker is created in the `Initializing` state.
    ///
    /// # Arguments
    ///
    /// * `status` — The worker's initial state (typically `Initializing`).
    /// * `msg_tx` — The message channel sender.
    /// * `event_tx` — The event broadcast sender.
    /// * `child` — The subprocess child, if spawned. `None` for tests.
    /// * `bridge_handles` — Handles for the bridge writer and reader tasks.
    /// * `keepalive_handle` — Handle for the keepalive heartbeat task.
    /// * `heartbeat_handle` — Handle for signalling heartbeat shutdown.
    /// * `worker_id` — The worker's stable identity string.
    /// * `device_name` — The GPU device name (populated from `Ready` event in production).
    /// * `device_index` — The GPU device index (populated from `Ready` event in production).
    #[allow(clippy::too_many_arguments)] // test constructor with many parameters
    pub fn get_status(&self) -> Arc<tokio::sync::RwLock<WorkerStatus>> {
        // Get a clone of the status Arc for external status inspection.
        // This is primarily intended for testing, allowing callers to
        // observe the worker's current state without holding the worker.
        self.status.clone()
    }

    #[allow(clippy::too_many_arguments)] // test constructor with many parameters
    pub fn new(
        status: WorkerStatus,
        msg_tx: mpsc::Sender<WorkerMessage>,
        event_tx: broadcast::Sender<(String, WorkerEvent)>,
        child: Option<tokio::process::Child>,
        bridge_handles: Option<(JoinHandle<()>, JoinHandle<()>)>,
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
            bridge_handles,
            keepalive_handle,
            heartbeat_handle,
            respawn_policy: RespawnPolicy::default(),
            worker_id,
            device_name,
            device_index,
        }
    }

    /// Spawn a Python worker subprocess and return a `ManagedWorker` to supervise it.
    ///
    /// This is the primary entry point for creating a managed worker in production.
    /// It constructs a subprocess command via `build_command()`, spawns the child
    /// process, sets up IPC channels, spawns the bridge and keepalive tasks, and
    /// returns a `ManagedWorker` in the `Initializing` state.
    ///
    /// # Arguments
    ///
    /// * `cfg` — The server configuration (provides venv path and IPC payload cap).
    /// * `device` — The GPU device this worker will operate on.
    /// * `transport` — The shared `RouterTransport` for IPC communication.
    /// * `worker_id` — The worker's stable display identity string (e.g.
    ///   `"worker-0"`), used for logging, `WorkerInfo`, and WebSocket
    ///   broadcasts. This is NOT used as the IPC routing key — the bridge
    ///   is given `device.index.to_string()` instead, since that is what
    ///   matches the ZMQ identity the Python worker registers (see
    ///   `ANVILML_WORKER_ID` in `build_worker_env()`).
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the subprocess fails to spawn.
    ///
    /// # Subprocess lifecycle
    ///
    /// The subprocess is spawned with stdout/stderr piped for log capture.
    /// On Linux, `PR_SET_PDEATHSIG` is set so the worker is killed if the
    /// parent supervisor dies.
    #[tracing::instrument(skip(cfg, device, transport), fields(worker_id, device_index = %device.index))]
    pub async fn spawn(
        cfg: &ServerConfig,
        device: &GpuDevice,
        transport: Arc<RouterTransport>,
        worker_id: String,
    ) -> Result<Self, AnvilError> {
        // Build the subprocess command with the venv interpreter, worker script,
        // injected environment variables, and piped stdio for log capture.
        // The port parameter is derived from the transport's bound port.
        let port = transport.port;
        let mut cmd = build_command(cfg, device, port);

        // Spawn the subprocess. This executes the Python interpreter with
        // the worker script and injected environment variables. On failure,
        // we return an Io error with the underlying OS error message.
        let child = cmd.spawn().map_err(AnvilError::Io)?;

        // Set the initial status to Initializing. The worker will transition
        // to Idle only after receiving a Ready event from the subprocess.
        let status = Arc::new(tokio::sync::RwLock::new(WorkerStatus::Initializing));

        // Create the mpsc channel for sending messages to the worker.
        // The bridge writer task receives from this channel and forwards
        // messages to the transport. Channel capacity of 16 is sufficient
        // for normal operation; backpressure will flow through the mpsc
        // mechanism if the bridge falls behind.
        let (msg_tx, msg_rx) = mpsc::channel(16);

        // Create the broadcast channel for events from the worker.
        // The bridge reader task broadcasts events received from the
        // transport to this channel. Capacity of 16 allows the state
        // machine to lag slightly without dropping events.
        let (event_tx, _event_rx) = broadcast::channel(16);

        // Spawn the IPC bridge: two independent tokio tasks for message routing.
        // The writer forwards messages from mpsc → transport, the reader forwards
        // events from transport → broadcast. We clone event_tx because the bridge
        // task takes ownership of one copy.
        // The IPC routing identity must match what the Python worker sets as its
        // ZMQ identity — ANVILML_WORKER_ID, which build_worker_env() sets to the
        // bare device index (e.g. "0"), not the "worker-N" display label. Using
        // the display label here causes every send() to fail with "Destination
        // client not found by identity", since ZMQ never registered that string.
        let ipc_identity_bytes = device.index.to_string().into_bytes();
        let (writer_handle, reader_handle) = bridge::start(
            transport.clone(),
            ipc_identity_bytes,
            msg_rx,
            event_tx.clone(),
        );
        let bridge_handles = Some((writer_handle, reader_handle));

        // Create the keepalive timeout callback. The callback captures a weak
        // reference to the status Arc and transitions the worker to Dead on
        // timeout. We use a weak reference to avoid a borrow cycle: the status
        // Arc is owned by the ManagedWorker, and the callback is stored inside
        // the keepalive task. If we used a strong reference, dropping the
        // ManagedWorker would never drop the keepalive task (and vice versa).
        //
        // Since the callback is synchronous (Fn()) but the status transition
        // requires async access to the RwLock, we spawn a new tokio task
        // inside the callback to perform the transition on the runtime.
        let status_weak = Arc::downgrade(&status);
        let worker_id_for_callback = worker_id.clone();
        let on_timeout = move || {
            let status_weak = status_weak.clone();
            let worker_id = worker_id_for_callback.clone();
            // Spawn a new task to perform the async status transition.
            // This is necessary because the callback is synchronous (Fn())
            // but we need async access to the RwLock.
            tracing::info!(
                worker_id = %worker_id,
                "keepalive timeout — spawning status transition task"
            );
            tokio::spawn(async move {
                if let Some(s) = status_weak.upgrade() {
                    *s.write().await = WorkerStatus::Dead;
                }
            });
        };

        // Spawn the keepalive heartbeat task. The heartbeat periodically sends
        // Ping messages and waits for matching Pong responses. If a pong is
        // not received within the timeout, the on_timeout callback is invoked.
        // The ping_interval is 30 seconds (how often to send pings) and the
        // pong_timeout is 10 seconds (how long to wait for a response).
        let (keepalive_handle, heartbeat_handle) = keepalive::start(
            worker_id.clone(),
            msg_tx.clone(),
            _event_rx,
            Duration::from_secs(30), // ping_interval
            Duration::from_secs(10), // pong_timeout
            on_timeout,
        );

        // Store the device name from the GpuDevice for logging during run loop.
        // This will be overwritten by the Ready event's device_name if the
        // worker reports a different name (which can happen with auto-detected devices).
        let device_name = device.name.clone();

        // Log at INFO level: mandatory log point per ENVIRONMENT.md §9.
        // The worker_id, device_index, and pid fields are indexed by log aggregators.
        tracing::info!(
            worker_id = %worker_id,
            device_index = %device.index,
            pid = %child.id().unwrap_or(0),
            "worker spawned"
        );

        Ok(Self {
            status,
            msg_tx,
            event_tx,
            child: Some(child),
            bridge_handles,
            keepalive_handle: Some(keepalive_handle),
            heartbeat_handle: Some(heartbeat_handle),
            respawn_policy: RespawnPolicy::default(),
            worker_id,
            device_name,
            device_index: device.index,
        })
    }

    /// Run the managed worker's state machine event loop.
    ///
    /// This is the main loop that drives the worker's state machine. It consumes
    /// `self` (taking ownership of all fields) and enters a select loop that
    /// processes events from the broadcast channel.
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
        // Subscribe to the event broadcast channel for the run loop.
        // The bridge reader task writes events to event_tx, and this
        // receiver delivers them to the state machine.
        let mut event_rx = self.event_tx.subscribe();

        // Drop the sender held by the worker struct. The bridge reader task
        // (spawned in `spawn()`) holds its own clone for writing events.
        // In tests there is no bridge reader, so the test holds the only
        // remaining sender — dropping it closes the channel and lets the
        // loop exit. In production the bridge reader's clone is the only
        // sender; when the bridge exits, the channel closes.
        drop(self.event_tx);

        // Main event loop — processes events for the worker's full lifetime.
        // The loop exits only when the broadcast channel is closed (bridge
        // reader exited). The ready_timeout is scoped to the Initializing
        // state only; once the worker reaches Idle, the timeout is disarmed.
        loop {
            // Mandatory DEBUG log per ENVIRONMENT.md §9.11.5 — IPC log point
            // for "event received from a worker" tracking.
            tracing::debug!(worker_id = %self.worker_id, "run loop iteration");

            // Scope ready_timeout to Initializing only.
            // Once the worker is no longer Initializing, the timeout must not
            // fire — a worker sitting Idle for hours must not be killed by
            // a stale 60-second timer. We read the status under a shared lock
            // before entering the select to decide whether to arm the timeout.
            let mut ready_timeout = if *self.status.read().await == WorkerStatus::Initializing {
                Some(tokio::time::sleep(Duration::from_secs(60)))
            } else {
                None
            };

            tokio::select! {
                // Ready timeout arm. When ready_timeout is Some, this fires
                // after 60 seconds without a Ready event — the worker is
                // considered unresponsive. When ready_timeout is None, this
                // arm uses a Duration::MAX sleep that will never fire, so
                // it can never win the select.
                _ = async {
                    // `tokio::time::Sleep` is !Unpin, so we must take ownership
                    // and pin it before awaiting. `take()` consumes the Option,
                    // replacing it with None. If the event arm wins the select
                    // before the timeout fires, the sleep is dropped harmlessly;
                    // on the next loop iteration a fresh timeout is created.
                    if let Some(sleep) = ready_timeout.take() {
                        // `pin!` creates a Pin<&mut Sleep> from the owned value,
                        // satisfying the !Unpin requirement of tokio::time::Sleep.
                        std::pin::pin!(sleep).as_mut().await;
                    } else {
                        // Never-firing sleep — this branch can never win.
                        // tokio::select! requires all arms to be futures,
                        // so we use a Duration::MAX sleep as a placeholder.
                        // Duration::MAX ≈ 292,471 years — safe no-op branch.
                        tokio::time::sleep(std::time::Duration::MAX).await;
                    }
                } => {
                    tracing::warn!(
                        worker_id = %self.worker_id,
                        "ready timeout, worker dead"
                    );
                    *self.status.write().await = WorkerStatus::Dead;
                }

                // An event was received from the broadcast channel.
                result = event_rx.recv() => {
                    match result {
                        Ok((_id, event)) => {
                            // Process the event based on the current status.
                            // We read the status first, then make the transition
                            // under a write lock. This ensures we don't race with
                            // other status updates (though in practice only the
                            // run loop writes to status).
                            tracing::debug!(
                                worker_id = %self.worker_id,
                                event_type = ?event,
                                "processing event in run loop"
                            );
                            let _current_status = *self.status.read().await;
                            // The device_name match result is unused — we set
                            // self.device_name directly in the Ready arm above.
                            let _device_name = match &event {
                                WorkerEvent::Ready {
                                    device_name,
                                    device_index,
                                    ..
                                } => {
                                    // Update device_name and device_index from the Ready
                                    // event. The worker may report a different name than
                                    // the initial device name (e.g., auto-detected vs.
                                    // configured). We use the worker's actual reported
                                    // values for subsequent logging and status broadcast.
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
                                            // Worker reported Ready — transition to Idle.
                                            // This is the synchronization point between
                                            // Rust and Python. The worker is now ready
                                            // to accept jobs.
                                            *s = WorkerStatus::Idle;
                                            tracing::info!(
                                                worker_id = %self.worker_id,
                                                device = %device_name,
                                                "worker reached Ready"
                                            );
                                            Some(device_name.clone())
                                        }
                                        // Other states don't transition on Ready.
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
                                            // Dying event from Idle — transition to Dead.
                                            // The worker is terminating.
                                            if matches!(&event, WorkerEvent::Dying { .. }) {
                                                *s = WorkerStatus::Dead;
                                            }
                                        }
                                        WorkerStatus::Busy => {
                                            // Job completion events from Busy — transition to Idle.
                                            // The worker is now ready to accept new jobs.
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
                                            // Terminal states — no further processing.
                                            // Events received while in a terminal state
                                            // are logged at DEBUG but do not change state.
                                        }
                                        WorkerStatus::Initializing => {
                                            // Non-Ready events during initialization
                                            // are ignored (state remains Initializing).
                                        }
                                    }
                                    None
                                }
                            };
                        }
                        // Broadcast channel closed — the bridge reader has exited.
                        // This is a terminal condition; no more events will arrive.
                        // Break the loop to exit the run method and trigger cleanup.
                        Err(broadcast::error::RecvError::Closed) => {
                            tracing::info!(
                                worker_id = %self.worker_id,
                                "broadcast channel closed, worker gone"
                            );
                            break;
                        }
                        // Broadcast channel had lagged events — we dropped some.
                        // This can happen when the state machine falls behind on
                        // recv() calls (e.g., during slow status transitions).
                        // We log at DEBUG and continue; the missed events are
                        // not critical for state machine correctness.
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!(
                                worker_id = %self.worker_id,
                                dropped = %n,
                                "managed dropped lagged events"
                            );
                        }
                    }
                }

                // Child process exited unexpectedly.
                // This arm only fires when child is Some (production).
                // In tests (child = None), the arm uses a never-firing
                // placeholder so it can never win the select.
                _ = async {
                    match self.child.as_mut() {
                        Some(child) => {
                            // Wait for the subprocess to exit. This is the crash
                            // detection mechanism — if the child exits without
                            // sending a Dying event, we detect it here.
                            // `child.wait()` returns a `ChildWait` future that
                            // resolves to an `ExitStatus`. We ignore the result
                            // since we only care that the process exited.
                            let _ = child.wait().await;
                        }
                        None => {
                            // No child process (test mode). Use a never-firing
                            // sleep so this arm never wins the select.
                            tokio::time::sleep(std::time::Duration::MAX).await;
                        }
                    }
                } => {
                    // The subprocess has exited. Transition to Dead, broadcast
                    // the status change, and log the exit code.
                    //
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
                // Note: we cannot broadcast a Dying event here because
                // self.event_tx was dropped at the start of run() (the
                // bridge reader holds the only remaining clone). Subscribers
                // will observe the Dead status via the next GET /v1/workers
                // poll anyway.
                break;
                }
            }
        }

        // After the loop, drop all handles for cleanup.
        // The child process will eventually exit on its own (or be killed
        // by the OS if the parent dies). We don't explicitly wait for it
        // here — the child is dropped when self is dropped at the end of
        // this function.
        drop(self.bridge_handles);
        drop(self.keepalive_handle);
        drop(self.heartbeat_handle);

        tracing::info!(
            worker_id = %self.worker_id,
            "worker run loop ended"
        );
    }

    /// Shut down the managed worker cleanly.
    ///
    /// This method performs a graceful shutdown sequence:
    /// 1. Signal the heartbeat to stop via `HeartbeatHandle::shutdown()`
    /// 2. Send a `Shutdown` message to the bridge (best-effort, may fail
    ///    if the channel is already closed)
    /// 3. Drop `msg_tx` to signal the bridge writer to exit
    /// 4. Drop bridge handles to abort the bridge tasks
    /// 5. Drop the keepalive handle to abort the keepalive task
    /// 6. Wait up to 5 seconds for the subprocess to exit on its own, then
    ///    force-kill it if it hasn't — this is required because dropping a
    ///    `tokio::process::Child` does not terminate the OS process, only
    ///    the Rust handle to it.
    ///
    /// This method consumes `self` to take ownership of all fields.
    #[tracing::instrument(skip(self), fields(worker_id = %self.worker_id))]
    pub async fn shutdown(mut self) {
        // Signal the heartbeat to stop after the current ping/pong cycle.
        // This prevents the heartbeat from sending pings to a worker that
        // is already shutting down.
        if let Some(handle) = &self.heartbeat_handle {
            handle.shutdown().await;
        }

        // Send a Shutdown message to the bridge before dropping the sender.
        // This is best-effort — if the channel is already full or closed,
        // the ok() swallows the error. The bridge writer will also exit
        // when msg_tx is dropped below.
        self.msg_tx.send(WorkerMessage::Shutdown).await.ok();

        // Drop the message sender to signal the bridge writer to exit once
        // it has drained the channel. The writer's `while let Some(msg) =
        // msg_rx.recv().await` loop exits when `recv()` returns `None`,
        // which only happens after every sender is dropped AND the buffer
        // is empty — so dropping here does not discard the Shutdown message
        // we just enqueued above.
        drop(self.msg_tx);

        // Await the writer task so the Shutdown message is actually
        // delivered to the transport before we proceed. This is the fix for
        // workers never receiving Shutdown during supervisor exit: a bare
        // `mpsc::Sender::send().await` only guarantees the message reached
        // the channel buffer, not that the writer task has been scheduled
        // to pull it off and call `transport.send()`. Previously this
        // method dropped the `JoinHandle` immediately afterward — dropping
        // a `tokio::JoinHandle` does NOT abort the task, but it also gives
        // no guarantee the task is ever polled again before the runtime
        // tears down at process exit, since tokio's scheduler is
        // cooperative and nothing in this function naturally yielded long
        // enough for the writer to run. Bounded with a short timeout in
        // case the writer task is itself stuck (e.g. transport socket in a
        // bad state) — shutdown must still make progress in that case.
        if let Some((writer_handle, reader_handle)) = self.bridge_handles.take() {
            if let Err(_) = tokio::time::timeout(Duration::from_secs(2), writer_handle).await {
                tracing::warn!(
                    worker_id = %self.worker_id,
                    "bridge writer task did not finish within grace period during shutdown"
                );
            }
            // The reader task terminates on its own once the transport
            // returns an error (e.g. after the worker process exits and the
            // socket closes) or once the worker subprocess is killed below.
            // We don't block shutdown on it — only abort it so it doesn't
            // outlive this function.
            reader_handle.abort();
        }

        // Drop the keepalive handle to abort the keepalive task.
        drop(self.keepalive_handle);

        // Wait for the subprocess to exit on its own (it should react to the
        // Shutdown message above and exit cleanly), bounded by a grace period.
        // Without this step the OS process survives the supervisor exiting —
        // dropping a `tokio::process::Child` does NOT terminate the child,
        // it only drops Rust's handle to it. This is the fix for the defect
        // where workers remained alive after the supervisor was Ctrl+C'd:
        // `with_graceful_shutdown` only drains HTTP connections, it never
        // touched worker subprocesses, so the child outlived the parent.
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
                    // The grace period elapsed without the child exiting.
                    // Force-kill so the OS process does not outlive the
                    // supervisor.
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

        // Simulate the keepalive callback's spawned task mechanism.
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

        // Simulate the main loop (like run()) — just wait.
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
        });

        // Fire the callback.
        on_timeout();

        // Wait for the main loop to finish.
        let _ = handle.await;

        // Verify the callback fired.
        assert!(
            callback_fired_clone.load(std::sync::atomic::Ordering::SeqCst),
            "callback should have fired"
        );

        // Verify the status was updated by the spawned task.
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
            None,
            None,
            None,
            "test-worker".to_string(),
            "test-device".to_string(),
            0, // device_index
        );

        let status = worker.get_status();

        // Spawn run() — it subscribes to the broadcast channel.
        let run_handle = tokio::spawn(worker.run());

        // Give run() time to subscribe and enter the select loop.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Send a Ready event.
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

        // Wait briefly for the event to be processed.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Verify the status transitioned to Idle.
        let final_status = *status.read().await;
        assert_eq!(
            final_status,
            WorkerStatus::Idle,
            "status should be Idle after Ready event, got {:?}",
            final_status
        );

        // Close the channel to let run() exit.
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
            None,
            None,
            None,
            "test-worker".to_string(),
            "test-device".to_string(),
            0, // device_index
        );

        let status = worker.get_status();

        // Spawn run() — it subscribes to the broadcast channel.
        let run_handle = tokio::spawn(worker.run());

        // Give run() time to subscribe.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Manually set status to Busy (simulating job dispatch).
        {
            let mut s = status.write().await;
            *s = WorkerStatus::Busy;
        }

        // Wait for the write to complete.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Send a Completed event.
        let completed_event = WorkerEvent::Completed {
            job_id: uuid::Uuid::new_v4(),
            elapsed_ms: 5000,
        };
        let _ = event_tx.send(("test-worker".to_string(), completed_event));

        // Wait for the event to be processed.
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify the status transitioned to Idle.
        let final_status = *status.read().await;
        assert_eq!(
            final_status,
            WorkerStatus::Idle,
            "status should be Idle after Completed event, got {:?}",
            final_status
        );

        // Close the channel to let run() exit.
        drop(event_tx);
        let _ = run_handle.await;
    }
}
