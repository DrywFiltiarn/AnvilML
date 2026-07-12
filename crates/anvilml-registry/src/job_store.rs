//! JobStore — SQLite-backed persistence for `Job` records.
//!
//! Provides `JobStore`, the single persistence layer for job metadata.
//! All methods operate on the `jobs` table created by migration `003_jobs.sql`.
//! Jobs are serialized to JSON TEXT columns (`graph`, `settings`) and stored
//! with RFC 3339 timestamps for reliable roundtripping.

use anvilml_core::{AnvilError, Job, JobSettings, JobStatus};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

/// SQLite-backed persistence for `Job` records.
///
/// Wraps a `SqlitePool` and provides CRUD operations on the `jobs` table:
/// inserting or replacing a job (`upsert`), fetching a single job by ID
/// (`get`), listing jobs with optional status filter and limit (`list`),
/// and resetting stale `Queued`/`Running` jobs to `Failed` on server
/// restart (`reset_ghost_jobs`).
///
/// The `jobs` table schema is defined in `database/migrations/003_jobs.sql`.
pub struct JobStore {
    /// Database connection pool. All methods acquire a connection from this pool.
    pool: SqlitePool,
}

/// Helper struct for reading `jobs` table rows as raw values.
///
/// `Job` cannot be used directly with `sqlx::query_as!` because it contains
/// `Uuid`, `DateTime<Utc>`, and `serde_json::Value`, which sqlx does not
/// natively map from SQLite column types. This struct captures the raw column
/// values as strings and integers, then `JobStore` converts them to `Job`
/// fields manually.
///
/// The `FromRow` derive is required by sqlx's `query_as` to map SQL columns
/// to struct fields by name.
#[derive(sqlx::FromRow)]
struct JobRow {
    id: String,
    status: String,
    graph: String,
    settings: String,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    worker_id: Option<String>,
    error: Option<String>,
    queue_position: Option<i64>,
}

impl JobStore {
    /// Construct a new `JobStore` backed by the given connection pool.
    ///
    /// # Arguments
    ///
    /// * `pool` — A `SqlitePool` that has already had migrations applied.
    ///   The pool must be connected to a database containing the `jobs` table.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or replace a `Job` row in the `jobs` table.
    ///
    /// Uses `INSERT OR REPLACE` to handle both insert and update in a single
    /// statement, keyed by the `id` primary key. If the row already exists,
    /// it is replaced entirely.
    ///
    /// The `graph` and `settings` fields are serialized to JSON TEXT via
    /// `serde_json::to_string`. The `status` field is serialized to its
    /// snake_case text representation (e.g. `"queued"`). Timestamps are
    /// formatted as RFC 3339 strings.
    ///
    /// # Arguments
    ///
    /// * `job` — The job to persist.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the SQL statement fails (e.g. connection error,
    /// constraint violation).
    /// Returns `AnvilError::Serde` if any serialization step fails (should never
    /// happen with known struct types).
    #[tracing::instrument(fields(id = %job.id), skip(self))]
    pub async fn upsert(&self, job: &Job) -> Result<(), AnvilError> {
        // Serialize graph and settings to JSON TEXT — these are stored as raw
        // JSON strings in the database, not parsed into individual columns.
        let graph_json =
            serde_json::to_string(&job.graph).map_err(|e| AnvilError::Serde(e.to_string()))?;
        let settings_json =
            serde_json::to_string(&job.settings).map_err(|e| AnvilError::Serde(e.to_string()))?;

        // Serialize status to snake_case text via serde_json — matches the
        // #[serde(rename_all = "snake_case")] attribute on JobStatus.
        // Strip the JSON quotes added by serde_json::to_string (e.g. "\"queued\""
        // → "queued") so the stored value is plain text matching the column type.
        let status_text =
            serde_json::to_string(&job.status).map_err(|e| AnvilError::Serde(e.to_string()))?;
        let status_clean = status_text.trim_matches('"');

        // Format all timestamps as RFC 3339 strings for storage.
        let created_at = job.created_at.to_rfc3339();
        let started_at = job.started_at.map(|t| t.to_rfc3339());
        let completed_at = job.completed_at.map(|t| t.to_rfc3339());

        // Cast queue_position from u32 to i64 for the INTEGER column.
        // This is safe because queue_position is always a small positive integer.
        let queue_pos = job.queue_position.map(|p| p as i64);

        sqlx::query(
            "INSERT OR REPLACE INTO jobs \
             (id, status, graph, settings, created_at, started_at, \
              completed_at, worker_id, error, queue_position) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(job.id.to_string())
        .bind(status_clean)
        .bind(graph_json)
        .bind(settings_json)
        .bind(created_at)
        .bind(started_at)
        .bind(completed_at)
        .bind(&job.worker_id)
        .bind(&job.error)
        .bind(queue_pos)
        .execute(&self.pool)
        .await?;

        tracing::debug!(id = %job.id, "upserted job");
        Ok(())
    }

    /// Fetch a single `Job` row by its `id` primary key.
    ///
    /// Returns `Ok(None)` if no row with the given ID exists.
    ///
    /// # Arguments
    ///
    /// * `id` — The job UUID.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the query fails (e.g. connection error).
    #[tracing::instrument(fields(id = %id), skip(self))]
    pub async fn get(&self, id: Uuid) -> Result<Option<Job>, AnvilError> {
        let row = sqlx::query_as::<_, JobRow>(
            "SELECT id, status, graph, settings, created_at, started_at, \
             completed_at, worker_id, error, queue_position \
             FROM jobs WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(self.row_to_job(r))),
            None => Ok(None),
        }
    }

    /// List `Job` rows, optionally filtered by status and limited in count.
    ///
    /// When `status` is `None`, all rows are returned. When `Some(s)`, only rows
    /// whose `status` column matches `s` are returned.
    ///
    /// When `limit` is `Some(n)`, at most `n` rows are returned, ordered by
    /// `created_at ASC` (oldest first). When `limit` is `None`, all matching
    /// rows are returned.
    ///
    /// Returns an empty vector if no rows match.
    ///
    /// # Arguments
    ///
    /// * `status` — Optional status filter.
    /// * `limit` — Optional maximum number of rows to return.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the query fails (e.g. connection error).
    /// Returns `AnvilError::Serde` if the status filter serializes unexpectedly.
    #[tracing::instrument(skip(self))]
    pub async fn list(
        &self,
        status: Option<JobStatus>,
        limit: Option<u32>,
    ) -> Result<Vec<Job>, AnvilError> {
        // Serialize the status filter to snake_case text to match the stored value.
        let status_text = match status {
            Some(s) => {
                let s_text =
                    serde_json::to_string(&s).map_err(|e| AnvilError::Serde(e.to_string()))?;
                Some(s_text.trim_matches('"').to_string())
            }
            None => None,
        };

        // Select the appropriate static query based on which filters are present.
        // sqlx requires `&'static str` for query strings — we cannot use dynamic
        // SQL. Instead, we pick from a small set of pre-built static queries.
        // This is the same pattern used by `ModelStore::list`.
        //
        // The four combinations map to four static queries:
        // 1. No filter, no limit → simplest query
        // 2. No filter, with limit → ORDER BY + LIMIT
        // 3. With status, no limit → WHERE clause only
        // 4. With status, with limit → WHERE + ORDER BY + LIMIT
        let (query_str, has_status, has_limit) = match (&status_text, limit) {
            (None, None) => (
                "SELECT id, status, graph, settings, created_at, started_at, \
                 completed_at, worker_id, error, queue_position FROM jobs",
                false,
                false,
            ),
            (None, Some(_)) => (
                "SELECT id, status, graph, settings, created_at, started_at, \
                 completed_at, worker_id, error, queue_position \
                 FROM jobs ORDER BY created_at ASC LIMIT ?",
                false,
                true,
            ),
            (Some(_), None) => (
                "SELECT id, status, graph, settings, created_at, started_at, \
                 completed_at, worker_id, error, queue_position \
                 FROM jobs WHERE status = ?",
                true,
                false,
            ),
            (Some(_), Some(_)) => (
                "SELECT id, status, graph, settings, created_at, started_at, \
                 completed_at, worker_id, error, queue_position \
                 FROM jobs WHERE status = ? ORDER BY created_at ASC LIMIT ?",
                true,
                true,
            ),
        };

        // Bind parameters based on which filters are present.
        // Parameters are bound in the order they appear in the query string.
        let rows: Vec<JobRow> = if has_status && has_limit {
            // Two parameters: status text, then limit.
            sqlx::query_as::<_, JobRow>(query_str)
                .bind(status_text.unwrap())
                .bind(limit.unwrap() as i64)
                .fetch_all(&self.pool)
                .await?
        } else if has_status {
            // One parameter: status text.
            sqlx::query_as::<_, JobRow>(query_str)
                .bind(status_text.unwrap())
                .fetch_all(&self.pool)
                .await?
        } else if has_limit {
            // One parameter: limit.
            sqlx::query_as::<_, JobRow>(query_str)
                .bind(limit.unwrap() as i64)
                .fetch_all(&self.pool)
                .await?
        } else {
            // No parameters.
            sqlx::query_as::<_, JobRow>(query_str)
                .fetch_all(&self.pool)
                .await?
        };

        Ok(rows.into_iter().map(|r| self.row_to_job(r)).collect())
    }

    /// Delete a `Job` row by its `id` primary key.
    ///
    /// Returns `Ok(())` on success. No error if the row did not exist — SQL
    /// DELETE is a no-op for missing rows.
    ///
    /// This method does not delete associated artifacts — callers should
    /// use `ArtifactStore::list(Some(id))` followed by `ArtifactStore::delete(hash)`
    /// to clean up artifacts before calling this method.
    ///
    /// # Arguments
    ///
    /// * `id` — The job UUID to delete.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the SQL statement fails (e.g. connection error).
    #[tracing::instrument(fields(id = %id), skip(self))]
    pub async fn delete(&self, id: Uuid) -> Result<(), AnvilError> {
        sqlx::query("DELETE FROM jobs WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        tracing::debug!(id = %id, "deleted job");
        Ok(())
    }

    /// Reset stale "ghost" jobs that were `Queued` or `Running` at server shutdown.
    ///
    /// When the server crashes or is restarted, any job that was `Queued` (waiting
    /// for a worker) or `Running` (being executed by a worker) may be left in a
    /// stale state. This method transitions those jobs to `Failed` with
    /// `error = "server_restart"` so they are visible in the job list and can be
    /// retried or discarded by the operator.
    ///
    /// Per `ANVILML_DESIGN.md §19.2`, only `Queued` and `Running` states are
    /// affected — `Completed`, `Failed`, and `Cancelled` jobs are left untouched.
    ///
    /// Returns the number of jobs that were reset.
    ///
    /// # Errors
    ///
    /// Returns `AnvilError::Db` if the SQL statement fails.
    #[tracing::instrument(skip(self))]
    pub async fn reset_ghost_jobs(&self) -> Result<u32, AnvilError> {
        // Update all queued/running jobs to failed with a server_restart marker.
        // The IN clause matches the stored snake_case status text.
        let result = sqlx::query(
            "UPDATE jobs SET status = 'failed', error = 'server_restart' \
             WHERE status IN ('queued', 'running')",
        )
        .execute(&self.pool)
        .await?;

        let count = result.rows_affected();

        if count > 0 {
            tracing::info!(count, "ghost jobs reset to failed");
        }

        Ok(count as u32)
    }

    /// Convert a raw `JobRow` (string/integer fields from SQL) into a
    /// fully-typed `Job` struct.
    ///
    /// Each text field is parsed back through serde for enum types, UUID
    /// is parsed from its text representation, timestamps are parsed from
    /// RFC 3339 strings, and the graph/settings JSON is deserialized.
    fn row_to_job(&self, row: JobRow) -> Job {
        // Parse the UUID from its text representation.
        // The stored value comes from Uuid::to_string(), so it is always valid.
        let id = Uuid::parse_str(&row.id)
            .expect("id should be valid UUID — stored value comes from Uuid::to_string()");

        // Deserialize status from its snake_case text representation.
        // The #[serde(rename_all = "snake_case")] attribute means serde_json
        // produces lowercase text, and deserialization expects the same format.
        // This conversion cannot fail for values that were produced by
        // serde_json::to_string on valid enum variants, so .expect() is safe.
        let status = serde_json::from_str::<JobStatus>(&format!("\"{}\"", row.status))
            .expect("status should parse — stored value comes from serde_json serialization");

        // Parse the graph JSON — stored as a raw JSON string, deserialized to
        // serde_json::Value to preserve the exact structure.
        let graph = serde_json::from_str::<serde_json::Value>(&row.graph)
            .expect("graph should be valid JSON — stored value comes from serde_json::to_string");

        // Parse the settings JSON — stored as a raw JSON string, deserialized
        // to JobSettings. This conversion cannot fail for values that were
        // produced by serde_json::to_string on valid JobSettings.
        let settings = serde_json::from_str::<JobSettings>(&row.settings)
            .expect("settings should parse — stored value comes from serde_json serialization");

        // Parse the RFC 3339 timestamp back to DateTime<Utc>.
        // The stored value comes from DateTime::to_rfc3339(), so it is always valid.
        let created_at = DateTime::parse_from_rfc3339(&row.created_at)
            .expect("created_at should be valid RFC 3339 — stored value comes from to_rfc3339()")
            .with_timezone(&Utc);

        // Parse optional timestamps — these may be None for jobs that have not
        // yet started or completed. When present, the stored value comes from
        // to_rfc3339() and is always valid.
        let started_at = row.started_at.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|t| t.with_timezone(&Utc))
                .ok()
        });

        let completed_at = row.completed_at.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|t| t.with_timezone(&Utc))
                .ok()
        });

        Job {
            id,
            status,
            graph,
            settings,
            created_at,
            started_at,
            completed_at,
            worker_id: row.worker_id,
            error: row.error,
            // Cast from i64 back to u32 — safe because queue_position is
            // always a small positive integer stored as INTEGER.
            queue_position: row.queue_position.map(|p| p as u32),
        }
    }
}
