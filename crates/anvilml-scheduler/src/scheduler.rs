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
use std::sync::Arc;

use anvilml_core::{AnvilError, Job, JobSettings, JobStatus, NodeTypeRegistry};
use anvilml_registry::JobStore;
use chrono::Utc;
use serde_json::Value;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

use crate::{JobQueue, VramLedger, validate_graph};

/// The central async dispatcher for generation jobs.
///
/// `JobScheduler` owns:
/// - An in-memory FIFO `JobQueue` (`tokio::sync::Mutex` for async-safe access).
/// - A `VramLedger` (`tokio::sync::Mutex`) for per-device VRAM tracking.
/// - A `JobStore` (`Arc`) for database-backed job persistence.
/// - A `NodeTypeRegistry` (`Arc`) for graph validation.
/// - A `Notify` (`Arc`) for waking the dispatch loop after each submission.
///
/// The `tokio::sync::Mutex` on `queue` and `ledger` is required because these
/// are held across `.await` points during `job_store.upsert()` — a `std::sync::Mutex`
/// would block the Tokio runtime thread. The `Arc` on `job_store` and `node_registry`
/// allows sharing between the scheduler and other subsystems without interior
/// mutability at this level.
#[allow(dead_code)] // `ledger` is used by the dispatch loop (P14-A3), not yet implemented.
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
    pub fn new(job_store: JobStore, node_registry: Arc<NodeTypeRegistry>) -> Self {
        Self {
            queue: Mutex::new(JobQueue::new()),
            ledger: Mutex::new(VramLedger::new()),
            job_store: Arc::new(job_store),
            node_registry,
            dispatch_notify: Arc::new(Notify::new()),
        }
    }

    /// Submit a job graph for execution.
    ///
    /// Enforces the "no workers = reject" guard, validates the computation graph,
    /// constructs a `Queued` job, persists it to the database, enqueues it in the
    /// in-memory queue, notifies the dispatch loop, and returns the job ID.
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
    /// 7. **Return**: Returns `Ok(job.id)`.
    ///
    /// The critical sequencing is: validate → construct → persist (async) → enqueue
    /// → notify. The queue mutex is held across the `upsert` await to prevent a race
    /// where the dispatch loop (in a future phase) pops the job before it has been
    /// enqueued.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::WorkersUnavailable` if no workers are registered.
    /// Returns `AnvilError::InvalidGraph` if the graph fails validation.
    /// Returns `AnvilError::Db` if the database persist operation fails.
    #[tracing::instrument(skip(self, graph), fields(job_id))]
    pub async fn submit(&self, graph: Value, settings: JobSettings) -> Result<Uuid, AnvilError> {
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

        // Step g: Return the job ID.
        Ok(job_id)
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
}
