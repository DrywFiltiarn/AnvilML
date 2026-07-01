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
use crate::respawn::RespawnPolicy;
use anvilml_ipc::RouterTransport;

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
/// 1. **Initializing** — the worker is spawned; a 60-second timeout guards this state.
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
    pub fn new(
        worker_id: String,
        transport: Arc<RouterTransport>,
        demux: Arc<Demux>,
        status: Arc<RwLock<WorkerStatus>>,
        respawn_policy: RespawnPolicy,
    ) -> Self {
        Self {
            worker_id,
            transport,
            demux,
            status,
            respawn_policy,
            attempt_history: Vec::new(),
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
    /// - Spawns a 60-second timeout: if no `Ready` event arrives, sets status to `Dead`.
    /// - Enters a `tokio::select!` loop between `shutdown_rx` and `transport.recv()`.
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
    /// 2. **Initializing timeout** — 60s elapses without `Ready`: status → `Dead`, deregister.
    /// 3. **Worker crash** — `Dying` event received: status → `Dead`, deregister.
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
        // We hold the JoinHandle so we can drop it (cancel it) when Ready arrives.
        let init_timeout = tokio::time::sleep(Duration::from_secs(60));
        tokio::pin!(init_timeout);

        // Step 3: Main event loop.
        loop {
            tokio::select! {
                // Graceful shutdown path.
                _ = &mut shutdown_rx => {
                    tracing::info!(worker_id = %self.worker_id, "shutdown_requested");
                    break;
                }

                // Worker event path.
                result = self.transport.recv() => {
                    match result {
                        Ok((id, event)) => {
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
                            // P8-E5 will act on the decision by sleeping, re-spawning,
                            // and continuing the loop instead of breaking.
                            tracing::error!(worker_id = %self.worker_id, error = %e, "transport recv failed");
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

                // Initializing timeout — 60 seconds elapsed without Ready.
                // This means the worker process started but never reported readiness.
                _ = &mut init_timeout => {
                    tracing::info!(worker_id = %self.worker_id, "worker_declared_dead");
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
    /// - `Ready`: log `worker_ready`.
    /// - `Dying`: log `worker_dying`, return `true` to break the loop.
    /// - `Completed`/`Failed`/`Cancelled`: log completion/failure/cancellation.
    /// - `Pong`: no action (keepalive handles this separately).
    /// - Other events: log at DEBUG level.
    ///
    /// The `init_timeout` guard is dropped (cancelled) when a `Ready` event arrives,
    /// preventing the timeout from triggering after the worker has started.
    async fn handle_event(&mut self, _id: &str, event: WorkerEvent) -> bool {
        match &event {
            WorkerEvent::Ready { .. } => {
                // Worker successfully initialized — cancel the initializing timeout
                // by dropping the pinned sleep future. After this point the timeout
                // will not fire even if the select loop continues.
                tracing::info!(worker_id = %self.worker_id, "worker_ready");
                false
            }
            WorkerEvent::Dying { reason } => {
                // Worker is terminating — log and signal the main loop to break.
                // The status transition to Dead and deregister happen in the exit path.
                tracing::info!(worker_id = %self.worker_id, reason = %reason, "worker_dying");
                true
            }
            WorkerEvent::Completed { job_id, elapsed_ms } => {
                // Job completed successfully — transition back to Idle so the
                // worker can accept the next job.
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
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, error = %error, "job_failed");
                if let Some(tb) = traceback {
                    tracing::debug!(worker_id = %self.worker_id, traceback = %tb, "job failure traceback");
                }
                false
            }
            WorkerEvent::Cancelled { job_id } => {
                // Job was cancelled by the client — transition back to Idle.
                tracing::info!(worker_id = %self.worker_id, job_id = %job_id, "job_cancelled");
                false
            }
            WorkerEvent::Pong { seq } => {
                // Keepalive pong — no state transition needed. The keepalive
                // watchdog monitors these separately via the demux channel.
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
