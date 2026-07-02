//! A cheap, `Clone`-able handle for interacting with a worker's lifecycle,
//! and the full lifecycle manager for a single Python worker subprocess.
//!
//! Two types are defined:
//!
//! - `WorkerHandle` — a cheap, `Clone`-able handle for interacting with a worker's
//!   lifecycle. Each handle owns an `Arc`-reference to the worker's status lock,
//!   a `oneshot::Sender` for requesting shutdown, and an `Arc<Mutex<Option<JoinHandle>>>`
//!   for tracking the worker task.
//! - `ManagedWorker` — the full lifecycle task that owns a worker's lifetime.
//!   It calls `demux.register()` on entry and `demux.deregister()` on every exit
//!   path (graceful shutdown, crash, timeout).
//!
//! The `WorkerHandle` is a lightweight view into shared state; `ManagedWorker`
//! is the consuming task that runs the lifecycle loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tokio::sync::oneshot;

use anvilml_core::types::worker::WorkerStatus;
use anvilml_ipc::WorkerEvent;

use crate::demux::Demux;
use crate::keepalive::{KeepaliveWatchdog, RouterTransportAdapter};
use crate::respawn::RespawnPolicy;
use anvilml_ipc::RouterTransport;

/// Production default for the Initializing→Idle grace period.
///
/// Per `ANVILML_DESIGN.md §9.2`: "A worker that fails to reach `Idle` within
/// 60 seconds is killed and respawned." This is real grace time for the
/// Python subprocess to load a diffusion model onto the GPU (CUDA init +
/// weight loading) — not an arbitrary value, and not one production callers
/// should shorten. It exists as an explicit constant (rather than being
/// buried inline in `run()`) so every caller of `ManagedWorker::new()` states
/// its intent plainly: `ManagedWorker::new(..., DEFAULT_INIT_TIMEOUT)` for
/// production, or a short `Duration` for tests exercising the timeout path
/// itself.
pub const DEFAULT_INIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Production default for the keepalive watchdog ping interval.
///
/// Per `ANVILML_DESIGN.md §9.2`: "Keepalive pings every 30 seconds."
/// Tests override with a short duration so they don't block on real seconds.
pub const DEFAULT_WATCHDOG_PING_INTERVAL: Duration = Duration::from_secs(30);

/// Production default for the keepalive watchdog pong timeout.
///
/// Per `ANVILML_DESIGN.md §9.2`: "no pong within 10 seconds → dead."
/// Tests override with a short duration so they don't block on real seconds.
pub const DEFAULT_WATCHDOG_PONG_TIMEOUT: Duration = Duration::from_secs(10);

/// A cheap, `Clone`-able handle for interacting with a worker's lifecycle.
///
/// Each `WorkerHandle` owns an `Arc`-reference to the worker's status lock,
/// a `oneshot::Sender` for requesting shutdown, and an `Arc<Mutex<Option<JoinHandle>>>`
/// for tracking the worker task. Cloning a handle produces a new handle that shares
/// the same status lock and join handle — both clones observe the same status and can
/// request the same shutdown. The `worker_id` field is copied (not shared) across clones.
///
/// This handle does **not** own the `ManagedWorker` struct itself — it is a lightweight
/// view into the worker's shared state, designed to be freely shared across tasks and
/// API handlers without `Arc`-wrapping the full worker.
pub struct WorkerHandle {
    /// Stable worker identity — the bare device index as a string (e.g. `"0"`).
    /// Copied, not shared, across clones.
    pub worker_id: String,

    /// Shared status lock — all clones read from the same lock.
    /// Private: consumers must use `status()` to read the current state.
    status: Arc<RwLock<WorkerStatus>>,

    /// Optional shutdown trigger — `take()`n on the first call to `request_shutdown()`,
    /// making the operation idempotent (second call is a no-op).
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// Shared join handle wrapper — allows the pool to extract and await the handle
    /// during shutdown with a bounded timeout.
    join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

// `shutdown_tx` (oneshot::Sender) is not Clone — clones cannot request shutdown.
// Only the original handle retains the ability to trigger shutdown.
impl Clone for WorkerHandle {
    fn clone(&self) -> Self {
        Self {
            worker_id: self.worker_id.clone(),
            status: Arc::clone(&self.status),
            // Clone cannot take the sender — the clone loses the ability to
            // request shutdown, preserving the invariant that only the original
            // handle can trigger it.
            shutdown_tx: None,
            join_handle: Arc::clone(&self.join_handle),
        }
    }
}

impl WorkerHandle {
    /// Construct a new `WorkerHandle` from its component parts.
    ///
    /// # Arguments
    ///
    /// * `worker_id` — Stable worker identity string (e.g. `"0"`). Copied into the handle.
    /// * `status` — Shared `Arc<RwLock<WorkerStatus>>` for the worker's lifecycle state.
    ///   All clones of this handle share this same lock.
    /// * `shutdown_tx` — Optional `oneshot::Sender` used to signal the worker to shut down.
    ///   If `None`, `request_shutdown()` becomes a no-op.
    /// * `join_handle` — Shared `Arc<Mutex<Option<JoinHandle>>>` for tracking the worker task.
    pub fn new(
        worker_id: String,
        status: Arc<RwLock<WorkerStatus>>,
        shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
        join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    ) -> Self {
        Self {
            worker_id,
            status,
            shutdown_tx,
            join_handle,
        }
    }

    /// Read the worker's current lifecycle state.
    ///
    /// Acquires a read lock on the shared status, copies the `Copy` value,
    /// and releases the lock. The caller never holds the lock.
    pub async fn status(&self) -> WorkerStatus {
        *self.status.read().await
    }

    /// Set the worker's lifecycle state.
    ///
    /// Acquires a write lock on the shared status, overwrites the stored value,
    /// and releases the lock. This is the only public mutator on `WorkerHandle`.
    ///
    /// # Arguments
    ///
    /// * `new` — The new `WorkerStatus` value to set.
    pub async fn set_status(&self, new: WorkerStatus) {
        *self.status.write().await = new;
    }

    /// Request the worker to shut down gracefully.
    ///
    /// Takes the `oneshot::Sender` from `shutdown_tx` and sends `()` to the receiver.
    /// If `shutdown_tx` is already `None` (already called), this is a no-op — making
    /// the method idempotent. The result of `tx.send(())` is ignored because the
    /// receiver may already have been dropped (worker already exited).
    pub fn request_shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            // Ignore the result — the receiver may already be dropped if the worker
            // has already exited. This makes request_shutdown idempotent.
            let _ = tx.send(());
        }
    }
}

/// Full lifecycle manager for a single Python worker subprocess.
///
/// `ManagedWorker` owns the worker's stable identity, a shared `RouterTransport` for
/// receiving events from the worker, a shared `Demux` for the routing table, and a
/// `RespawnPolicy` for crash-recovery decisions. It consumes `self` in `run()`, which
/// owns the worker's entire lifetime.
///
/// The lifecycle is:
/// 1. **Initializing** — the worker is spawned; `init_timeout` (60s in production,
///    via `DEFAULT_INIT_TIMEOUT`; configurable per-instance, see `new()`) guards
///    this state.
/// 2. **Idle** — the worker sent a `Ready` event; waiting for job assignment.
/// 3. **Busy** — the worker is executing a job.
/// 4. **Dying** — the worker received a shutdown signal or sent a `Dying` event.
/// 5. **Dead** — the worker has terminated.
/// 6. On exit, `deregister()` is called to remove the worker from the routing table.
///
/// `run()` is the consuming method — it takes `self` by value and returns only after
/// the worker has fully exited and been deregistered.
///
/// Registration with the demux is performed by the pool **before** spawning the
/// `ManagedWorker` task. `run()` only performs `deregister()` on exit.
pub struct ManagedWorker {
    /// Stable worker identity — the bare device index as a string (e.g. `"0"`).
    /// Used for deregister on exit.
    worker_id: String,

    /// Shared ROUTER socket transport for receiving events from the worker.
    ///
    /// This is an `Arc` so the test can share the same transport with the worker's
    /// `ManagedWorker` and the event-sending test code.
    transport: Arc<RouterTransport>,

    /// Shared routing table — used to deregister on exit.
    ///
    /// Wrapped in `Arc` so the test can inspect it after `run()` consumes the
    /// `ManagedWorker` and returns. Registration is performed by the pool
    /// before `run()` is spawned.
    demux: Arc<Demux>,

    /// Shared status lock — used by `run()` to track the worker's lifecycle state.
    ///
    /// The pool creates this and passes it into `ManagedWorker`. The `WorkerHandle`
    /// that the pool returns to callers shares this same lock.
    status: Arc<RwLock<WorkerStatus>>,

    /// Crash-recovery backoff policy.
    ///
    /// Decides whether a crashed worker may be respawned based on the count of
    /// recent crash attempts within a sliding window. Consulted on every
    /// crash (transport recv error) to determine if respawn is permissible.
    respawn_policy: RespawnPolicy,

    /// Timestamps of crash transitions.
    ///
    /// Each time the worker crashes (transport recv error), `Instant::now()`
    /// is appended to this vector. Consulted by `RespawnPolicy::should_respawn()`
    /// to decide whether a respawn is permissible.
    attempt_history: Vec<Instant>,

    /// Grace period for the Initializing→Idle transition.
    ///
    /// Production callers should pass `DEFAULT_INIT_TIMEOUT` (60s per
    /// `ANVILML_DESIGN.md §9.2`). Tests exercising the timeout path itself
    /// pass a short duration so the test doesn't block on a real 60s wait —
    /// see `KeepaliveWatchdog::new()`'s `ping_interval`/`pong_timeout` params
    /// for the same pattern elsewhere in this crate.
    init_timeout: Duration,

    /// Oneshot receiver for the watchdog's death signal.
    ///
    /// When the watchdog detects a missing Pong (pong_timeout elapsed without a
    /// matching response), it sends on `dead_tx` and this receiver becomes ready.
    /// The `run()` loop polls this in a `select!` branch alongside shutdown and
    /// transport recv — when ready, the worker is declared Dead.
    watchdog_dead_rx: tokio::sync::oneshot::Receiver<()>,

    /// Optional oneshot sender for the watchdog's death signal.
    ///
    /// Created in `new()`, stored here, then `take()`n in `run()` when spawning
    /// the watchdog task. This ensures the sender is owned by the watchdog task
    /// and dropped when the task exits.
    watchdog_dead_tx: Option<tokio::sync::oneshot::Sender<()>>,

    /// Channel sender for forwarding Pong events to the watchdog.
    ///
    /// Each `WorkerEvent::Pong` from `handle_event()` is sent here. The watchdog
    /// filters for matching sequence numbers internally. A closed or full channel
    /// is not an error — the watchdog will eventually timeout and declare the
    /// worker dead, which is the correct failure mode.
    pong_tx: tokio::sync::mpsc::Sender<anvilml_ipc::WorkerEvent>,

    /// Time between consecutive Ping messages sent by the watchdog.
    ///
    /// Production default: 30 seconds per `ANVILML_DESIGN.md §9.2`.
    /// Tests override with a short duration.
    watchdog_ping_interval: Duration,

    /// Maximum time to wait for a matching Pong after sending a Ping.
    ///
    /// Production default: 10 seconds per `ANVILML_DESIGN.md §9.2`.
    /// Tests override with a short duration.
    watchdog_pong_timeout: Duration,
}

impl ManagedWorker {
    /// Construct a new `ManagedWorker` from its component parts.
    ///
    /// The worker is **not** registered in the demux by this constructor — the
    /// pool must call `demux.register(worker_id, tx)` before spawning `run()`.
    /// `run()` will call `demux.deregister()` on every exit path.
    ///
    /// # Arguments
    ///
    /// * `worker_id` — Stable worker identity string (e.g. `"0"`).
    /// * `transport` — Shared `RouterTransport` for receiving events from the worker.
    /// * `demux` — Shared routing table for deregister on exit.
    /// * `status` — Shared status lock for tracking lifecycle state.
    /// * `respawn_policy` — Crash-recovery decision logic.
    /// * `init_timeout` — Grace period for the Initializing→Idle transition.
    ///   Production callers pass `DEFAULT_INIT_TIMEOUT`; tests exercising the
    ///   timeout path itself pass a short duration.
    /// * `pong_tx` — Channel sender for forwarding Pong events to the keepalive
    ///   watchdog. The caller creates the channel pair and passes the sender here;
    ///   the receiver is used internally by the watchdog spawned in `run()`.
    /// * `watchdog_ping_interval` — Time between consecutive Ping messages.
    ///   Production callers should pass `DEFAULT_WATCHDOG_PING_INTERVAL`.
    ///   Tests use a short duration so the test doesn't block on a real 30s wait.
    /// * `watchdog_pong_timeout` — Maximum wait for a matching Pong after a Ping.
    ///   Production callers should pass `DEFAULT_WATCHDOG_PONG_TIMEOUT`.
    ///   Tests use a short duration so the test doesn't block on a real 10s wait.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        worker_id: String,
        transport: Arc<RouterTransport>,
        demux: Arc<Demux>,
        status: Arc<RwLock<WorkerStatus>>,
        respawn_policy: RespawnPolicy,
        init_timeout: Duration,
        pong_tx: tokio::sync::mpsc::Sender<anvilml_ipc::WorkerEvent>,
        watchdog_ping_interval: Duration,
        watchdog_pong_timeout: Duration,
    ) -> Self {
        let (watchdog_dead_tx, watchdog_dead_rx) = tokio::sync::oneshot::channel();
        Self {
            worker_id,
            transport,
            demux,
            status,
            respawn_policy,
            attempt_history: Vec::new(),
            init_timeout,
            watchdog_dead_rx,
            watchdog_dead_tx: Some(watchdog_dead_tx),
            pong_tx,
            watchdog_ping_interval,
            watchdog_pong_timeout,
        }
    }

    /// Returns the number of crash attempts tracked in `attempt_history`.
    ///
    /// Each crash (transport recv error) appends an `Instant` to the history.
    /// This accessor is primarily for testing — it lets callers verify that
    /// crash-attempt tracking is working correctly without exposing the
    /// internal `Vec<Instant>` directly.
    pub fn attempt_count(&self) -> usize {
        self.attempt_history.len()
    }

    /// Run the worker's full lifecycle.
    ///
    /// This method consumes `self` and owns the worker's entire lifetime:
    /// - Sets status to `Initializing` on entry.
    /// - Spawns the `init_timeout` guard (60s in production; configurable, see `new()`):
    ///   if no `Ready` event arrives before it elapses, sets status to `Dead`.
    /// - Spawns a `KeepaliveWatchdog` task that sends Pings and monitors Pongs;
    ///   if no Pong arrives within the timeout (pong_timeout), the watchdog signals
    ///   death via `watchdog_dead_rx`, which triggers the same crash path as a
    ///   transport error.
    /// - Enters a `tokio::select!` loop between `shutdown_rx`, `transport.recv()`,
    ///   `init_timeout`, and `watchdog_dead_rx`.
    /// - Transitions status based on events: `Ready → Idle`, `Dying → Dead`,
    ///   `Completed/Failed/Cancelled → Idle`.
    /// - Calls `demux.deregister()` on every exit path.
    ///
    /// # Arguments
    ///
    /// * `shutdown_rx` — A oneshot receiver; when `()` is sent, the worker transitions
    ///   to `Dying` and exits cleanly.
    ///
    /// # Exit paths
    ///
    /// 1. **Graceful shutdown** — `shutdown_rx` receives `()`: status → `Dying`, deregister.
    /// 2. **Initializing timeout** — `init_timeout` elapses without `Ready` (60s in
    ///    production): status → `Dead`, deregister.
    /// 3. **Worker crash (explicit)** — `Dying` event received: status → `Dead`, deregister.
    /// 4. **Worker crash (transport)** — `transport.recv()` returns `Err`: status → `Dead`,
    ///    `attempt_history` is appended and `RespawnPolicy::should_respawn()` is consulted
    ///    (the decision itself is acted on by `P8-E6`), deregister.
    /// 5. **Keepalive watchdog timeout** — `watchdog_dead_rx` becomes ready (no Pong
    ///    within `pong_timeout`): status → `Dead`, `attempt_history` appended and
    ///    `should_respawn()` consulted (same crash path as transport error), deregister.
    ///
    /// On all exit paths, `demux.deregister(&self.worker_id)` is the final action.
    #[tracing::instrument(skip(self, shutdown_rx), fields(worker_id = %self.worker_id))]
    pub async fn run(mut self, mut shutdown_rx: oneshot::Receiver<()>) {
        // Step 1: Set status to Initializing.
        // The worker must transition through Initializing before it can reach Idle.
        // Registration with the demux was performed by the pool before spawning.
        *self.status.write().await = WorkerStatus::Initializing;

        // Step 2: Spawn the Initializing timeout guard.
        // If this sleep completes before a Ready event, the worker is declared Dead.
        // `initialized` disarms this branch once Ready arrives — see the `if
        // !initialized` precondition on the select branch below. Without this,
        // the timeout keeps ticking for the worker's entire lifetime and fires
        // at `self.init_timeout` regardless of how long ago Ready was actually
        // received.
        let init_timeout = tokio::time::sleep(self.init_timeout);
        tokio::pin!(init_timeout);
        let mut initialized = false;

        // Step 3: Spawn the keepalive watchdog.
        // The watchdog sends Pings at a configurable interval and waits for
        // matching Pongs through its pong_rx channel. If no Pong arrives within
        // pong_timeout after a Ping, it sends on watchdog_dead_tx, making
        // watchdog_dead_rx ready in the select! below.
        //
        // Production defaults: 30s ping interval, 10s pong timeout
        // (ANVILML_DESIGN.md §9.2).
        let watchdog_dead_tx = self
            .watchdog_dead_tx
            .take()
            .expect("watchdog_dead_tx should be Some before run() starts");
        let (watchdog_pong_tx, watchdog_pong_rx) = tokio::sync::mpsc::channel(16);
        // Replace self.pong_tx with the sender for the watchdog's channel.
        // This ensures handle_event() forwards Pongs into the same channel
        // the watchdog is reading from.
        self.pong_tx = watchdog_pong_tx;
        let watchdog = KeepaliveWatchdog::new(
            self.worker_id.clone(),
            RouterTransportAdapter(Arc::clone(&self.transport)),
            watchdog_pong_rx,
            watchdog_dead_tx,
            self.watchdog_ping_interval,
            self.watchdog_pong_timeout,
        );
        tokio::spawn(watchdog.run());

        // Step 4: Main event loop.
        loop {
            tokio::select! {
                // Graceful shutdown path.
                _ = &mut shutdown_rx => {
                    *self.status.write().await = WorkerStatus::Dying;
                    tracing::info!(worker_id = %self.worker_id, "shutdown_requested");
                    break;
                }

                // Worker event path.
                result = self.transport.recv() => {
                    match result {
                        Ok((id, event)) => {
                            if matches!(event, WorkerEvent::Ready { .. }) {
                                initialized = true;
                            }
                            // handle_event returns true when the Dying event is
                            // received, signaling the main loop to break and exit.
                            if self.handle_event(&id, event).await {
                                break;
                            }
                        }
                        Err(e) => {
                            // Transport recv failed — this is a fatal error for the
                            // managed worker. Track the crash attempt and decide
                            // whether a respawn is permissible.
                            // P8-E6 will act on the decision by sleeping, re-spawning,
                            // and continuing the loop instead of breaking.
                            tracing::error!(worker_id = %self.worker_id, error = %e, "transport recv failed");
                            *self.status.write().await = WorkerStatus::Dead;
                            // Record this crash attempt.
                            self.attempt_history.push(Instant::now());
                            // Consult the respawn policy — this is the decision point
                            // that P8-D1 was built for but nothing had wired up yet.
                            let should = self.respawn_policy.should_respawn(&self.attempt_history);
                            tracing::info!(worker_id = %self.worker_id, should_respawn = should, "crash_respawn_decision");
                            break;
                        }
                    }
                }

                // Initializing timeout — init_timeout elapsed without Ready.
                // This means the worker process started but never reported readiness.
                // Disarmed once `initialized` is true, per the precondition below —
                // this branch is not even polled once Ready has been received.
                _ = &mut init_timeout, if !initialized => {
                    *self.status.write().await = WorkerStatus::Dead;
                    tracing::info!(worker_id = %self.worker_id, "worker_declared_dead");
                    break;
                }

                // Watchdog dead path — the keepalive watchdog detected a missing Pong.
                // This is a second crash source, independent of transport.recv().
                // Handled identically to the transport-error branch.
                _ = &mut self.watchdog_dead_rx => {
                    tracing::error!(worker_id = %self.worker_id, "watchdog timeout — worker declared dead");
                    *self.status.write().await = WorkerStatus::Dead;
                    self.attempt_history.push(Instant::now());
                    let should = self.respawn_policy.should_respawn(&self.attempt_history);
                    tracing::info!(worker_id = %self.worker_id, should_respawn = should, "crash_respawn_decision");
                    break;
                }
            }
        }

        // Final action on every exit path: deregister the worker.
        self.demux.deregister(&self.worker_id);
        tracing::info!(worker_id = %self.worker_id, "worker_deregistered");
    }

    /// Handle a single `WorkerEvent` from the worker.
    ///
    /// Returns `true` if the main loop should break (on `Dying` event),
    /// `false` otherwise. Transitions the worker's status based on the event type:
    /// - `Ready`: status → `Idle`, log `worker_ready`.
    /// - `Dying`: status → `Dead`, log `worker_dying`, return `true` to break the loop.
    /// - `Completed`/`Failed`/`Cancelled`: status → `Idle`, log completion/failure/cancellation.
    /// - `Pong`: forward to the keepalive watchdog's pong channel via `try_send`
    ///   (best-effort; a failed send is not an error — the watchdog will timeout
    ///   if it misses a Pong).
    /// - Other events: log at DEBUG level.
    ///
    /// The `init_timeout` branch in `run()`'s `select!` is disarmed once a `Ready`
    /// event is seen (tracked via `run()`'s own `initialized` flag, not by this
    /// method) — see `run()`'s doc comment for the mechanism.
    async fn handle_event(&mut self, _id: &str, event: WorkerEvent) -> bool {
        // Clone the event for forwarding to the watchdog before the match.
        // The match borrows `event` via `&event`, so we can't move it
        // into `try_send()` without first cloning.
        let pong_forward = event.clone();
        match &event {
            WorkerEvent::Ready { .. } => {
                // Worker successfully initialized. The Initializing timeout is
                // disarmed by run()'s own select! precondition (`initialized`),
                // set the moment this event is matched in the recv() branch —
                // not here, since handle_event() doesn't own that state.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, "worker_ready");
                false
            }
            WorkerEvent::Dying { reason } => {
                // Worker is terminating — the worker itself reported this, so it
                // is already gone: transition straight to Dead (not the Dying
                // status, which is reserved for a supervisor-initiated shutdown
                // still in flight — see the shutdown_rx branch in run()).
                *self.status.write().await = WorkerStatus::Dead;
                tracing::info!(worker_id = %self.worker_id, reason = %reason, "worker_dying");
                true
            }
            WorkerEvent::Completed { job_id, elapsed_ms } => {
                // Job completed successfully — transition back to Idle so the
                // worker can accept the next job.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, elapsed_ms = %elapsed_ms, "job_completed");
                false
            }
            WorkerEvent::Failed {
                job_id,
                error,
                traceback,
            } => {
                // Job failed — transition back to Idle. The traceback is logged
                // at DEBUG level for diagnostic purposes.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, error = %error, "job_failed");
                if let Some(tb) = traceback {
                    tracing::debug!(worker_id = %self.worker_id, traceback = %tb, "job failure traceback");
                }
                false
            }
            WorkerEvent::Cancelled { job_id } => {
                // Job was cancelled by the client — transition back to Idle.
                *self.status.write().await = WorkerStatus::Idle;
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, "job_cancelled");
                false
            }
            WorkerEvent::Pong { seq } => {
                // Forward the Pong to the keepalive watchdog's pong channel.
                // The watchdog filters for matching sequence numbers internally.
                // A failed send (closed/full channel) is best-effort — the watchdog
                // will timeout and declare the worker dead if it misses a Pong,
                // which is the correct failure mode.
                let _ = self.pong_tx.try_send(pong_forward);
                tracing::debug!(worker_id = %self.worker_id, seq = %seq, "pong_received");
                false
            }
            WorkerEvent::Progress { .. }
            | WorkerEvent::ImageReady { .. }
            | WorkerEvent::MemoryReport { .. } => {
                // These events are informational — log at DEBUG level.
                // They do not affect the worker's lifecycle state.
                tracing::debug!(worker_id = %self.worker_id, event = ?event, "unhandled_event");
                false
            }
        }
    }
}
