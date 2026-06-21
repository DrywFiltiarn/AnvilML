//! Central job scheduler that owns the job queue, VRAM ledger, node registry
//! reference, SQLite database pool, and WebSocket event broadcaster.
//!
//! The scheduler is the primary entry point for job submission and query
//! operations. It validates graphs against the node type registry, persists
//! jobs to SQLite, enqueues them for dispatch, and broadcasts WebSocket
//! events to connected clients.
//!
//! **Hard constraints:** Uses `tokio::sync::Mutex` (not `std::sync::Mutex`)
//! for the queue and ledger because they are held across `.await` points in
//! the dispatch loop (Phase 014).

use std::sync::Arc;
use std::time::Duration;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    types::WsEvent, AnvilError, Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse,
};
use anvilml_ipc::{EventBroadcaster, WorkerMessage};
use anvilml_worker::pool::WorkerPool;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use tracing::info;
use uuid::Uuid;

use crate::dag::validate_graph;
use crate::ledger::VramLedger;
use crate::queue::JobQueue;
use anvilml_core::NodeTypeRegistry;

/// Central job scheduler owning the job queue, VRAM ledger, node registry
/// reference, SQLite database pool, and WebSocket event broadcaster.
///
/// This is the primary entry point for job submission and query operations.
/// The scheduler validates computation graphs against the node type registry,
/// persists job records to SQLite, enqueues them for dispatch by the worker
/// pool, and broadcasts WebSocket events to connected clients.
///
/// The `tokio::sync::Mutex` guards on `queue` and `ledger` exist because the
/// dispatch loop (Phase 014) holds these locks across `.await` points when
/// waiting for available workers. A `std::sync::Mutex` would block the Tokio
/// runtime thread, starving other tasks.
pub struct JobScheduler {
    /// FIFO job queue with O(1) cancellation.
    ///
    /// Jobs are pushed here by `submit()` and popped by the dispatch loop.
    queue: Arc<tokio::sync::Mutex<JobQueue>>,

    /// Per-device VRAM reservation tracking ledger.
    ///
    /// The dispatch loop calls `would_fit` before `reserve` to enforce
    /// capacity limits. The ledger itself panics on over-reservation as a
    /// programming error guard.
    // This field is used by the dispatch loop (Phase 014) for VRAM tracking:
    // `would_fit` checks capacity before dispatch, and `reserve`/`release`
    // track per-device reservation totals.
    ledger: Arc<tokio::sync::Mutex<VramLedger>>,

    /// Registry of known node types, populated from worker `Ready` events.
    ///
    /// Used by `submit()` to validate that every node type in the
    /// submitted graph is known to at least one worker.
    node_registry: Arc<NodeTypeRegistry>,

    /// SQLite connection pool for job persistence.
    ///
    /// Jobs are INSERTed here at submission time and queried by `get_job`
    /// and `list_jobs`. The pool is configured with WAL mode and a single
    /// connection for in-memory test databases.
    db: SqlitePool,

    /// WebSocket event broadcaster for pushing state-change events to
    /// connected clients.
    ///
    /// After a job is queued, the scheduler broadcasts a `JobQueued` event
    /// so clients can update their UI with the new job and its queue position.
    broadcaster: Arc<EventBroadcaster>,

    /// Content-addressed artifact storage for generated images.
    ///
    /// The event loop uses this to persist images when `WorkerEvent::ImageReady`
    /// arrives. Stored as `Arc` so it can be shared with the event loop task
    /// at spawn time.
    artifact_store: Arc<ArtifactStore>,

    /// Wake signal for the dispatch loop background task.
    ///
    /// The dispatch loop waits on this `Notify` between periodic polls.
    /// `submit()` calls `notify_one()` after enqueueing a job, waking
    /// the dispatch loop immediately rather than waiting for the next
    /// 200ms periodic poll. This reduces the latency between job
    /// submission and dispatch.
    notify: Arc<tokio::sync::Notify>,

    /// Reference to the worker pool for sending IPC messages to workers.
    ///
    /// Used by `cancel_job` when cancelling a running job — the scheduler
    /// sends a `WorkerMessage::CancelJob` to the owning worker. The dispatch
    /// loop already uses workers (via a parameter), but storing the reference
    /// here allows `cancel_job` to be a direct method on `&self` without
    /// requiring a workers parameter.
    workers: Option<Arc<WorkerPool>>,
}

impl JobScheduler {
    /// Create a new `JobScheduler` with the required dependencies.
    ///
    /// This constructor performs no async work — it simply stores the
    /// provided dependencies. All I/O happens lazily when methods are called.
    ///
    /// # Arguments
    ///
    /// * `queue` — The FIFO job queue for dispatch ordering.
    /// * `ledger` — The VRAM reservation ledger for per-device capacity tracking.
    /// * `node_registry` — The registry of known node types from workers.
    /// * `db` — The SQLite connection pool for job persistence.
    /// * `broadcaster` — The WebSocket event broadcaster for client notifications.
    /// * `artifact_store` — The artifact storage backend for persisting
    ///   generated images when `WorkerEvent::ImageReady` arrives.
    /// * `workers` — The worker pool for sending IPC messages (e.g. job
    ///   cancellation). `None` disables cancellation support — callers
    ///   that need it must pass an `Arc<WorkerPool>`.
    #[tracing::instrument(skip(
        queue,
        ledger,
        node_registry,
        db,
        broadcaster,
        artifact_store,
        workers
    ))]
    pub fn new(
        queue: Arc<tokio::sync::Mutex<JobQueue>>,
        ledger: Arc<tokio::sync::Mutex<VramLedger>>,
        node_registry: Arc<NodeTypeRegistry>,
        db: SqlitePool,
        broadcaster: Arc<EventBroadcaster>,
        artifact_store: Arc<ArtifactStore>,
        workers: Option<Arc<WorkerPool>>,
    ) -> Self {
        Self {
            queue,
            ledger,
            node_registry,
            db,
            broadcaster,
            artifact_store,
            notify: Arc::new(tokio::sync::Notify::new()),
            workers,
        }
    }

    /// Submit a new job: validate the graph, persist to SQLite, enqueue,
    /// and broadcast a `JobQueued` event.
    ///
    /// This is the primary entry point for job submission. The method
    /// performs three stages:
    /// 1. **Validation** — the computation graph is checked against the
    ///    node type registry for structural and semantic correctness.
    /// 2. **Persistence** — the job is INSERTed into the SQLite database
    ///    so it survives server restarts.
    /// 3. **Enqueue** — the job is pushed to the FIFO queue and a
    ///    `JobQueued` WebSocket event is broadcast.
    ///
    /// # Arguments
    ///
    /// * `req` — The job submission request containing the graph JSON and
    ///   settings.
    ///
    /// # Returns
    ///
    /// `Ok(SubmitJobResponse)` with the assigned job ID and queue position
    /// on success. `Err(AnvilError::InvalidGraph(_))` if the graph fails
    /// validation. `Err(AnvilError::Db(_))` if the database INSERT fails.
    /// `Err(AnvilError::Serde(_))` if JSON serialization fails.
    #[tracing::instrument(skip(self, req), fields(graph_nodes = ?req.graph.get("nodes").and_then(|n| n.get("len").map(|l| l.as_u64()))))]
    pub async fn submit(&self, req: SubmitJobRequest) -> Result<SubmitJobResponse, AnvilError> {
        // Stage 1: validate the computation graph against the node type
        // registry. If validation fails, return early without persisting
        // or enqueuing — we don't store invalid graphs.
        validate_graph(&req.graph, &self.node_registry)
            .await
            .map_err(AnvilError::InvalidGraph)?;

        // Stage 2: generate a unique job ID and construct the Job record.
        // The queue_position is set to the current queue length + 1 (1-based).
        let job_id = Uuid::new_v4();
        let queue_len;

        {
            // Lock the queue briefly to read its length.
            // We need the position before pushing, so we capture it here.
            let queue = self.queue.lock().await;
            queue_len = queue.len() as u32 + 1;
        }

        let now = Utc::now();

        let job = Job {
            id: job_id,
            status: JobStatus::Queued,
            graph: req.graph.clone(),
            settings: req.settings,
            created_at: now,
            started_at: None,
            completed_at: None,
            worker_id: None,
            error: None,
            queue_position: Some(queue_len),
        };

        // Stage 3: persist the job to SQLite before enqueueing.
        // Persisting first ensures that even if the queue push fails,
        // the job record exists in the database for later inspection.
        self.insert_job(&job).await?;

        // Push to the in-memory queue for the dispatch loop.
        self.queue.lock().await.push(job.clone());

        // Wake the dispatch loop so it can pick up this new job
        // immediately rather than waiting for the next periodic poll.
        self.notify.notify_one();

        // Broadcast the JobQueued event so connected WebSocket clients
        // can update their UI with the new job and its position.
        let queue_position = queue_len;
        self.broadcaster.send(WsEvent::JobQueued {
            job_id,
            queue_position,
        });

        // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler: Job
        // dispatched" maps to the job being queued (the first lifecycle event
        // the scheduler logs).
        info!(job_id = %job_id, queue_position = queue_position, "job queued");

        Ok(SubmitJobResponse {
            job_id,
            queue_position,
        })
    }

    /// Query a job by its UUID from the SQLite database.
    ///
    /// Returns `Some(job)` if a job with the given ID exists, `None` if
    /// no matching row was found. The caller (HTTP handler) translates
    /// `None` to a 404 response.
    ///
    /// # Arguments
    ///
    /// * `id` — The UUID of the job to look up.
    ///
    /// # Returns
    ///
    /// `Ok(Some(job))` if found, `Ok(None)` if not found, or
    /// `Err(AnvilError::Db(_))` if the database query fails.
    #[tracing::instrument(skip(self), fields(job_id = %id))]
    pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError> {
        // Query the jobs table by UUID hex string. The id column stores
        // UUIDs as hex strings (32 characters, no dashes), so we convert
        // the UUID to hex for the SQL parameter.
        let row = sqlx::query("SELECT * FROM jobs WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.db)
            .await
            .map_err(AnvilError::Db)?;

        match row {
            Some(row) => {
                // Map the database row to a Job struct. Since Job derives
                // Serialize/Deserialize but not sqlx::FromRow, we manually
                // construct it from the row columns using try_get.
                let job = row_to_job(&row)?;
                Ok(Some(job))
            }
            None => {
                // Job not found in the database — this is not an error;
                // the HTTP handler translates None to 404.
                tracing::debug!(job_id = %id, "job not found in database");
                Ok(None)
            }
        }
    }

    /// Query jobs from the SQLite database with optional filters.
    ///
    /// Builds a dynamic SQL query with optional `WHERE status = ?`,
    /// `ORDER BY created_at DESC`, `LIMIT ?`, and `WHERE created_at < ?`
    /// (before filter) clauses.
    ///
    /// # Arguments
    ///
    /// * `status` — If `Some`, filter to jobs with this status.
    /// * `limit` — If `Some`, maximum number of jobs to return.
    /// * `before` — If `Some`, only return jobs created before this time.
    ///
    /// # Returns
    ///
    /// A vector of matching jobs, ordered by `created_at` descending.
    /// Empty vector if no jobs match the filters.
    /// `Err(AnvilError::Db(_))` if the database query fails.
    pub async fn list_jobs(
        &self,
        status: Option<JobStatus>,
        limit: Option<u32>,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<Job>, AnvilError> {
        // Build the dynamic SQL query using sqlx::QueryBuilder.
        // QueryBuilder safely handles parameter binding and SQL-safe string
        // interpolation, avoiding the injection vulnerability that would
        // arise from manual string concatenation.
        //
        // Key: `push` appends raw SQL text (no `?`), `push_bind` appends
        // a `?` placeholder and the bound value. We build the WHERE clause
        // incrementally: push the column comparison text, then call push_bind
        // for the value. The `?` is added at the end of the current query
        // string by push_bind.
        let mut qb = sqlx::QueryBuilder::new("SELECT * FROM jobs");

        // Collect WHERE condition fragments (without `?`) and their binds.
        // Each fragment is a column comparison like "status = " or
        // "created_at < ". The `?` placeholder is added by push_bind.
        let mut conditions: Vec<(&str, String)> = Vec::new();

        if let Some(s) = status {
            // Status filter — compare the TEXT column against the enum's
            // serialized form (snake_case, e.g. "queued", "running").
            conditions.push(("status = ", status_to_string(s).to_string()));
        }

        if let Some(b) = before {
            // Before filter — only jobs created strictly before this time.
            // Uses the RFC3339 string representation for comparison against
            // the TEXT created_at column.
            conditions.push(("created_at < ", b.to_rfc3339()));
        }

        // Build the WHERE clause from collected conditions.
        // Each condition is pushed as "column_op" text followed by push_bind.
        for (i, (col_op, val)) in conditions.into_iter().enumerate() {
            if i == 0 {
                // First condition — use " WHERE ".
                qb.push(" WHERE ");
            } else {
                // Subsequent conditions — use " AND ".
                qb.push(" AND ");
            }
            // Push the column comparison text, then bind the value.
            // push_bind appends `?` at the end of the current query string.
            qb.push(col_op);
            qb.push_bind(val);
        }

        // Order by created_at descending so the most recent jobs appear first.
        // This is the natural order for a job list UI.
        qb.push(" ORDER BY created_at DESC");

        // Append LIMIT if provided.
        if let Some(l) = limit {
            // For LIMIT, use push_bind which adds a `?` placeholder.
            // The `?` is added by push_bind, not in the pushed text.
            qb.push(" LIMIT ");
            qb.push_bind(l as i64);
        }

        // Execute the query and map each row to a Job.
        // build() returns a Query<'_, Sqlite, SqliteArguments> which
        // can be used with fetch_all to get SqliteRow results.
        let rows: Vec<sqlx::sqlite::SqliteRow> = qb
            .build()
            .fetch_all(&self.db)
            .await
            .map_err(AnvilError::Db)?;

        let jobs: Vec<Job> = rows
            .into_iter()
            .map(|row| row_to_job(&row))
            .collect::<Result<Vec<_>, _>>()?;

        // Routine debug logging for observability.
        tracing::debug!(count = jobs.len(), "list jobs returned {} jobs", jobs.len());

        Ok(jobs)
    }

    /// Rehydrate the in-memory `JobQueue` from SQLite at startup.
    ///
    /// The in-memory queue (`self.queue`) is constructed fresh and empty by
    /// the caller on every process start — it is never itself persisted.
    /// Job *records* survive a restart in SQLite, but a `Queued` job from
    /// a prior process has no entry in the new process's in-memory queue
    /// until this method runs. Without calling this once at startup, two
    /// problems follow: the dispatch loop has nothing to dispatch even
    /// though `Queued` rows exist in the DB, and `submit()`'s
    /// `queue_position` (derived from `self.queue.lock().await.len()`)
    /// undercounts by however many `Queued` jobs already existed before
    /// this process started — in the extreme case of an otherwise-empty
    /// in-memory queue, every first post-restart submission reports
    /// position 1 regardless of how many jobs are still genuinely ahead
    /// of it in SQLite.
    ///
    /// Jobs are loaded in `created_at` ascending order (oldest first) and
    /// pushed in that order, preserving original FIFO dispatch order
    /// across the restart — `list_jobs` returns descending order for the
    /// list-jobs API's UI use case, so the result is reversed here rather
    /// than adding a second sort mode to `list_jobs` for one caller.
    ///
    /// Must be called exactly once, after `JobScheduler::new` and before
    /// `start_dispatch_loop`, and before the server starts accepting
    /// `POST /v1/jobs` requests — calling it after new submissions have
    /// already landed in the queue would duplicate or reorder entries.
    ///
    /// # Returns
    ///
    /// The number of jobs rehydrated into the queue, for startup logging.
    /// `Err(AnvilError::Db(_))` if the underlying `list_jobs` query fails.
    pub async fn rehydrate_queue(&self) -> Result<usize, AnvilError> {
        // Oldest-first so push() reproduces original FIFO order; list_jobs
        // returns newest-first, so reverse after fetching.
        let mut queued_jobs = self.list_jobs(Some(JobStatus::Queued), None, None).await?;
        queued_jobs.reverse();

        let count = queued_jobs.len();
        let mut queue = self.queue.lock().await;
        for job in queued_jobs {
            queue.push(job);
        }
        drop(queue);

        tracing::info!(count, "rehydrated queue from database");

        Ok(count)
    }

    /// Cancel a job by its UUID.
    ///
    /// Handles cancellation differently based on the job's current status:
    /// - **Queued**: Immediately removes the job from the in-memory queue,
    ///   updates the database to `Cancelled`, and broadcasts
    ///   `WsEvent::JobCancelled`.
    /// - **Running**: Sends a `WorkerMessage::CancelJob` IPC message to the
    ///   owning worker via the worker pool. Returns `Ok(())` immediately —
    ///   the actual cancellation is confirmed asynchronously when
    ///   `WorkerEvent::Cancelled` arrives and is processed by the event loop.
    /// - **Terminal** (Completed, Failed, Cancelled): Returns
    ///   `AnvilError::InvalidOperation` (409) — a terminal job cannot be
    ///   cancelled again.
    ///
    /// If the job is not found in the database, returns
    /// `AnvilError::JobNotFound` (404).
    ///
    /// # Arguments
    ///
    /// * `id` — The UUID of the job to cancel.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success. `Err(AnvilError::InvalidOperation(_))` if the job
    /// is in a terminal state. `Err(AnvilError::JobNotFound(_))` if no job
    /// with the given ID exists. `Err(AnvilError::Ipc(_))` if the IPC send
    /// fails when cancelling a running job.
    #[tracing::instrument(skip(self), fields(job_id = %id))]
    pub async fn cancel_job(&self, id: Uuid) -> Result<(), AnvilError> {
        // Look up the job from the database to determine its current status.
        // This is the authoritative source — the in-memory queue may not
        // contain jobs that were dispatched (they're no longer queued).
        let job = self
            .get_job(id)
            .await?
            .ok_or_else(|| AnvilError::JobNotFound(id.to_string()))?;

        match job.status {
            JobStatus::Queued => {
                // Cancel from the in-memory queue — this is O(1) via swap-remove.
                // The queue's cancel() returns true if found, false otherwise.
                // If the job is not in the queue (race with dispatch), fall through.
                let mut queue = self.queue.lock().await;
                if !queue.cancel(id) {
                    // Job was already dispatched (race condition: dispatch
                    // popped it between our get_job() and queue.cancel()).
                    // Fall through to check if it's now Running.
                    drop(queue);
                    // Re-fetch to check if it's now Running.
                    let job = self
                        .get_job(id)
                        .await?
                        .ok_or_else(|| AnvilError::JobNotFound(id.to_string()))?;

                    match job.status {
                        JobStatus::Running => {
                            // Send cancel IPC to the owning worker.
                            self.cancel_running_job(&job).await?;
                        }
                        _ => {
                            // Terminal state — should not happen since we checked above,
                            // but handle it defensively.
                            return Err(AnvilError::InvalidOperation(format!(
                                "job {} is in terminal state {:?}",
                                id, job.status
                            )));
                        }
                    }
                } else {
                    drop(queue);
                    // Update DB status to cancelled.
                    let _ = sqlx::query(
                        "UPDATE jobs SET status = 'cancelled', completed_at = ? WHERE id = ?",
                    )
                    .bind(Utc::now().to_rfc3339())
                    .bind(id.to_string())
                    .execute(&self.db)
                    .await;

                    // Broadcast JobCancelled event.
                    self.broadcaster.send(WsEvent::JobCancelled { job_id: id });

                    // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler:
                    // job cancelled" with job_id field.
                    info!(job_id = %id, "job cancelled");
                }
            }
            JobStatus::Running => {
                self.cancel_running_job(&job).await?;
            }
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
                // Terminal state — cannot cancel a job that has already finished.
                // Return 409 Conflict per the task specification.
                return Err(AnvilError::InvalidOperation(format!(
                    "job {} is in terminal state {:?}",
                    id, job.status
                )));
            }
        }

        Ok(())
    }

    /// Cancel a running job by sending an IPC message to the owning worker.
    ///
    /// The actual cancellation is confirmed asynchronously via the event loop
    /// when `WorkerEvent::Cancelled` arrives. This method returns Ok(())
    /// immediately after sending the message.
    ///
    /// # Arguments
    ///
    /// * `job` — The running job, which provides worker_id and device_index.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Internal` if the worker_id cannot be parsed to
    /// derive a device index, or `AnvilError::Ipc` if the worker pool is
    /// unavailable or the IPC send fails.
    async fn cancel_running_job(&self, job: &Job) -> Result<(), AnvilError> {
        // Derive the device index from the worker_id ("worker-N" → N).
        // The job's worker_id was set by the dispatch loop. We parse it
        // to get the device_index for send_cancel().
        let device_index: u32 = job
            .worker_id
            .as_ref()
            .and_then(|wid| wid.strip_prefix("worker-"))
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| {
                AnvilError::Internal(format!(
                    "cannot derive device_index from worker_id: {:?}",
                    job.worker_id
                ))
            })?;

        // Send the CancelJob message to the owning worker.
        // The worker will set its cancel flag and stop execution.
        // If the send fails (worker disconnected), return an error so the
        // caller can report the failure to the client.
        let workers = self.workers.as_ref().ok_or_else(|| {
            AnvilError::Internal(
                "worker pool not configured; cannot cancel running job".to_string(),
            )
        })?;

        workers.send_cancel(device_index, job.id).await?;

        Ok(())
    }

    /// Register a GPU device's total VRAM capacity with the dispatch
    /// loop's VRAM ledger.
    ///
    /// The `VramLedger` starts empty (no registered devices) when the
    /// scheduler is constructed — registration is a separate step so
    /// that the scheduler itself has no dependency on hardware detection
    /// timing. The caller (`main.rs`, after `detect_all_devices`) must
    /// call this once per detected `GpuDevice` before the dispatch loop
    /// starts, or `VramLedger::would_fit` will return `false` for every
    /// device unconditionally — `would_fit` treats an unregistered
    /// device index as having zero free VRAM, by design (see its doc
    /// comment), since a missing registration is indistinguishable from
    /// "no capacity" without this explicit call. The dispatch loop's
    /// VRAM-fit check (`dispatch_once`) would then silently `break` out
    /// of its dispatch attempt for every queued job on every tick — with
    /// no log line, since a deliberately-empty queue and an
    /// unregistered-device skip are otherwise indistinguishable from the
    /// loop's perspective.
    ///
    /// Calling this more than once for the same device index is a no-op
    /// (delegates to `VramLedger::register_device`, which is itself
    /// idempotent) — safe to call again after a model rescan or device
    /// re-detection without double-counting capacity.
    ///
    /// # Arguments
    ///
    /// * `index` — The GPU device index, matching `GpuDevice::index`.
    /// * `vram_total_mib` — The device's total VRAM in mebibytes, matching
    ///   `GpuDevice::vram_total_mib`.
    pub async fn register_device(&self, index: u32, vram_total_mib: u32) {
        self.ledger
            .lock()
            .await
            .register_device(index, vram_total_mib);
    }

    /// Start the dispatch loop background task.
    ///
    /// This method spawns a tokio task that runs the dispatch loop for the
    /// lifetime of the scheduler. The task wakes on new-job notifications
    /// (from `Notify` triggered by `submit()`) and periodically checks for
    /// idle workers to dispatch queued jobs.
    ///
    /// The caller must store the returned `JoinHandle` and await it on
    /// shutdown to prevent the loop from silently stopping. Dropping the
    /// handle without awaiting detaches the task, which will continue
    /// running until it naturally exits (queue empty and no idle workers).
    ///
    /// # Arguments
    ///
    /// * `workers` — The `WorkerPool` providing idle worker information.
    ///   Shared via `Arc` so the loop can read it concurrently with other
    ///   pool consumers.
    ///
    /// # Returns
    ///
    /// A `JoinHandle<()>` for the background dispatch task. The caller
    /// should store this and await it during shutdown.
    pub fn start_dispatch_loop(&self, workers: Arc<WorkerPool>) -> tokio::task::JoinHandle<()> {
        let queue = Arc::clone(&self.queue);
        let ledger = Arc::clone(&self.ledger);
        let db = self.db.clone();
        let notify = Arc::clone(&self.notify);
        let broadcaster = Arc::clone(&self.broadcaster);

        tokio::spawn(async move {
            // The dispatch loop uses a two-wake mechanism:
            // 1. Notify fires when a new job is enqueued (via submit()).
            // 2. Periodic 200ms poll catches workers that become idle between
            //    job submissions. Without it, the loop would miss the window
            //    where a worker finishes and becomes available.
            // The 200ms interval balances responsiveness against CPU usage.
            loop {
                tokio::select! {
                    _ = notify.notified() => {}
                    _ = tokio::time::sleep(Duration::from_millis(200)) => {}
                }

                Self::dispatch_once(&queue, &ledger, &db, &workers, &broadcaster).await;
            }
        })
    }

    /// Start the event subscription loop background task.
    ///
    /// This method spawns a tokio task that receives `WorkerEvent` messages
    /// from the event broadcaster and processes Completed/Failed events:
    /// updating job status in the database, releasing VRAM reservations,
    /// and broadcasting WebSocket events to clients.
    ///
    /// The caller must store the returned `JoinHandle` and await it on
    /// shutdown to prevent the loop from running indefinitely.
    ///
    /// # Returns
    ///
    /// A `JoinHandle<()>` for the background event loop task. The caller
    /// should store this and await it during shutdown.
    pub fn start_event_loop(&self) -> tokio::task::JoinHandle<()> {
        crate::event_loop::start_event_loop(self)
    }

    /// Return a `MutexGuard` over the internal VRAM ledger.
    ///
    /// This is a test accessor — it exposes the internal ledger for
    /// verification in integration tests.
    #[doc(hidden)]
    pub async fn __ledger(&self) -> tokio::sync::MutexGuard<'_, VramLedger> {
        self.ledger.lock().await
    }

    /// Return a `MutexGuard` over the internal job queue.
    ///
    /// This is a test accessor — it exposes the internal queue for
    /// verification in integration tests.
    #[doc(hidden)]
    pub async fn __queue(&self) -> tokio::sync::MutexGuard<'_, JobQueue> {
        self.queue.lock().await
    }

    /// Return a reference to the internal VRAM ledger.
    ///
    /// The event loop uses this to access the ledger for VRAM release
    /// when processing Completed/Failed events. The reference is cloned
    /// into the event loop task at spawn time.
    #[doc(hidden)]
    pub fn ledger(&self) -> &Arc<tokio::sync::Mutex<VramLedger>> {
        &self.ledger
    }

    /// Return a reference to the internal event broadcaster.
    ///
    /// The event loop uses this to subscribe to worker events and
    /// broadcast WsEvent notifications to WebSocket clients.
    #[doc(hidden)]
    pub fn broadcaster(&self) -> &Arc<EventBroadcaster> {
        &self.broadcaster
    }

    /// Return a reference to the internal artifact store.
    ///
    /// The event loop uses this to persist images when `WorkerEvent::ImageReady`
    /// arrives. Exposed as a test accessor so tests can verify artifact
    /// persistence.
    #[doc(hidden)]
    pub fn artifact_store(&self) -> &Arc<ArtifactStore> {
        &self.artifact_store
    }

    /// Return a clone of the internal SQLite database pool.
    ///
    /// The event loop uses this to query and update job status.
    #[doc(hidden)]
    pub fn db(&self) -> SqlitePool {
        self.db.clone()
    }

    /// Attempt to dispatch one or more jobs from the queue to idle workers.
    ///
    /// Iterates the queue front-to-back. For each queued job:
    /// 1. Find an idle worker via `select_worker`.
    /// 2. If a worker is found: mark job Running in DB, reserve VRAM,
    ///    send `WorkerMessage::Execute` to the worker, and remove from queue.
    /// 3. If no worker is available: skip the job (leave it queued).
    ///
    /// Dispatch continues until the queue is exhausted or no idle workers remain.
    ///
    /// VRAM estimation uses a conservative default of 4096 MiB per job.
    /// Phase 015 will replace this with model-specific metadata.
    async fn dispatch_once(
        queue: &Arc<tokio::sync::Mutex<JobQueue>>,
        ledger: &Arc<tokio::sync::Mutex<VramLedger>>,
        db: &SqlitePool,
        workers: &WorkerPool,
        broadcaster: &Arc<EventBroadcaster>,
    ) {
        let idle_workers = workers.get_idle_workers().await;
        if idle_workers.is_empty() {
            tracing::debug!("no idle workers available for dispatch");
            return;
        }

        // Collect jobs to dispatch (avoid holding queue lock while sending IPC).
        // We peek at the front, decide whether to dispatch, and only pop on success.
        let mut queue = queue.lock().await;
        let mut to_dispatch = Vec::new();

        while let Some(job) = queue.peek_front() {
            // Refresh idle workers list — a worker may have been dispatched
            // to between iterations. This prevents stale snapshot issues.
            let available = workers.get_idle_workers().await;
            if available.is_empty() {
                // No more idle workers — leave remaining jobs queued.
                break;
            }

            // Extract device preference from job settings.
            // Formats: "cuda:0", "rocm:1", "0", etc. We parse the last
            // colon-separated segment as a device index.
            let device_pref = job.settings.device_preference.as_ref().and_then(|s| {
                s.split(':')
                    .next_back()
                    .and_then(|idx| idx.parse::<u32>().ok())
            });

            // VRAM estimate: conservative default of 4096 MiB.
            // This will be replaced by model-specific metadata in Phase 015.
            let vram_estimate = 4096u32;

            // Check if VRAM would fit on any available worker before
            // committing to a dispatch. This prevents selecting a worker
            // only to find out later that VRAM is insufficient.
            {
                let guard = ledger.lock().await;
                let can_fit = available
                    .iter()
                    .any(|(_, idx)| guard.would_fit(*idx, vram_estimate));
                drop(guard);

                if !can_fit {
                    // VRAM insufficient — stop dispatching (jobs are FIFO,
                    // later jobs will also need VRAM).
                    break;
                }
            }

            // Select the best worker for this job.
            // We need the ledger locked for ranking by free VRAM.
            let selected = {
                let guard = ledger.lock().await;
                let total_vram: Vec<u32> = available
                    .iter()
                    .map(|(_, idx)| guard.total_vram(*idx).unwrap_or(0))
                    .collect();
                Self::select_worker(&available, device_pref, &guard, &total_vram)
            };
            let Some((selected_worker_id, device_index)) = selected else {
                // No suitable worker found (e.g. device preference mismatch).
                break;
            };

            // Reserve VRAM for this job on the selected device.
            // The ledger panics on over-reservation as a programming error guard.
            {
                let mut guard = ledger.lock().await;
                guard.reserve(device_index, vram_estimate);
            }

            // Pop the job from the queue now that we've committed to dispatch.
            let dispatched_job = queue.pop_front().expect("peek_front returned Some");

            // Mark job Running in database before sending the execute message.
            // This ensures the job status is persisted even if IPC fails.
            // We need to update the DB directly since insert_job is for new jobs.
            let status_str = "running";
            let _ = sqlx::query("UPDATE jobs SET status = ? WHERE id = ?")
                .bind(status_str)
                .bind(dispatched_job.id.to_string())
                .execute(db)
                .await;

            // Record the worker assignment for lifecycle tracking.
            // This UPDATE sets started_at and worker_id unconditionally.
            // The device_index column (added in migration 002) is set
            // in a separate UPDATE below so that the worker_id update
            // succeeds even on databases that haven't run the migration.
            let worker_id_for_db = selected_worker_id.clone();
            let _ = sqlx::query("UPDATE jobs SET started_at = ?, worker_id = ? WHERE id = ?")
                .bind(dispatched_job.created_at.to_rfc3339())
                .bind(worker_id_for_db.clone())
                .bind(dispatched_job.id.to_string())
                .execute(db)
                .await;

            // Set device_index if the column exists (migration 002+).
            // On older databases this silently fails — the event loop
            // falls back to parsing worker_id ("worker-N" → N).
            let _ = sqlx::query("UPDATE jobs SET device_index = ? WHERE id = ?")
                .bind(device_index as i64)
                .bind(dispatched_job.id.to_string())
                .execute(db)
                .await;

            // Remove queue_position since the job is now dispatched.
            let _ = sqlx::query("UPDATE jobs SET queue_position = NULL WHERE id = ?")
                .bind(dispatched_job.id.to_string())
                .execute(db)
                .await;

            to_dispatch.push((dispatched_job, selected_worker_id, device_index));
        }
        drop(queue);

        // Send Execute messages outside queue lock to avoid blocking
        // other queue operations while IPC is in flight.
        for (job, worker_id, device_index) in to_dispatch {
            let msg = WorkerMessage::Execute {
                job_id: job.id,
                graph: job.graph.clone(),
                settings: job.settings.clone(),
                device_index,
            };
            // If send fails, log and continue — the job remains in the
            // queue (it was popped but the dispatch is incomplete).
            // TODO: P14-A4 will add job re-enqueue on send failure.
            if let Err(e) = workers.send_execute(device_index, &msg).await {
                tracing::warn!(
                    job_id = %job.id,
                    worker_id = %worker_id,
                    error = %e,
                    "failed to send execute message to worker"
                );
                continue;
            }

            // Broadcast JobStarted so WebSocket clients know execution has
            // begun on a specific worker. This is the second event in the
            // lifecycle sequence: JobQueued → JobStarted → ...
            broadcaster.send(WsEvent::JobStarted {
                job_id: job.id,
                worker_id: worker_id.clone(),
            });

            // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler:
            // Job dispatched" with job_id and worker_id fields.
            info!(
                job_id = %job.id,
                worker_id = %worker_id,
                "job dispatched"
            );
        }
    }

    /// Select the best worker for a job from the list of idle workers.
    ///
    /// Worker selection strategy:
    /// 1. If `device_preference` is set, filter idle workers to only those
    ///    matching the preference (exact device_index match), then pick the
    ///    one with the most free VRAM.
    /// 2. If no preference or no matching worker, rank all idle workers by
    ///    free VRAM (total - reserved) descending and pick the top candidate.
    /// 3. If no idle workers exist, return None.
    ///
    /// # Arguments
    ///
    /// * `idle_workers` — List of `(worker_id, device_index)` for idle workers.
    /// * `device_preference` — Optional device index the job prefers.
    /// * `ledger` — The VRAM ledger for reservation tracking.
    /// * `total_vram` — Total VRAM per device (indexed by device_index).
    ///
    /// # Returns
    ///
    /// `Some((worker_id, device_index))` if a suitable worker is found,
    /// `None` if no idle workers are available or none have enough VRAM.
    fn select_worker(
        idle_workers: &[(String, u32)],
        device_preference: Option<u32>,
        ledger: &VramLedger,
        total_vram: &[u32],
    ) -> Option<(String, u32)> {
        // Filter to preferred device if specified.
        let candidates = match device_preference {
            Some(pref) => idle_workers
                .iter()
                .filter(|(_, idx)| *idx == pref)
                .cloned()
                .collect::<Vec<_>>(),
            None => idle_workers.to_vec(),
        };

        if candidates.is_empty() {
            return None;
        }

        // Rank candidates by free VRAM descending — prefer workers with
        // the most available capacity. This spreads load across devices
        // and avoids overloading a single GPU.
        let mut ranked = candidates;
        ranked.sort_by(|a, b| {
            let free_a = total_vram[a.1 as usize]
                .saturating_sub(ledger.reservations().get(&a.1).copied().unwrap_or(0));
            let free_b = total_vram[b.1 as usize]
                .saturating_sub(ledger.reservations().get(&b.1).copied().unwrap_or(0));
            free_b.cmp(&free_a) // descending
        });

        Some(ranked[0].clone())
    }

    /// Insert a job record into the SQLite database.
    ///
    /// Serialises the graph and settings fields as JSON strings, and
    /// timestamps as RFC 3339 strings. This is an internal helper called
    /// by `submit()` before enqueueing the job.
    ///
    /// # Arguments
    ///
    /// * `job` — The job to persist.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(AnvilError::Db(_))` on database error,
    /// or `Err(AnvilError::Serde(_))` if JSON serialization fails.
    async fn insert_job(&self, job: &Job) -> Result<(), AnvilError> {
        // Serialize the graph and settings fields to JSON strings for
        // storage in the TEXT columns. The graph is already a serde_json::Value
        // so we serialise it; the settings struct is also serialised to JSON.
        let graph_json = serde_json::to_string(&job.graph).map_err(|e| {
            // Convert serde_json::Error to AnvilError::Serde with the
            // error message string. The closure captures the error and
            // transforms it into our unified error type.
            AnvilError::Serde(e.to_string())
        })?;
        let settings_json =
            serde_json::to_string(&job.settings).map_err(|e| AnvilError::Serde(e.to_string()))?;

        // Serialise timestamps as RFC 3339 strings for storage in TEXT columns.
        // Nullable fields (started_at, completed_at) are serialised as NULL
        // when None, matching the SQL column's nullable type.
        let created_at = job.created_at.to_rfc3339();
        let started_at = job.started_at.as_ref().map(|t| t.to_rfc3339());
        let completed_at = job.completed_at.as_ref().map(|t| t.to_rfc3339());
        let queue_position = job.queue_position.map(|p| p as i64);

        // Map JobStatus to its snake_case string representation for storage.
        // The database stores status as lowercase (e.g. "queued"), matching
        // the serde default for JobStatus.
        let status_str = match job.status {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        };

        // INSERT the job into the jobs table using positional parameters.
        // The column order matches the migration schema. device_index is
        // NULL at submission time — it is set by the dispatch loop when
        // a worker is assigned to the job.
        sqlx::query(
            "INSERT INTO jobs \
             (id, status, graph, settings, created_at, started_at, \
              completed_at, worker_id, error, queue_position) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(job.id.to_string())
        .bind(status_str)
        .bind(graph_json)
        .bind(settings_json)
        .bind(created_at)
        .bind(started_at)
        .bind(completed_at)
        .bind(job.worker_id.clone())
        .bind(job.error.clone())
        .bind(queue_position)
        .execute(&self.db)
        .await
        .map_err(AnvilError::Db)?;

        Ok(())
    }
}

/// Convert a SQLite database row to a `Job` struct.
///
/// Since `Job` derives `Serialize/Deserialize` but not `sqlx::FromRow`,
/// this helper manually maps each column by name. This is more robust
/// than relying on column index order, which can change if the migration
/// schema is modified.
///
/// # Arguments
///
/// * `row` — A `SqliteRow` from a `SELECT * FROM jobs` query.
///
/// # Returns
///
/// `Ok(Job)` if all fields parse correctly, or
/// `Err(AnvilError::Internal)` on parse failure.
fn row_to_job(row: &sqlx::sqlite::SqliteRow) -> Result<Job, AnvilError> {
    // Read the UUID hex string from the `id` column and parse it.
    // The migration stores UUIDs as hex strings (32 chars, no dashes).
    let id_hex: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_hex)
        .map_err(|e| AnvilError::Internal(format!("invalid UUID in job row: {e}")))?;

    // Parse the status string back into the enum.
    // The database stores status as lowercase snake_case (e.g. "queued"),
    // matching the serde default for JobStatus.
    let status_str: String = row.try_get("status")?;
    let status = match status_str.as_str() {
        "queued" => JobStatus::Queued,
        "running" => JobStatus::Running,
        "completed" => JobStatus::Completed,
        "failed" => JobStatus::Failed,
        "cancelled" => JobStatus::Cancelled,
        other => {
            return Err(AnvilError::Internal(format!(
                "unknown job status in database: {other}"
            )))
        }
    };

    // Deserialize the graph JSON string back to a Value.
    // The graph was stored as a JSON string by insert_job.
    let graph_str: String = row.try_get("graph")?;
    let graph: serde_json::Value =
        serde_json::from_str(&graph_str).map_err(|e| AnvilError::Serde(e.to_string()))?;

    // Deserialize the settings JSON string back to JobSettings.
    let settings_str: String = row.try_get("settings")?;
    let settings: JobSettings =
        serde_json::from_str(&settings_str).map_err(|e| AnvilError::Serde(e.to_string()))?;

    // Parse the created_at timestamp from RFC 3339 string.
    let created_at_str: String = row.try_get("created_at")?;
    let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| AnvilError::Internal(format!("invalid created_at timestamp: {e}")))?;

    // Parse optional timestamps — they are NULL when the field hasn't been
    // set yet. sqlx's try_get returns None for SQL NULL on Option<T>.
    let started_at: Option<String> = row.try_get("started_at")?;
    let started_at = started_at.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    let completed_at: Option<String> = row.try_get("completed_at")?;
    let completed_at = completed_at.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    });

    // Read remaining optional fields.
    let worker_id: Option<String> = row.try_get("worker_id")?;
    let error: Option<String> = row.try_get("error")?;
    let queue_position: Option<i64> = row.try_get("queue_position")?;

    Ok(Job {
        id,
        status,
        graph,
        settings,
        created_at,
        started_at,
        completed_at,
        worker_id,
        error,
        queue_position: queue_position.map(|p| p as u32),
    })
}

/// Convert a `JobStatus` enum to its snake_case string representation.
///
/// Used for filtering in `list_jobs` where the status must be compared
/// against the TEXT column in the database.
fn status_to_string(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}
