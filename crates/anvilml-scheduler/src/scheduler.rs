use std::sync::Arc;
/// JobScheduler — the central async dispatcher for generation jobs.
///
/// `JobScheduler` owns the in-memory job queue, VRAM ledger, and a `Notify` for
/// waking the dispatch loop. It persists jobs to the database via `JobStore` and
/// validates computation graphs via `NodeTypeRegistry` before accepting them.
///
/// The scheduler uses `tokio::sync::Mutex` for `queue` and `ledger` because they
/// are held across `.await` points during `job_store.upsert()` — a hard requirement
/// per `ANVILML_DESIGN.md §4.7`. The `std::sync::Arc` wrapper on `job_store` and
/// `node_registry` allows sharing between the scheduler and other subsystems without
/// interior mutability at this level.
///
/// # Submit flow
///
/// 1. Workers-available check (reject if empty registry)
/// 2. Graph validation via `validate_graph()`
/// 3. Job construction with a fresh UUID
/// 4. Persistence via `JobStore::upsert()` (async)
/// 5. Enqueue into the in-memory queue
/// 6. Notify the dispatch loop
/// 7. Return the job ID
use std::sync::atomic::{AtomicUsize, Ordering};

use anvilml_artifacts::ArtifactStore;
use anvilml_core::types::worker::WorkerStatus;
use anvilml_core::{AnvilError, Job, JobSettings, JobStatus, NodeTypeRegistry};
use anvilml_ipc::WorkerMessage;
use anvilml_registry::JobStore;
use chrono::Utc;
use serde_json::Value;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::{JobQueue, VramLedger, validate_graph};

/// The outcome of a single `dispatch_one()` attempt.
///
/// Distinguishing these three cases (rather than a bare `bool`) matters for
/// two reasons: (1) only `NoIdleWorkers` is a legitimate reason for
/// `start_dispatch_loop()` to stop iterating a wake cycle's queued jobs early
/// per `ANVILML_DESIGN.md §12.5` — `Failed` must not block subsequent jobs
/// that have their own idle worker available; and (2) `Failed` implies a
/// worker was tentatively marked `Busy` and has since been reverted to
/// `Idle`, which callers rely on to know the worker is immediately eligible
/// for re-selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The job was successfully sent to a worker and is now `Running`.
    Dispatched,
    /// No `Idle` worker was available. The job remains queued unchanged;
    /// no worker status was touched.
    NoIdleWorkers,
    /// A worker was selected, but persistence or the IPC send failed after
    /// the worker was marked `Busy`. The VRAM reservation was released and
    /// the worker's status was reverted to `Idle` before returning. The job
    /// remains queued for retry on a later wake.
    Failed,
}

/// The central async dispatcher for generation jobs.
///
/// `JobScheduler` owns:
/// - An in-memory FIFO `JobQueue` (`tokio::sync::Mutex` for async-safe access).
/// - A `VramLedger` (`tokio::sync::Mutex`) for per-device VRAM tracking.
/// - A `JobStore` (`Arc`) for database-backed job persistence.
/// - A `NodeTypeRegistry` (`Arc`) for graph validation.
/// - A `Notify` (`Arc`) for waking the dispatch loop after each submission.
/// - An `ArtifactStore` (`Arc`) for persisting generated image artifacts.
///
/// The `tokio::sync::Mutex` on `queue` and `ledger` is required because these
/// are held across `.await` points during `job_store.upsert()` — a `std::sync::Mutex`
/// would block the Tokio runtime thread. The `Arc` on `job_store` and `node_registry`
/// allows sharing between the scheduler and other subsystems without interior
/// mutability at this level.
pub struct JobScheduler {
    /// In-memory FIFO queue of jobs awaiting dispatch.
    ///
    /// Protected by `tokio::sync::Mutex` because the dispatch loop (later phase)
    /// will `await` while popping, and `submit()` holds this lock across the
    /// `job_store.upsert().await` call to maintain the invariant that a job is
    /// only popped after it has been enqueued.
    queue: Mutex<JobQueue>,

    /// Per-device VRAM reservation ledger.
    ///
    /// Protected by `tokio::sync::Mutex` for the same reason as `queue`:
    /// async-safe access when the dispatch loop (later phase) needs to
    /// reserve/release VRAM across `.await` points.
    #[allow(dead_code)] // Used by the dispatch loop in P14-A4.
    ledger: Mutex<VramLedger>,

    /// Database-backed job persistence layer.
    ///
    /// Wrapped in `Arc` so it can be shared between the scheduler and other
    /// subsystems (e.g. the API handler) without requiring mutable access.
    job_store: Arc<JobStore>,

    /// Node type registry for graph validation.
    ///
    /// Wrapped in `Arc` so it can be shared between the scheduler and other
    /// subsystems. The registry is populated by workers at `Ready` time and
    /// is read-only for the scheduler.
    node_registry: Arc<NodeTypeRegistry>,

    /// Notifies the dispatch loop when a new job is submitted.
    ///
    /// `submit()` calls `notify_one()` after enqueuing a job, waking a single
    /// waiter (the dispatch loop task, introduced in a later phase).
    dispatch_notify: Arc<Notify>,

    /// Atomic counter tracking how many times `wake_dispatch()` has been called.
    ///
    /// Incremented by `wake_dispatch()` (called from the event loop on every
    /// terminal event) so tests can verify the dispatch loop was woken without
    /// needing to intercept the `Notify` itself. Production callers ignore this
    /// counter — it is test observability only.
    dispatch_wake_count: Arc<AtomicUsize>,

    /// Content-addressed artifact storage for generated image outputs.
    ///
    /// Used by the `event_loop` module to persist decoded PNG images from
    /// `WorkerEvent::ImageReady` events. The field is not accessed directly
    /// by the dispatch loop — it is passed through to `handle_image_ready()`.
    ///
    /// Made `pub(crate)` so the `event_loop` module can access it for spawning
    /// the event loop task.
    #[allow(dead_code)]
    pub(crate) artifact_store: Arc<ArtifactStore>,
}

impl JobScheduler {
    /// Construct a new `JobScheduler` with fresh queue and ledger.
    ///
    /// Creates a new in-memory `JobQueue` and `VramLedger` (both wrapped in
    /// `tokio::sync::Mutex`), wraps the provided `JobStore` in an `Arc`, and
    /// creates a fresh `Notify` for waking the dispatch loop.
    ///
    /// # Arguments
    ///
    /// * `job_store` — The database-backed job persistence layer. The scheduler
    ///   takes ownership and wraps it in an `Arc` for sharing.
    /// * `node_registry` — The node type registry for graph validation. The caller
    ///   constructs this; it is wrapped in an `Arc` for sharing.
    /// * `artifact_store` — The artifact storage backend for persisting generated
    ///   images. Passed through to the `event_loop` module for `ImageReady` handling.
    pub fn new(
        job_store: JobStore,
        node_registry: Arc<NodeTypeRegistry>,
        artifact_store: Arc<ArtifactStore>,
    ) -> Self {
        Self {
            queue: Mutex::new(JobQueue::new()),
            ledger: Mutex::new(VramLedger::new()),
            job_store: Arc::new(job_store),
            node_registry,
            dispatch_notify: Arc::new(Notify::new()),
            dispatch_wake_count: Arc::new(AtomicUsize::new(0)),
            artifact_store,
        }
    }

    /// Submit a job graph for execution.
    ///
    /// Enforces the "no workers = reject" guard, validates the computation graph,
    /// constructs a `Queued` job, persists it to the database, enqueues it in the
    /// in-memory queue, notifies the dispatch loop, and returns the job ID and
    /// its position in the queue.
    ///
    /// # Steps
    ///
    /// 1. **Workers-available check**: If the `NodeTypeRegistry` is empty (no
    ///    workers have reached `Ready`), returns `AnvilError::WorkersUnavailable`
    ///    immediately — per `ANVILML_DESIGN.md §12.2`, an empty registry means
    ///    no worker is available, so reject before any other work.
    /// 2. **Graph validation**: Calls `validate_graph()` which runs all six
    ///    checks (structural root, duplicate IDs, unknown types, dangling edges,
    ///    slot-type compatibility, cycle detection). Converts any `GraphError`
    ///    into `AnvilError::InvalidGraph`.
    /// 3. **Job construction**: Generates a fresh UUID v4, creates a `Job` with
    ///    `status = Queued`, `created_at = Utc::now()`, and `queue_position = Some(1)`.
    /// 4. **Persistence**: Calls `job_store.upsert()` to persist the job to the
    ///    database. This is an async operation that acquires a database connection.
    /// 5. **Enqueue**: Acquires the queue mutex lock and pushes the job into the
    ///    in-memory queue.
    /// 6. **Notify**: Calls `dispatch_notify.notify_one()` to wake the dispatch
    ///    loop task (introduced in a later phase).
    /// 7. **Return**: Returns `Ok((job.id, queue_position))`.
    ///
    /// The critical sequencing is: validate → construct → persist (async) → enqueue
    /// → notify. The queue mutex is held across the `upsert` await to prevent a race
    /// where the dispatch loop (in a future phase) pops the job before it has been
    /// enqueued.
    ///
    /// # Returns
    ///
    /// A `(Uuid, u32)` pair: the job ID and its 1-based position in the queue
    /// (i.e. `queue.len()` after the push). The queue position is captured while
    /// the mutex is held, so it is accurate at the time of submission.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::WorkersUnavailable` if no workers are registered.
    /// Returns `AnvilError::InvalidGraph` if the graph fails validation.
    /// Returns `AnvilError::Db` if the database persist operation fails.
    #[tracing::instrument(skip(self, graph), fields(job_id))]
    pub async fn submit(
        &self,
        graph: Value,
        settings: JobSettings,
    ) -> Result<(Uuid, u32), AnvilError> {
        // Step a: Workers-available check. An empty registry means no worker has
        // reached Ready state, so we reject the submission before performing any
        // expensive validation or database I/O. Per ANVILML_DESIGN.md §12.2.
        if self.node_registry.is_empty() {
            tracing::warn!("rejecting submit: no workers registered");
            return Err(AnvilError::WorkersUnavailable(
                "no workers registered".into(),
            ));
        }

        // Step b: Graph validation. validate_graph() runs all six checks and
        // returns Err(Vec<GraphError>) if any check fails. We convert the error
        // messages into the AnvilError::InvalidGraph format expected by callers.
        let validated = match validate_graph(graph, &self.node_registry) {
            Ok(vg) => vg,
            Err(graph_errors) => {
                let error_messages: Vec<String> =
                    graph_errors.iter().map(|e| e.to_string()).collect();
                tracing::warn!(
                    errors = ?error_messages,
                    "graph validation failed"
                );
                return Err(AnvilError::InvalidGraph(error_messages));
            }
        };

        // Step c: Construct the Job. Generate a fresh UUID v4 for the job ID.
        // The job starts in Queued status with created_at set to the current
        // time. Optional fields (started_at, completed_at, worker_id, error)
        // are None. queue_position is set to 1 (first in line; updated by
        // the dispatch loop as jobs are popped).
        let job_id = Uuid::new_v4();
        let job = Job {
            id: job_id,
            status: JobStatus::Queued,
            graph: validated.0,
            settings,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            error: None,
            queue_position: Some(1),
        };

        // Step d: Persist the job to the database. This is an async operation
        // that acquires a database connection. We must hold the queue mutex
        // across this await to maintain the invariant that a job is only
        // popped after it has been enqueued.
        self.job_store.upsert(&job).await?;

        // Step e: Enqueue the job into the in-memory FIFO queue. Acquire the
        // tokio::sync::Mutex lock (non-blocking wait since no other task
        // holds it concurrently during submit). Push the job to the back of
        // the deque.
        {
            let mut queue = self.queue.lock().await;
            queue.push(job);
            tracing::debug!(
                job_id = %job_id,
                "enqueued job in memory queue"
            );
        }

        // Step f: Notify the dispatch loop. notify_one() wakes a single waiter
        // (the dispatch loop task, introduced in a later phase). If no task
        // is waiting, this is a no-op.
        self.dispatch_notify.notify_one();

        tracing::info!(job_id = %job_id, "submitted job");

        // Step g: Capture the queue position (1-based index) while still holding
        // the mutex scope is closed above, so we read queue.len() here. This is
        // the exact position this job occupies in the queue at submission time.
        let queue_position = {
            let queue = self.queue.lock().await;
            queue.len() as u32
        };

        // Step h: Return the job ID and its queue position. The caller (HTTP
        // handler) uses this to construct the 202 response body.
        Ok((job_id, queue_position))
    }

    /// Cancel a queued job by its ID.
    ///
    /// Delegates to the in-memory `JobQueue::cancel()` which marks the job as cancelled
    /// (O(1) via HashSet insertion). The job remains in the queue until `pop_front()`
    /// encounters it and discards it — this is the lazy removal that gives cancel() its
    /// O(1) guarantee.
    ///
    /// Returns `Ok(true)` if the ID was newly marked as cancelled, `Ok(false)` if the
    /// ID was already cancelled or not present in the queue. The job may have already
    /// left the queue (e.g. if it completed or was dispatched), in which case the method
    /// still returns `Ok(false)` — the authoritative state for terminal jobs is the
    /// database, not the in-memory queue.
    ///
    /// # Arguments
    ///
    /// * `id` — The job UUID to cancel.
    ///
    /// # Errors
    ///
    /// This method does not return errors; it always returns `Ok(bool)`. It is declared
    /// as `Result<bool, AnvilError>` for API consistency with `get_job()` and to allow
    /// future error propagation (e.g. if database cancellation logging is added).
    #[tracing::instrument(skip(self), fields(job_id = %id))]
    pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError> {
        let mut queue = self.queue.lock().await;
        let cancelled = queue.cancel(id);
        if cancelled {
            tracing::info!(job_id = %id, "cancelled job in queue");
        }
        Ok(cancelled)
    }

    /// Look up a job by its ID from the database.
    ///
    /// Delegates to `JobStore::get()` which queries the `jobs` table. This is the
    /// authoritative source for all jobs, including those that have already left the
    /// in-memory queue (Completed, Failed, Cancelled). A job that is currently in the
    /// queue is also queryable here since it was persisted before being enqueued.
    ///
    /// # Arguments
    ///
    /// * `id` — The job UUID to look up.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the database query fails (e.g. connection error).
    #[tracing::instrument(skip(self), fields(job_id = %id))]
    pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError> {
        let job = self.job_store.get(id).await?;
        if job.is_some() {
            tracing::debug!(job_id = %id, "retrieved job from database");
        }
        Ok(job)
    }

    /// List jobs, optionally filtered by status and limited in count.
    ///
    /// Delegates to `JobStore::list()` which queries the `jobs` table.
    /// This is the authoritative source for all jobs, including those that
    /// have already left the in-memory queue (Completed, Failed, Cancelled).
    ///
    /// # Arguments
    ///
    /// * `status` — Optional status filter. When `None`, all jobs are returned.
    /// * `limit` — Optional maximum number of rows to return. When `None`,
    ///   all matching rows are returned.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the database query fails.
    /// Returns `AnvilError::Serde` if the status filter serializes unexpectedly.
    #[tracing::instrument(skip(self), fields(status, limit))]
    pub async fn list_jobs(
        &self,
        status: Option<JobStatus>,
        limit: Option<u32>,
    ) -> Result<Vec<Job>, AnvilError> {
        let jobs = self.job_store.list(status, limit).await?;
        tracing::debug!(count = jobs.len(), "listed jobs from database");
        Ok(jobs)
    }

    /// Look up the current VRAM reservation for a device index.
    ///
    /// Returns the amount currently reserved (MiB). Returns `0` if the device
    /// has no reservation. Used by the event loop to determine how much
    /// VRAM to release when a terminal event arrives.
    ///
    /// # Arguments
    ///
    /// * `device_index` — The device index to look up.
    #[tracing::instrument(fields(device_index), skip(self))]
    pub(crate) async fn get_reservation(&self, device_index: u32) -> u32 {
        let ledger = self.ledger.lock().await;
        ledger.get_reservation(device_index)
    }

    /// Release the VRAM reservation for a job on the given device.
    ///
    /// Called from the event loop when a terminal `WorkerEvent`
    /// (`Completed`/`Failed`/`Cancelled`) arrives. Acquires the ledger
    /// mutex internally and calls `VramLedger::release()`.
    ///
    /// The reservation amount is the same `vram_free_mib` value that was
    /// reserved at dispatch time — the dispatch path uses `vram_free_mib`
    /// as a placeholder reservation per `ANVILML_DESIGN.md §12.4`.
    ///
    /// # Arguments
    ///
    /// * `device_index` — The device index parsed from the worker's `worker_id`.
    /// * `vram_mib` — The VRAM amount to release (the same value that was
    ///   reserved at dispatch).
    #[tracing::instrument(fields(device_index, vram_mib), skip(self))]
    pub(crate) async fn release_reservation(&self, device_index: u32, vram_mib: u32) {
        let mut ledger = self.ledger.lock().await;
        ledger.release(device_index, vram_mib);
        tracing::debug!(device_index, vram_mib, "released VRAM reservation");
    }

    /// Wake the dispatch loop and record the wake for test observability.
    ///
    /// Calls `dispatch_notify.notify_one()` to wake a single waiter (the
    /// dispatch loop task spawned by `start_dispatch_loop()`). The wake
    /// count is incremented atomically so tests can verify the dispatch
    /// loop was woken without needing to intercept the `Notify` itself.
    ///
    /// Called from the event loop on every terminal event to ensure the
    /// dispatch loop re-evaluates the queue after a worker frees up.
    pub(crate) fn wake_dispatch(&self) {
        self.dispatch_wake_count.fetch_add(1, Ordering::Relaxed);
        self.dispatch_notify.notify_one();
    }

    /// Test accessor: return the current dispatch wake count.
    ///
    /// Only available when the `test-util` feature is enabled. Returns the
    /// total number of times `wake_dispatch()` has been called since the
    /// scheduler was constructed.
    #[cfg(feature = "test-util")]
    pub async fn dispatch_wake_count_test(&self) -> usize {
        self.dispatch_wake_count.load(Ordering::Relaxed)
    }

    /// Update a job's terminal status in the database.
    ///
    /// Fetches the current job row, mutates its `status`, `completed_at`,
    /// and `error` fields, and persists via `upsert()`. Used by the event
    /// loop when a terminal `WorkerEvent` arrives.
    ///
    /// # Arguments
    ///
    /// * `job_id` — The job to update.
    /// * `status` — The terminal `JobStatus` to set.
    /// * `error` — Optional error string (used for `Failed` events).
    #[tracing::instrument(fields(job_id, ?status), skip(self))]
    pub async fn update_job_terminal_status(
        &self,
        job_id: Uuid,
        status: JobStatus,
        error: Option<String>,
    ) {
        match self.job_store.get(job_id).await {
            Ok(Some(mut job)) => {
                job.status = status;
                job.completed_at = Some(Utc::now());
                job.error = error;
                if let Err(e) = self.job_store.upsert(&job).await {
                    tracing::error!(
                        job_id = %job_id,
                        error = %e,
                        status = ?status,
                        "event_loop: failed to persist terminal status"
                    );
                } else {
                    tracing::info!(
                        job_id = %job_id,
                        status = ?status,
                        "event_loop: persisted terminal status"
                    );
                }
            }
            Ok(None) => {
                tracing::warn!(
                    job_id = %job_id,
                    "event_loop: received terminal event for unknown job"
                );
            }
            Err(e) => {
                tracing::error!(
                    job_id = %job_id,
                    error = %e,
                    "event_loop: failed to fetch job for terminal status update"
                );
            }
        }
    }

    /// Attempt to dispatch a single job to an idle worker.
    ///
    /// Implements the two-step worker selection algorithm from
    /// `ANVILML_DESIGN.md §12.5`:
    ///
    /// 1. **Device preference match** — if `job.settings.device_preference`
    ///    is `Some(id)`, find an `Idle` worker whose `worker_id` matches
    ///    and select the first one.
    /// 2. **VRAM ranking** — if no device preference match (or preference
    ///    is `None`), rank all `Idle` workers by `vram_free_mib` descending
    ///    (from `workers.devices()`) and pick the top candidate.
    ///
    /// On a successful match, sets the worker's status to `Busy` immediately,
    /// reserves VRAM via the ledger, transitions the job to `Running`, persists
    /// the updated job, sends `WorkerMessage::Execute` to the selected worker,
    /// and returns `DispatchOutcome::Dispatched`.
    ///
    /// If persistence or the IPC send fails *after* the worker was marked
    /// `Busy`, the VRAM reservation and the worker's `Busy` status are both
    /// rolled back to `Idle` before returning `DispatchOutcome::Failed` — the
    /// `Execute` message was never durably delivered in that case, so no
    /// terminal `WorkerEvent` will ever arrive to trigger the normal
    /// completion-time `Idle` restoration (a later phase's concern). Without
    /// this rollback the worker would be stranded `Busy` indefinitely.
    ///
    /// The IPC-send-failure path additionally reverts the job's *database*
    /// record back to `Queued` — step (iii)'s persist already wrote
    /// `Running` before step (iv)'s send failed, and the caller re-enqueues
    /// the original (still-`Queued`) `Job` on any `Failed` outcome, so
    /// without this second write `get_job()` (DB-authoritative) would
    /// disagree with the in-memory queue about the job's own status.
    ///
    /// # Arguments
    ///
    /// * `job` — The job to attempt dispatching.
    /// * `workers` — The worker pool, used for idle-worker discovery and
    ///   device metadata.
    ///
    /// # Returns
    ///
    /// A `(DispatchOutcome, Option<String>)` pair. The second element is the
    /// selected worker's `worker_id` whenever a selection was made
    /// (`Dispatched` or `Failed`), and `None` for `NoIdleWorkers` (no
    /// selection ever happens). This is exposed primarily for the
    /// `test-util`-gated selection-observability test hook — production
    /// callers only need the outcome.
    ///
    /// * `DispatchOutcome::Dispatched` — the job was sent to a worker.
    /// * `DispatchOutcome::NoIdleWorkers` — no `Idle` worker exists; the job
    ///   remains queued. Per `§12.5`, this is the only condition under which
    ///   the dispatch loop should stop attempting further jobs in a wake cycle,
    ///   since every subsequent job would fail identically.
    /// * `DispatchOutcome::Failed` — a selection was made but persistence or
    ///   IPC send failed; the job remains queued and the worker is `Idle`
    ///   again. This is unrelated to idle-worker availability, so the caller
    ///   must keep attempting subsequent jobs in the same wake cycle.
    #[tracing::instrument(skip(self, job, workers), fields(job_id = %job.id))]
    async fn dispatch_one(
        &self,
        job: &Job,
        workers: &anvilml_worker::WorkerPool,
    ) -> (DispatchOutcome, Option<String>) {
        // Step 1: Collect all idle workers with their handle references.
        // Iterate handles and call status() on each — this acquires a read
        // lock on the shared status, which is async because the lock is
        // tokio::sync::RwLock. The pool is expected to have at most a
        // handful of workers (one per GPU), so this iteration is fast.
        let mut idle_workers: Vec<anvilml_worker::WorkerHandle> = Vec::new();
        for handle in workers.handles() {
            if handle.status().await == WorkerStatus::Idle {
                idle_workers.push(handle.clone());
            }
        }

        // If no idle workers exist, return NoIdleWorkers — the job stays
        // queued. This is not an error condition per the design doc, and is
        // the one outcome that legitimately stops the caller's iteration for
        // this wake cycle.
        if idle_workers.is_empty() {
            tracing::debug!("dispatch_one_no_idle_workers");
            return (DispatchOutcome::NoIdleWorkers, None);
        }

        tracing::debug!(idle_count = idle_workers.len(), "dispatch_one_idle_workers");

        // Step 2a: Device preference match.
        // If the job specifies a device_preference, find an idle worker
        // whose worker_id (device index as string, e.g. "0") matches.
        // The worker_id convention is the bare device index as a string,
        // matching ANVILML_DESIGN.md §12.5.
        let selected = if let Some(preferred_id) = &job.settings.device_preference {
            // Look for an idle worker whose worker_id matches the preference.
            // Use the first match — if multiple workers share the same ID
            // (shouldn't happen in practice), pick the first one.
            idle_workers
                .iter()
                .find(|h| h.worker_id == *preferred_id)
                .cloned()
        } else {
            None
        };

        // Step 2b: VRAM ranking fallback.
        // If no device_preference match (or device_preference is None),
        // rank all idle workers by vram_free_mib descending.
        let selected = match selected {
            Some(handle) => {
                // Device preference matched — log and use it.
                tracing::debug!(
                    worker_id = %handle.worker_id,
                    "dispatch_one_device_preference_match"
                );
                handle
            }
            None => {
                // No device preference match — rank by VRAM.
                // Build a list of (worker_index, vram_free_mib) pairs from
                // workers.devices(). The device at index i corresponds to
                // handles()[i], so we match by index.
                let devices = workers.devices();
                let mut ranked: Vec<(usize, u32)> = Vec::with_capacity(idle_workers.len());

                for (idx, handle) in idle_workers.iter().enumerate() {
                    // Parse worker_id as u32 to look up the device.
                    // worker_id is the bare device index as a string (e.g. "0").
                    if let Ok(device_index) = handle.worker_id.parse::<u32>() {
                        // Find the device in the pool's device list by index.
                        // devices()[i].index == i, so we can index directly.
                        if device_index < devices.len() as u32 {
                            ranked.push((idx, devices[device_index as usize].vram_free_mib));
                        }
                    }
                }

                // Sort descending by vram_free_mib and pick the top.
                // If the ranked list is empty (no devices matched idle workers),
                // fall back to the first idle worker as a best-effort choice.
                if let Some(&(_, _)) = ranked.iter().max_by_key(|&(_, vram)| vram) {
                    // Find the index of the worker with the most VRAM.
                    let best_idx = ranked
                        .iter()
                        .max_by_key(|&(_, vram)| vram)
                        .map(|&(idx, _)| idx)
                        .expect("ranked list is non-empty above");
                    let handle = idle_workers[best_idx].clone();
                    let vram = ranked
                        .iter()
                        .find(|&&(i, _)| i == best_idx)
                        .map(|&(_, v)| v)
                        .unwrap_or(0);
                    tracing::debug!(
                        worker_id = %handle.worker_id,
                        vram_free_mib = vram,
                        "dispatch_one_vram_ranking_select"
                    );
                    handle
                } else {
                    // No devices matched — fall back to first idle worker.
                    // This shouldn't happen in normal operation (devices are
                    // always populated at spawn time), but we handle it gracefully.
                    idle_workers[0].clone()
                }
            }
        };

        // On match: four steps must happen together (per §12.5):
        // (i) Reserve VRAM via ledger
        // (ii) Transition job to Running
        // (iii) Persist to database
        // (iv) Send WorkerMessage::Execute

        let worker_id = selected.worker_id.clone();
        let device_index = worker_id.parse::<u32>().unwrap_or(0); // worker_id is always a valid index string

        // Mark the selected worker as Busy — this prevents the same worker
        // from being selected again in a concurrent dispatch cycle. The
        // status transition happens before VRAM reservation so that if
        // reservation fails, the worker is still marked Busy (the status
        // will be corrected later by the idle-restoration path in a future
        // task). `selected` is a clone of the handle from `idle_workers`,
        // which itself is a clone of the handle from `workers.handles()`.
        // All clones share the same Arc<RwLock<WorkerStatus>>, so
        // set_status() on the clone updates the shared lock that the pool's
        // original handle also reads from.
        selected.set_status(WorkerStatus::Busy).await;
        tracing::debug!(worker_id = %worker_id, "dispatch_one_worker_marked_busy");

        // (i) Reserve VRAM. Acquire the ledger mutex and call reserve()
        // with the device's vram_free_mib as a placeholder reservation.
        // The actual reservation amount will be refined in later tasks
        // based on model metadata. The ledger is advisory, so over-
        // reservation is recoverable via release() on job completion.
        // Compute the reservation amount before acquiring the lock.
        let vram_to_reserve = workers
            .devices()
            .get(device_index as usize)
            .map(|d| d.vram_free_mib)
            .unwrap_or(0);
        {
            let mut ledger = self.ledger.lock().await;
            // Use the device's reported free VRAM as the reservation amount.
            // This is a placeholder — the real amount depends on the model
            // being dispatched, which this task doesn't have access to yet.
            ledger.reserve(device_index, vram_to_reserve);
            tracing::debug!(
                device_index = device_index,
                vram_reserved_mib = vram_to_reserve,
                "dispatch_one_vram_reserved"
            );
        }

        // (ii) Transition job to Running. Set status, worker_id, and
        // started_at timestamp. This is in-memory only — the persistent
        // copy is written in step (iii).
        let mut job = job.clone();
        job.status = JobStatus::Running;
        job.worker_id = Some(worker_id.clone());
        job.started_at = Some(chrono::Utc::now());

        // (iii) Persist the updated job to the database via upsert().
        // This writes the Running status, worker_id, and started_at.
        if let Err(e) = self.job_store.upsert(&job).await {
            tracing::error!(
                job_id = %job.id,
                worker_id = %worker_id,
                error = %e,
                "dispatch_one_upsert_failed"
            );
            // Release the VRAM reservation we just made, since we couldn't
            // persist the job. The ledger is advisory, but we should clean
            // up after ourselves to avoid phantom reservations.
            {
                let mut ledger = self.ledger.lock().await;
                ledger.release(device_index, vram_to_reserve);
            }
            // Revert the worker's status to Idle. The Execute message was
            // never sent (we failed before reaching step (iv)), so no
            // terminal WorkerEvent will ever arrive for this job to trigger
            // the normal completion-time Idle restoration — without this,
            // the worker would be stranded Busy indefinitely.
            selected.set_status(WorkerStatus::Idle).await;
            tracing::warn!(
                worker_id = %worker_id,
                "dispatch_one_worker_reverted_idle_after_upsert_failure"
            );
            return (DispatchOutcome::Failed, Some(worker_id));
        }

        // (iv) Send WorkerMessage::Execute to the selected worker.
        // Build the execute message with job_id, graph, settings, and
        // device_index. The worker will resolve the graph and dispatch
        // node execution.
        let msg = WorkerMessage::Execute {
            job_id: job.id,
            graph: job.graph.clone(),
            settings: job.settings.clone(),
            device_index,
        };

        if let Err(e) = workers.transport().send(&worker_id, &msg).await {
            tracing::error!(
                job_id = %job.id,
                worker_id = %worker_id,
                error = %e,
                "dispatch_one_send_failed"
            );
            // Release VRAM reservation on send failure.
            {
                let mut ledger = self.ledger.lock().await;
                ledger.release(device_index, vram_to_reserve);
            }
            // Revert the worker's status to Idle for the same reason as the
            // upsert-failure branch above: the message was never delivered,
            // so nothing else will ever restore this worker to Idle.
            //
            // Note: a send failure may also indicate the worker process has
            // actually died, in which case Phase 8's own crash-detection
            // path (the child process wait future, §19.4) will independently
            // observe the exit and transition the worker to Dead, superseding
            // this Idle write. The two are not mutually exclusive — reverting
            // to Idle here is always safe in the meantime, since a genuinely
            // dead worker will never be selected successfully regardless of
            // the status recorded here (its Ready/handshake state is what the
            // pool actually depends on for a subsequent respawn).
            selected.set_status(WorkerStatus::Idle).await;
            tracing::warn!(
                worker_id = %worker_id,
                "dispatch_one_worker_reverted_idle_after_send_failure"
            );

            // Revert the job's DB record too. Step (iii)'s upsert already
            // wrote status=Running, worker_id, and started_at *before* this
            // send attempt failed — left as-is, the database would disagree
            // with the in-memory queue (the caller re-enqueues the original,
            // still-Queued `job` parameter on a Failed outcome) about this
            // job's status. get_job() is documented as DB-authoritative
            // (P14-A2), so that mismatch would surface directly to
            // GET /v1/jobs/:id. Best-effort: if this second upsert also
            // fails, the mismatch is logged but not retried here — the next
            // successful dispatch attempt for this job will overwrite it
            // with a consistent Running record anyway.
            let mut reverted = job.clone();
            reverted.status = JobStatus::Queued;
            reverted.worker_id = None;
            reverted.started_at = None;
            if let Err(e2) = self.job_store.upsert(&reverted).await {
                tracing::error!(
                    job_id = %reverted.id,
                    error = %e2,
                    "dispatch_one_failed_to_revert_db_status_after_send_failure"
                );
            }

            return (DispatchOutcome::Failed, Some(worker_id));
        }

        tracing::info!(
            worker_id = %worker_id,
            job_id = %job.id,
            "dispatched job to worker"
        );

        (DispatchOutcome::Dispatched, Some(worker_id))
    }

    /// Start the dispatch loop as a background tokio task.
    ///
    /// The loop waits on `dispatch_notify` (woken by `submit()` via
    /// `notify_one()`), then iterates the queue front-to-back, calling
    /// `dispatch_one()` for each job. On each wake, it processes jobs until
    /// the queue is empty or `dispatch_one()` reports `NoIdleWorkers`. A
    /// `Failed` outcome (persistence or IPC error) requeues only that job
    /// and continues to the next one in the same cycle — per `§12.5`,
    /// idle-worker exhaustion is the only condition that legitimately
    /// applies to every remaining job.
    ///
    /// The loop runs indefinitely until the `JobScheduler` is dropped.
    /// It must not block the async runtime — all operations inside the
    /// loop body are async (queue lock, dispatch_one).
    ///
    /// # Arguments
    ///
    /// * `workers` — The worker pool, passed to `dispatch_one()` for
    ///   worker selection.
    ///
    /// # Returns
    ///
    /// A `JoinHandle<()>` for the spawned task. The caller should store
    /// this handle and await it during shutdown.
    #[tracing::instrument(skip(self, workers), fields(workers_count = workers.handles().len()))]
    pub fn start_dispatch_loop(
        self: Arc<Self>,
        workers: Arc<anvilml_worker::WorkerPool>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                // Wait for notification. notified() is cheap (no lock) and
                // works on Arc<Notify> — multiple callers can share the same
                // Notify without contention.
                self.dispatch_notify.notified().await;
                tracing::debug!("dispatch_loop_wake");

                // Collect all queued jobs while holding the lock briefly,
                // then release the lock before dispatching. This prevents
                // holding the queue mutex across await points in dispatch_one(),
                // which would deadlock if dispatch_one() ever acquires the
                // same mutex (e.g. P14-A4's VRAM ledger reservation).
                let jobs: Vec<Job> = {
                    let mut queue = self.queue.lock().await;
                    let mut jobs = Vec::new();
                    while let Some(job) = queue.pop_front() {
                        jobs.push(job);
                    }
                    jobs
                };

                // Dispatch each collected job without holding the queue lock.
                // Only DispatchOutcome::NoIdleWorkers is a reason to stop
                // early per §12.5 — every remaining job would fail
                // identically since idle-worker availability can't improve
                // mid-iteration. DispatchOutcome::Failed (a persistence or
                // IPC error, unrelated to idle-worker availability) must NOT
                // block subsequent jobs that have their own idle worker
                // available; that job is simply requeued and iteration
                // continues.
                let mut requeue: Vec<Job> = Vec::new();
                let mut jobs_iter = jobs.into_iter();
                while let Some(job) = jobs_iter.next() {
                    let (outcome, _worker_id) = self.dispatch_one(&job, &workers).await;
                    match outcome {
                        DispatchOutcome::Dispatched => {}
                        DispatchOutcome::Failed => {
                            requeue.push(job);
                        }
                        DispatchOutcome::NoIdleWorkers => {
                            requeue.push(job);
                            // Every remaining job in this cycle would also
                            // see zero idle workers, so requeue the rest
                            // unattempted and stop.
                            requeue.extend(jobs_iter);
                            break;
                        }
                    }
                }

                // Push any un-dispatched jobs back to the queue. The dispatch
                // loop then waits on notified() again.
                if !requeue.is_empty() {
                    let mut queue = self.queue.lock().await;
                    for job in requeue {
                        queue.push(job);
                    }
                }
            }
        })
    }

    /// Test helper: expose `dispatch_one()` for integration tests as a
    /// `bool`, preserving the original P14-A4/A5 test suite's assertions
    /// unchanged (`true` iff `DispatchOutcome::Dispatched`).
    ///
    /// `dispatch_one()` is private (not `pub`), so tests in the `tests/`
    /// directory (compiled as a separate crate) cannot call it directly.
    /// This method is `#[cfg(feature = "test-util")]`-gated, matching the
    /// existing pattern for test-only public methods in this crate.
    #[cfg(feature = "test-util")]
    pub async fn dispatch_one_test(&self, job: &Job, workers: &anvilml_worker::WorkerPool) -> bool {
        matches!(
            self.dispatch_one(job, workers).await.0,
            DispatchOutcome::Dispatched
        )
    }

    /// Test helper: expose `dispatch_one()`'s full `DispatchOutcome` for
    /// tests that need to distinguish `Failed` from `NoIdleWorkers` (unlike
    /// `dispatch_one_test()`, which collapses both to `false`).
    #[cfg(feature = "test-util")]
    pub async fn dispatch_one_outcome_test(
        &self,
        job: &Job,
        workers: &anvilml_worker::WorkerPool,
    ) -> DispatchOutcome {
        self.dispatch_one(job, workers).await.0
    }

    /// Test helper: expose both the outcome *and* the selected worker's
    /// `worker_id`, when a selection was made.
    ///
    /// Exists because this test harness has no real IPC peer for
    /// `set_up_test_workers()`-constructed handles, so `transport().send()`
    /// always fails and the (correctly) reverted `Idle` status can no longer
    /// be used to infer *which* worker a selection algorithm picked, the way
    /// earlier tests relied on before the send-failure revert was added.
    /// This gives selection-logic tests (VRAM ranking, device-preference
    /// matching, exclusion of already-Busy workers) a way to assert on the
    /// selection itself, independent of the post-selection revert.
    #[cfg(feature = "test-util")]
    pub async fn dispatch_one_selection_test(
        &self,
        job: &Job,
        workers: &anvilml_worker::WorkerPool,
    ) -> (DispatchOutcome, Option<String>) {
        self.dispatch_one(job, workers).await
    }

    /// Test helper: expose the ledger's reservations map for verifying
    /// that VRAM release happened correctly after terminal events.
    ///
    /// Acquires the ledger mutex and returns a clone of the inner
    /// `HashMap<u32, u32>`. Only available when the `test-util` feature
    /// is enabled.
    #[cfg(feature = "test-util")]
    pub async fn ledger_reservations_test(&self) -> std::collections::HashMap<u32, u32> {
        let ledger = self.ledger.lock().await;
        ledger.reservations().clone()
    }

    /// Test helper: reserve VRAM on the ledger for a specific device.
    ///
    /// Acquires the ledger mutex and calls `reserve()`. Only available
    /// when the `test-util` feature is enabled.
    #[cfg(feature = "test-util")]
    pub async fn reserve_vram_test(&self, device_index: u32, vram_mib: u32) {
        let mut ledger = self.ledger.lock().await;
        ledger.reserve(device_index, vram_mib);
    }

    /// Test helper: persist a job to the database.
    ///
    /// Wraps `job_store.upsert()` for test use. Only available when
    /// the `test-util` feature is enabled.
    #[cfg(feature = "test-util")]
    pub async fn persist_job_test(
        &self,
        job: &anvilml_core::Job,
    ) -> Result<(), anvilml_core::AnvilError> {
        self.job_store.upsert(job).await
    }
}
