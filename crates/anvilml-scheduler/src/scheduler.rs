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

use anvilml_core::{
    types::WsEvent, AnvilError, Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse,
};
use anvilml_ipc::EventBroadcaster;
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
    // This field is owned but not yet used — VRAM checks during submission
    // are out of scope for this task (Phase 013). The dispatch loop in Phase
    // 014 will call `would_fit` before `reserve`.
    #[expect(dead_code, reason = "used in Phase 014 dispatch loop")]
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
    #[tracing::instrument(skip(queue, ledger, node_registry, db, broadcaster))]
    pub fn new(
        queue: Arc<tokio::sync::Mutex<JobQueue>>,
        ledger: Arc<tokio::sync::Mutex<VramLedger>>,
        node_registry: Arc<NodeTypeRegistry>,
        db: SqlitePool,
        broadcaster: Arc<EventBroadcaster>,
    ) -> Self {
        Self {
            queue,
            ledger,
            node_registry,
            db,
            broadcaster,
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
        // The column order matches the migration schema exactly.
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
