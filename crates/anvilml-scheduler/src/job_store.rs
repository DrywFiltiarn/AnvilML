//! Job persistence helpers backed by SQLite via sqlx.
//!
//! Provides insert, get, list, and update_status functions for the `Job` domain type
//! defined in [`anvilml_core::types::job`].

use anvilml_core::types::job::{Job, JobSettings, JobStatus};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

// ── Row type for SELECT queries (all complex types stored as TEXT in SQLite) ────

/// Internal struct representing a single row in the `jobs` table.
#[derive(sqlx::FromRow, Debug)]
struct JobRow {
    id: String,
    status: String,
    graph: String,
    settings: String,
    device_index: Option<i64>,
    created_at: i64,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    worker_id: Option<String>,
    artifact_count: i64,
    error: Option<String>,
}

impl JobRow {
    fn into_job(self) -> Job {
        let id = Uuid::parse_str(&self.id).expect("job id is valid uuid");

        let status = match self.status.as_str() {
            "Running" => JobStatus::Running,
            "Completed" => JobStatus::Completed,
            "Failed" => JobStatus::Failed,
            "Cancelled" => JobStatus::Cancelled,
            _ => JobStatus::Queued,
        };

        let settings: JobSettings =
            serde_json::from_str(&self.settings).expect("settings JSON is valid");

        let graph: Value = serde_json::from_str(&self.graph).expect("graph JSON is valid");

        let created_at = DateTime::<Utc>::from_timestamp(self.created_at, 0)
            .expect("created_at is valid epoch seconds");

        let started_at = self
            .started_at
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));

        let completed_at = self
            .completed_at
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0));

        Job {
            id,
            status,
            graph,
            settings,
            device_index: self.device_index.map(|v| v as u32),
            created_at,
            started_at,
            completed_at,
            worker_id: self.worker_id,
            artifact_count: self.artifact_count as u32,
            error: self.error,
        }
    }
}

// ── Helper functions ───────────────────────────────────────────────────────────

/// Serialise a Job into INSERT parameter values.
fn job_to_params(job: &Job) -> (String, String, i64, Option<String>, Option<String>) {
    let graph_json = serde_json::to_string(&job.graph).expect("graph serialises");
    let settings_json = serde_json::to_string(&job.settings).expect("settings serialises");
    let device_index: i64 = job.device_index.map(|v| v as i64).unwrap_or(-1);
    (
        graph_json,
        settings_json,
        device_index,
        job.worker_id.clone(),
        job.error.clone(),
    )
}

/// Convert a JobStatus enum variant to its string representation.
fn status_to_str(status: &JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "Queued",
        JobStatus::Running => "Running",
        JobStatus::Completed => "Completed",
        JobStatus::Failed => "Failed",
        JobStatus::Cancelled => "Cancelled",
    }
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Insert a new job into the database.
///
/// The job's `created_at` timestamp is set to the current UTC time.
pub async fn insert_job(pool: &SqlitePool, job: &Job) -> Result<Uuid, sqlx::Error> {
    let (graph_json, settings_json, device_index, worker_id, error) = job_to_params(job);

    sqlx::query(
        "INSERT INTO jobs (id, status, graph, settings, device_index, created_at, started_at, completed_at, worker_id, artifact_count, error) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(job.id.to_string())
    .bind("Queued")
    .bind(graph_json)
    .bind(settings_json)
    .bind(device_index)
    .bind(job.created_at.timestamp())
    .bind(job.started_at.map(|t| t.timestamp()))
    .bind(job.completed_at.map(|t| t.timestamp()))
    .bind(worker_id)
    .bind(job.artifact_count as i64)
    .bind(error)
    .execute(pool)
    .await?;

    tracing::debug!(job_id = %job.id, "job inserted into DB");

    Ok(job.id)
}

/// Retrieve a single job by its UUID.
pub async fn get_job(pool: &SqlitePool, id: Uuid) -> Result<Option<Job>, sqlx::Error> {
    let row = sqlx::query_as::<_, JobRow>(
        "SELECT id, status, graph, settings, device_index, created_at, started_at, completed_at, worker_id, artifact_count, error \
         FROM jobs WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into_job()))
}

/// List jobs with optional status filter, limit, and before-cursor.
///
/// * `status` — if `Some`, only return jobs matching that status.
/// * `limit` — maximum number of results (default: 100).
/// * `before` — if `Some`, only return jobs created strictly before this timestamp.
pub async fn list_jobs(
    pool: &SqlitePool,
    status: Option<JobStatus>,
    limit: Option<u32>,
    before: Option<DateTime<Utc>>,
) -> Result<Vec<Job>, sqlx::Error> {
    let status_filter = status.map(|s| status_to_str(&s));
    let limit_val = limit.unwrap_or(100) as i64;

    let rows: Vec<JobRow> = if let Some(before_ts) = before {
        // With before-cursor: add `AND created_at < ?` condition.
        sqlx::query_as::<_, JobRow>(
            "SELECT id, status, graph, settings, device_index, created_at, started_at, completed_at, worker_id, artifact_count, error \
             FROM jobs \
             WHERE (status = ? OR ? IS NULL) AND created_at < ? \
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(status_filter)
        .bind(status_filter)
        .bind(before_ts.timestamp())
        .bind(limit_val)
        .fetch_all(pool)
        .await?
    } else {
        // Without before-cursor: no timestamp filter.
        sqlx::query_as::<_, JobRow>(
            "SELECT id, status, graph, settings, device_index, created_at, started_at, completed_at, worker_id, artifact_count, error \
             FROM jobs \
             WHERE (status = ? OR ? IS NULL) \
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(status_filter)
        .bind(status_filter)
        .bind(limit_val)
        .fetch_all(pool)
        .await?
    };

    Ok(rows.into_iter().map(|r| r.into_job()).collect())
}

/// Update a job's status to the given value.
///
/// Allows transitions from `Queued → Running` (dispatch loop) and from
/// `Running → Completed` / `Running → Failed` (event handler). Jobs that are
/// already in a terminal state are never modified.
///
/// * `started_at` — set when transitioning to `Running`.
/// * `completed_at` — set when transitioning to `Completed`.
/// * `error` — set when transitioning to `Failed`.
/// * `worker_id` — set when dispatching a job to a worker.
pub async fn update_status(
    pool: &SqlitePool,
    id: Uuid,
    new_status: JobStatus,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    error: Option<String>,
    worker_id: Option<String>,
) -> Result<bool, sqlx::Error> {
    let rows_affected = sqlx::query(
        "UPDATE jobs SET status = ?, started_at = COALESCE(?, started_at), \
         completed_at = COALESCE(?, completed_at), error = COALESCE(?, error), \
         worker_id = COALESCE(?, worker_id) \
         WHERE id = ? AND (status = 'Queued' OR status = 'Running')",
    )
    .bind(status_to_str(&new_status))
    .bind(started_at.map(|t| t.timestamp()))
    .bind(completed_at.map(|t| t.timestamp()))
    .bind(error)
    .bind(worker_id)
    .bind(id.to_string())
    .execute(pool)
    .await?;

    tracing::debug!(job_id = %id, status = ?new_status, "job status updated in DB");

    Ok(rows_affected.rows_affected() > 0)
}

/// Retrieve job IDs for deletion based on a status filter.
///
/// * `status_filter` — `None` → all terminal statuses
///   (Completed, Failed, Cancelled); exact match → only that status.
///
/// Returns the list of matching Uuids ordered by creation time (newest first).
/// The caller is responsible for actual deletion (artifact cleanup + row removal).
pub async fn delete_by_status(
    pool: &SqlitePool,
    status_filter: Option<&str>,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows: Vec<(String,)> = if let Some(status) = status_filter {
        // Exact status match — only that specific status.
        sqlx::query_as("SELECT id FROM jobs WHERE status = ? ORDER BY created_at DESC")
            .bind(status)
            .fetch_all(pool)
            .await?
    } else {
        // No filter — all terminal statuses.
        sqlx::query_as(
            "SELECT id FROM jobs \
             WHERE status IN ('Completed','Failed','Cancelled') \
             ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .filter_map(|(id,)| Uuid::parse_str(&id).ok())
        .collect())
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::path::Path;

    /// Create an in-memory SQLite pool and initialise the `jobs` table.
    async fn setup_pool() -> SqlitePool {
        let opts = SqliteConnectOptions::new()
            .filename(Path::new(":memory:"))
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .expect("connect in-memory SQLite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jobs (
                id         TEXT PRIMARY KEY,
                status     TEXT    NOT NULL DEFAULT 'Queued',
                graph      TEXT    NOT NULL,
                settings   TEXT    NOT NULL,
                device_index INTEGER          DEFAULT -1,
                created_at INTEGER   NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                worker_id  TEXT,
                artifact_count INTEGER DEFAULT 0,
                error      TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create jobs table");

        pool
    }

    /// Helper: insert a fully-populated Queued job.
    fn make_queued_job() -> Job {
        let now = Utc::now();
        Job {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            status: JobStatus::Queued,
            graph: serde_json::json!({ "nodes": [{ "id": "load", "type": "ZitLoadPipeline" }] }),
            settings: JobSettings {
                seed: 42,
                steps: 30,
                guidance_scale: 7.5,
                width: 1024,
                height: 1024,
                device_preference: None,
            },
            device_index: Some(0),
            created_at: now,
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_insert_and_get() {
        let pool = setup_pool().await;
        let job = make_queued_job();

        // Insert.
        let inserted_id = insert_job(&pool, &job).await.expect("insert succeeded");
        assert_eq!(inserted_id, job.id);

        // Retrieve.
        let retrieved = get_job(&pool, job.id)
            .await
            .expect("get succeeded")
            .expect("job exists");

        assert_eq!(retrieved.id, job.id);
        assert_eq!(retrieved.status, JobStatus::Queued);
        assert_eq!(retrieved.settings.seed, 42);
        assert_eq!(retrieved.settings.steps, 30);
        assert_eq!(retrieved.device_index, Some(0));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_list_jobs_all() {
        let pool = setup_pool().await;

        // Insert 3 distinct jobs.
        for i in 0..3 {
            let job = Job {
                id: Uuid::parse_str(&format!("550e8400-e29b-41d4-a716-44665544000{}", i)).unwrap(),
                status: JobStatus::Queued,
                graph: serde_json::json!({"nodes": []}),
                settings: JobSettings::default(),
                device_index: None,
                created_at: Utc::now(),
                started_at: None,
                completed_at: None,
                worker_id: None,
                artifact_count: 0,
                error: None,
            };
            insert_job(&pool, &job).await.expect("insert");
        }

        let all = list_jobs(&pool, None, None, None)
            .await
            .expect("list succeeded");
        assert_eq!(all.len(), 3);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_list_jobs_status_filter() {
        let pool = setup_pool().await;

        // Insert mixed-status jobs.
        let queued_job = Job {
            id: Uuid::parse_str("550e8400-0000-0000-0000-000000000001").unwrap(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        let running_job = Job {
            id: Uuid::parse_str("550e8400-0000-0000-0000-000000000002").unwrap(),
            status: JobStatus::Running,
            graph: serde_json::json!({"nodes": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        insert_job(&pool, &queued_job).await.expect("insert queued");
        // For running job, insert directly with Running status.
        let (graph_json, settings_json, device_index, worker_id, error) =
            job_to_params(&running_job);
        sqlx::query(
            "INSERT INTO jobs (id, status, graph, settings, device_index, created_at, started_at, completed_at, worker_id, artifact_count, error) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(running_job.id.to_string())
        .bind("Running")
        .bind(graph_json)
        .bind(settings_json)
        .bind(device_index)
        .bind(running_job.created_at.timestamp())
        .bind(running_job.started_at.map(|t| t.timestamp()))
        .bind(running_job.completed_at.map(|t| t.timestamp()))
        .bind(worker_id)
        .bind(running_job.artifact_count as i64)
        .bind(error)
        .execute(&pool)
        .await
        .expect("insert running");

        let queued_only = list_jobs(&pool, Some(JobStatus::Queued), None, None)
            .await
            .expect("list queued succeeded");
        assert_eq!(queued_only.len(), 1);
        assert_eq!(queued_only[0].status, JobStatus::Queued);

        let running_only = list_jobs(&pool, Some(JobStatus::Running), None, None)
            .await
            .expect("list running succeeded");
        assert_eq!(running_only.len(), 1);
        assert_eq!(running_only[0].status, JobStatus::Running);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_list_jobs_limit() {
        let pool = setup_pool().await;

        // Insert 5 jobs.
        for i in 0..5 {
            let job = Job {
                id: Uuid::parse_str(&format!("550e8400-0000-0000-0000-00000000000{i}")).unwrap(),
                status: JobStatus::Queued,
                graph: serde_json::json!({"nodes": []}),
                settings: JobSettings::default(),
                device_index: None,
                created_at: Utc::now(),
                started_at: None,
                completed_at: None,
                worker_id: None,
                artifact_count: 0,
                error: None,
            };
            insert_job(&pool, &job).await.expect("insert");
        }

        let limited = list_jobs(&pool, None, Some(2), None)
            .await
            .expect("list with limit succeeded");
        assert_eq!(limited.len(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_list_jobs_before_cursor() {
        let pool = setup_pool().await;

        // Use three distinct integer-second timestamps.
        let earlier_ts = Utc::now() - chrono::Duration::seconds(20);
        let cursor_ts = Utc::now() - chrono::Duration::seconds(10);
        let late_ts = Utc::now();

        // Insert an early job.
        let early_job = Job {
            id: Uuid::parse_str("550e8400-0000-0000-0000-000000000100").unwrap(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: earlier_ts,
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        insert_job(&pool, &early_job).await.expect("insert early");

        // Insert a late job.
        let late_job = Job {
            id: Uuid::parse_str("550e8400-0000-0000-0000-000000000200").unwrap(),
            status: JobStatus::Queued,
            graph: serde_json::json!({"nodes": []}),
            settings: JobSettings::default(),
            device_index: None,
            created_at: late_ts,
            started_at: None,
            completed_at: None,
            worker_id: None,
            artifact_count: 0,
            error: None,
        };
        insert_job(&pool, &late_job).await.expect("insert late");

        // Query with before=cursor_ts — should return only the early job
        // (early_ts < cursor_ts < late_ts).
        let filtered = list_jobs(&pool, None, None, Some(cursor_ts))
            .await
            .expect("list with cursor succeeded");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, early_job.id);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_update_status() {
        let pool = setup_pool().await;
        let job = make_queued_job();
        insert_job(&pool, &job).await.expect("insert");

        // Update to Running.
        let running_at = Utc::now();
        let updated = update_status(
            &pool,
            job.id,
            JobStatus::Running,
            Some(running_at),
            None,
            None,
            None,
        )
        .await
        .expect("update succeeded");
        assert!(updated);

        // Verify.
        let retrieved = get_job(&pool, job.id)
            .await
            .expect("get succeeded")
            .expect("job exists");
        assert_eq!(retrieved.status, JobStatus::Running);
        assert!(
            retrieved.started_at.is_some(),
            "started_at should be set after update to Running"
        );
    }
}
