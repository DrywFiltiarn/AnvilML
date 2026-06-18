//! Managed worker — a state machine that supervises a Python worker subprocess.
//!
//! `ManagedWorker` owns the lifecycle of a Python worker subprocess, including:
//! - Spawning the subprocess with proper environment variables and stdio piping
//! - Sending IPC messages via the bridge writer task (see `crate::bridge`)
//! - Heartbeat keepalive with timeout watchdog
//! - State machine transitions driven by events from the worker
//! - Clean shutdown of all spawned tasks, triggered either by the worker
//!   dying on its own or by an external shutdown request
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
//! Join handles are `Option<JoinHandle>` so `run()`'s shutdown arm can take
//! ownership and drop them to abort the task, since `tokio::JoinHandle` has
//! no `Clone`.
//!
//! **There is no standalone `shutdown()` method.** `run(self)` consumes the
//! worker for its entire lifetime, so a separate method requiring an owned
//! `ManagedWorker` later (after `run()` has already taken it) cannot exist —
//! `WorkerPool` only ever holds `run()`'s `JoinHandle` plus a
//! `oneshot::Sender<()>` once a worker is spawned. Requesting shutdown means
//! firing that sender; `run()`'s `select!` loop has a dedicated arm that
//! performs the full teardown sequence and then exits the loop. See `run()`'s
//! doc comment for the exact sequence.

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

/// A managed worker subprocess with state machine supervision.
///
/// Created via `spawn()` (launches the subprocess) or `new()` (pre-built
/// channels, for testing). Consumed entirely by `run()` (the event loop),
/// which also owns shutdown — see `run()`'s doc comment for the shutdown
/// sequence and why there is no separate `shutdown()` method.
#[allow(dead_code)] // child, device_index, and respawn_policy are for future respawn logic (P10-A1)
#[derive(Debug)]
pub struct ManagedWorker {
    /// Read-write locked so the run loop can write transitions while other
    /// tasks (e.g. the pool's status-change monitor) read concurrently.
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

    /// `Option`-wrapped so `run()`'s shutdown arm can take ownership and
    /// await it. There is no reader handle here — see the module docs for
    /// why a single pool-wide demux task replaced the per-worker reader.
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

    /// The pool-wide demux routing table, so `run()`'s shutdown arm can
    /// remove this worker's entry on the way out. `None` for workers built
    /// via `new()` without a real demux task (most unit tests) — in that
    /// case deregistration is a no-op, not an error, since there is no
    /// table to leak an entry in.
    routes: Option<crate::demux::RouteTable>,

    /// This worker's key in `routes`, rendered by `anvilml_ipc::render_identity`
    /// from the same `ipc_identity` bytes used to address `send()` calls.
    /// Stored alongside `routes` (both present or both absent) since neither
    /// is useful without the other.
    route_key: Option<String>,

    /// Fires once, in `run()`'s `Ready` transition arm, to release the
    /// keepalive task's gate so it can start pinging — see `crate::keepalive`'s
    /// `ready_rx` parameter. Unlike `routes`/`route_key`, `None` here is not
    /// solely a test-only state: in production this becomes `None` the
    /// instant `Ready` is processed (via `.take()`), so by the time a
    /// worker reaches `Idle` this field has already legitimately emptied
    /// itself. `None` from construction (as opposed to from firing) only
    /// occurs for test workers built via `new()` that don't exercise a
    /// real keepalive task at all.
    ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
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
    /// * `routes` — The pool-wide demux routing table, for deregistration on
    ///   shutdown. `None` when the test has no demux task running.
    /// * `route_key` — This worker's key in `routes`. `None` iff `routes` is
    ///   `None` — the two are only ever meaningful together.
    /// * `ready_tx` — Fired on the `Initializing → Idle` transition to
    ///   release the keepalive task's start gate. `None` for tests that
    ///   don't construct a real keepalive task (the common case) or that
    ///   fire their own `ready_tx` directly before calling `new()`.
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
        routes: Option<crate::demux::RouteTable>,
        route_key: Option<String>,
        ready_tx: Option<tokio::sync::oneshot::Sender<()>>,
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
            routes,
            route_key,
            ready_tx,
        }
    }
    /// Spawn a Python worker subprocess, register its route with the
    /// pool-wide demux task, and return the resulting `ManagedWorker`.
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
    /// * `routes` — The pool-wide demux routing table. `spawn()` registers
    ///   this worker's route before returning (rather than leaving
    ///   registration to the caller) so the same code path that knows the
    ///   IPC identity bytes also owns writing — and later, on shutdown,
    ///   erasing — the table entry derived from them. The worker retains
    ///   the table and its own key so `run()`'s shutdown arm can call
    ///   `demux::deregister` without the pool needing to track the key
    ///   separately.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Io` if the subprocess fails to spawn.
    ///
    /// # Subprocess lifecycle
    ///
    /// On Linux, `PR_SET_PDEATHSIG` is set so the worker is killed if the
    /// parent supervisor dies.
    ///
    /// # Registration ordering
    ///
    /// Registration happens here, before `spawn()` returns — not afterward
    /// by the caller — so there is no window between subprocess spawn and
    /// route registration during which the worker's first ping could be
    /// delivered to a table with no entry for it yet. See `crate::demux`'s
    /// module docs for why that race matters.
    #[tracing::instrument(skip(cfg, device, transport, routes), fields(worker_id, device_index = %device.index))]
    pub async fn spawn(
        cfg: &ServerConfig,
        device: &GpuDevice,
        transport: Arc<RouterTransport>,
        worker_id: String,
        routes: crate::demux::RouteTable,
    ) -> Result<Self, AnvilError> {
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
        // task (already running by the time spawn() is called — see
        // "Registration ordering" above) is the only reader of this
        // worker's events — see crate::demux.
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

        // Fires once, in run()'s Ready transition arm, to release the
        // keepalive task's start gate — see crate::keepalive's ready_rx
        // parameter for why pinging must not begin before Ready.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

        let (keepalive_handle, heartbeat_handle) = keepalive::start(
            worker_id.clone(),
            msg_tx.clone(),
            _event_rx,
            ready_rx,
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

        // Registered here, immediately, rather than handed back to the
        // caller as a RouteInfo to register later — see "Registration
        // ordering" above. render_identity is the single source of truth
        // for key rendering; using anything else here risks a mismatch
        // against what RouterTransport::recv() renders for the same bytes.
        let route_key = anvilml_ipc::render_identity(&ipc_identity_bytes);
        crate::demux::register(
            &routes,
            route_key.clone(),
            (worker_id.clone(), event_tx.clone()),
        )
        .await;

        Ok(Self {
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
            routes: Some(routes),
            route_key: Some(route_key),
            ready_tx: Some(ready_tx),
        })
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
    ///
    /// # Keepalive gating and abort
    ///
    /// The keepalive task spawned in `spawn()` does not send its first ping
    /// until this method fires `ready_tx` in the `Ready` transition arm —
    /// see `crate::keepalive`'s `ready_rx` parameter. Symmetrically, every
    /// path below that puts the worker into a confirmed-`Dead` state
    /// (ready timeout, unexpected child exit, and the graceful shutdown
    /// arm) calls `keepalive_handle.take().abort()` rather than merely
    /// dropping the handle: dropping a `JoinHandle` detaches Rust's
    /// interest in the task's result but does not stop the task itself,
    /// which would otherwise keep running — and, if it had already passed
    /// the `ready_rx` gate, keep pinging — a worker that no longer exists.
    ///
    /// # Shutdown
    ///
    /// `shutdown_rx` is the caller's means of requesting a graceful stop —
    /// firing the matching `oneshot::Sender` (held by `WorkerPool`) wakes
    /// the loop's shutdown arm, which runs the following sequence in order
    /// and then returns:
    /// 1. Signal the heartbeat to stop via `HeartbeatHandle::shutdown()`
    /// 2. Send a `Shutdown` message to the bridge (best-effort, may fail
    ///    if the channel is already closed)
    /// 3. Drop `msg_tx` to signal the bridge writer to exit
    /// 4. Await the bridge writer (bounded to 2s), so `Shutdown` is
    ///    actually transmitted before proceeding
    /// 5. Abort the keepalive task via its `JoinHandle` (see "Keepalive
    ///    gating and abort" above for why this is `.abort()` and not a
    ///    bare drop)
    /// 6. Deregister this worker's route from the demux table (no-op if
    ///    `routes`/`route_key` are `None`)
    /// 7. Wait up to 5 seconds for the subprocess to exit on its own, then
    ///    force-kill it — dropping a `tokio::process::Child` does not
    ///    terminate the OS process, only the Rust handle to it
    ///
    /// This is the only shutdown path: there is no separate method, since
    /// `run(self)` already holds the only owned `ManagedWorker` there will
    /// ever be once spawned. The loop's two other exit paths (ready timeout
    /// and unexpected child exit) skip this sequence — each aborts the
    /// keepalive task directly (see "Keepalive gating and abort" above)
    /// and otherwise only drops the remaining task handles — because in
    /// both cases the worker is already gone or unresponsive, so there is
    /// nothing live to signal a graceful `Shutdown` message to.
    #[tracing::instrument(skip(self, shutdown_rx), fields(worker_id = %self.worker_id))]
    pub async fn run(mut self, shutdown_rx: tokio::sync::oneshot::Receiver<()>) {
        let mut event_rx = self.event_tx.subscribe();
        let mut shutdown_rx = shutdown_rx;

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

                    // The worker never reported Ready, so the keepalive
                    // task is still parked on its ready_rx gate (or, if it
                    // somehow already passed it, is mid-ping-cycle against
                    // a worker that's now confirmed dead) — abort it
                    // outright rather than dropping the handle, which
                    // would only detach interest in the task without
                    // stopping it.
                    if let Some(handle) = self.keepalive_handle.take() {
                        handle.abort();
                    }
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
                                            // Release the keepalive task's
                                            // start gate now, not before —
                                            // see crate::keepalive's
                                            // ready_rx parameter. A send
                                            // failure here only means the
                                            // keepalive task already exited
                                            // on its own (e.g. the bridge
                                            // writer died first); nothing
                                            // further to do about that here.
                                            if let Some(tx) = self.ready_tx.take() {
                                                let _ = tx.send(());
                                            }
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

                    // The worker is confirmed dead here too — same
                    // reasoning as the ready-timeout arm above: abort
                    // rather than drop, since dropping the handle alone
                    // would leave the task running (and possibly still
                    // pinging) against a worker that no longer exists.
                    if let Some(handle) = self.keepalive_handle.take() {
                        handle.abort();
                    }
                    break;
                }

                // Fires once when the pool (or whatever holds the matching
                // oneshot::Sender) requests a graceful shutdown. This is the
                // only path that sends a Shutdown message to the Python
                // worker and waits for/kills its subprocess — the other two
                // break arms above assume the worker is already gone or
                // unresponsive, so there is no live peer left to notify.
                _ = &mut shutdown_rx => {
                    tracing::info!(
                        worker_id = %self.worker_id,
                        "shutdown requested, beginning teardown"
                    );

                    // Stop pinging before the worker is told to shut down,
                    // so no ping races the Shutdown message sent below.
                    if let Some(handle) = &self.heartbeat_handle {
                        handle.shutdown().await;
                    }

                    // Best-effort: the writer drains this from the channel
                    // below regardless of whether this particular send()
                    // succeeds.
                    self.msg_tx.send(WorkerMessage::Shutdown).await.ok();

                    // The writer's `while let Some(msg) = msg_rx.recv().await`
                    // only exits once every sender is dropped AND the buffer
                    // is drained — so dropping here cannot discard the
                    // Shutdown message just sent. This is a partial move out
                    // of self.msg_tx, legal here because run() owns self by
                    // value for its whole body and nothing below uses self
                    // as a complete struct again — only its remaining
                    // individual fields.
                    drop(self.msg_tx);

                    // A bare send().await only guarantees the message
                    // reached the channel buffer, not that the writer task
                    // has since been scheduled to pull it off and call
                    // transport.send(). Awaiting the handle (bounded, in
                    // case the writer is itself stuck on a bad transport)
                    // is what actually guarantees delivery before shutdown
                    // proceeds.
                    if let Some(writer_handle) = self.bridge_handle.take() {
                        if tokio::time::timeout(Duration::from_secs(2), writer_handle)
                            .await
                            .is_err()
                        {
                            tracing::warn!(
                                worker_id = %self.worker_id,
                                "bridge writer task did not finish within grace \
                                 period during shutdown"
                            );
                        }
                        // No reader handle to abort here — the demux task is
                        // pool-wide and outlives any single worker's
                        // shutdown. WorkerPool aborts it once, after every
                        // worker has stopped.
                    }

                    // Aborted, not merely dropped, for the same reason as
                    // the ready-timeout and child-exit arms: if shutdown
                    // was requested while still Initializing, ready_tx was
                    // never fired, so the keepalive task is parked on its
                    // ready_rx gate — that resolves to Err once ready_tx
                    // drops with self at the end of this function and the
                    // task would exit on its own either way, but aborting
                    // here keeps all three Dead-bound exit paths uniform
                    // rather than this one being the sole exception.
                    if let Some(handle) = self.keepalive_handle.take() {
                        handle.abort();
                    }

                    // Remove this worker's entry from the demux table so it
                    // doesn't accumulate stale routes across crashes and
                    // respawns — see crate::demux::deregister's doc comment
                    // for why an ever-growing table is the failure mode this
                    // guards against. No-op when routes/route_key are None
                    // (workers built via new() without a real demux task).
                    if let (Some(routes), Some(key)) = (&self.routes, &self.route_key) {
                        crate::demux::deregister(routes, key).await;
                    }

                    // Without this wait, the OS process survives the
                    // supervisor exiting: dropping tokio::process::Child
                    // only drops Rust's handle, never the underlying
                    // process.
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
                                // Grace period elapsed: force-kill so the
                                // process can't outlive the supervisor.
                                tracing::warn!(
                                    worker_id = %self.worker_id,
                                    "worker subprocess did not exit within grace \
                                     period, killing"
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

                    tracing::debug!(worker_id = %self.worker_id, "worker shutdown complete");
                    break;
                }
            }
        }

        // Reached by all four break paths (ready timeout, channel closed,
        // unexpected child exit, and the shutdown arm above). Three of
        // those four — every path except "channel closed" — already
        // `.take()` and abort `keepalive_handle` as part of their own
        // arm (see "Keepalive gating and abort" on this method's doc
        // comment), so `drop(self.keepalive_handle)` below is a no-op for
        // them (the Option is already None) and is only real cleanup for
        // the "channel closed" path, which never touches keepalive_handle
        // itself. `bridge_handle` is dropped here unconditionally, though
        // it's a no-op for the shutdown arm, which already consumed it via
        // `.take()` above. `heartbeat_handle` is only ever borrowed
        // (`&self.heartbeat_handle`, in the shutdown arm's
        // `HeartbeatHandle::shutdown()` call), never taken, so it reaches
        // this drop as `Some` on every path. The child subprocess isn't
        // waited on here for the three non-shutdown paths — if it's still
        // alive, it's either exiting on its own or will be reaped when
        // self drops.
        drop(self.bridge_handle);
        drop(self.keepalive_handle);
        drop(self.heartbeat_handle);

        tracing::info!(
            worker_id = %self.worker_id,
            "worker run loop ended"
        );
    }
}
