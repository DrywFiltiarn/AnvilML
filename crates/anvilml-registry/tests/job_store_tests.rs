//! Integration tests for `JobStore` — CRUD operations on the `jobs` table.
//!
//! Each test creates its own in-memory SQLite pool with migrations applied,
//! so there is no cross-test shared state and no `#[serial]` annotation is needed.

use anvilml_core::{Job, JobSettings, JobStatus};
use anvilml_registry::JobStore;
use chrono::Utc;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

/// Create an in-memory SQLite pool with migrations applied.
///
/// Each test gets its own pool — the in-memory database is isolated per connection
/// by using a unique cache name (uuid-based) so parallel tests don't collide on
/// the shared `:memory:` database.
///
/// The migration from `database/migrations/003_jobs.sql` is applied so the
/// `jobs` table exists before any CRUD operation.
async fn make_pool() -> SqlitePool {
    // Use a unique in-memory database name per test to avoid the shared `:memory:`
    // database problem: without a unique name, all connections in the same process
    // share the same in-memory database, causing cross-test interference.
    let unique_name = uuid::Uuid::new_v4().to_string();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(format!("file:{unique_name}?mode=memory&cache=shared"))
                .create_if_missing(true),
        )
        .await
        .expect("should be able to create in-memory SQLite pool");

    // Apply the migration so the `jobs` table exists.
    // sqlx::migrate!() embeds the migration at compile time; .run() applies
    // any pending migrations (idempotent — running against an already-migrated
    // database is a no-op).
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator.run(&pool).await.expect("migration should succeed");

    pool
}

/// Construct a `Job` with test values.
///
/// All fields are populated with synthetic values suitable for persistence testing.
fn test_job(id: Uuid, status: JobStatus) -> Job {
    Job {
        id,
        status,
        graph: serde_json::json!({ "nodes": [], "links": [] }),
        settings: JobSettings {
            device_preference: Some("cuda".to_string()),
        },
        created_at: Utc::now(),
        started_at: match status {
            JobStatus::Running | JobStatus::Completed | JobStatus::Failed => Some(Utc::now()),
            _ => None,
        },
        completed_at: match status {
            JobStatus::Completed | JobStatus::Failed => Some(Utc::now()),
            _ => None,
        },
        worker_id: match status {
            JobStatus::Running | JobStatus::Completed | JobStatus::Failed => {
                Some("worker-0".to_string())
            }
            _ => None,
        },
        // Only Failed jobs have an error message.
        error: if status == JobStatus::Failed {
            Some("test failure".to_string())
        } else {
            None
        },
        queue_position: match status {
            JobStatus::Queued => Some(1),
            _ => None,
        },
    }
}

/// `upsert` followed by `get` returns the same `Job` values.
///
/// Inserts a job with all fields populated, then retrieves it by ID and asserts
/// that every field (id, status, graph, settings, timestamps, worker_id, error,
/// queue_position) matches the original.
#[tokio::test]
async fn test_upsert_get_roundtrip() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    let job_id = Uuid::new_v4();
    let job = test_job(job_id, JobStatus::Queued);
    store.upsert(&job).await.expect("upsert should succeed");

    let fetched = store
        .get(job_id)
        .await
        .expect("get should succeed")
        .expect("row should exist");

    assert_eq!(fetched.id, job.id);
    assert_eq!(fetched.status, job.status);
    assert_eq!(fetched.settings, job.settings);
    assert_eq!(fetched.worker_id, job.worker_id);
    assert_eq!(fetched.error, job.error);
    assert_eq!(fetched.queue_position, job.queue_position);

    // Graph is a JSON value — compare as strings to verify exact roundtrip.
    assert_eq!(
        serde_json::to_string(&fetched.graph).unwrap(),
        serde_json::to_string(&job.graph).unwrap()
    );

    // Timestamps may differ by a few milliseconds due to time passage.
    let elapsed = fetched
        .created_at
        .signed_duration_since(job.created_at)
        .num_milliseconds()
        .abs();
    assert!(
        elapsed < 2000,
        "created_at should be within 2s of original, diff: {elapsed}ms"
    );
}

/// `list(None, None)` without filters returns all inserted rows.
///
/// Inserts three jobs with different statuses, then calls `list(None, None)`
/// and asserts the result contains exactly 3 rows.
#[tokio::test]
async fn test_list_no_filter() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();
    store
        .upsert(&test_job(id1, JobStatus::Queued))
        .await
        .unwrap();
    store
        .upsert(&test_job(id2, JobStatus::Running))
        .await
        .unwrap();
    store
        .upsert(&test_job(id3, JobStatus::Completed))
        .await
        .unwrap();

    let all = store.list(None, None).await.expect("list should succeed");
    assert_eq!(all.len(), 3, "expected 3 jobs, got {}", all.len());
}

/// `list(Some(status), None)` filters to only matching rows.
///
/// Inserts five jobs across three statuses (Queued, Running, Completed),
/// then calls `list(Some(JobStatus::Queued), None)` and asserts the result
/// contains exactly 2 rows (the queued jobs).
#[tokio::test]
async fn test_list_with_status_filter() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    // Insert 2 Queued, 2 Running, 1 Completed.
    store
        .upsert(&test_job(Uuid::new_v4(), JobStatus::Queued))
        .await
        .unwrap();
    store
        .upsert(&test_job(Uuid::new_v4(), JobStatus::Queued))
        .await
        .unwrap();
    store
        .upsert(&test_job(Uuid::new_v4(), JobStatus::Running))
        .await
        .unwrap();
    store
        .upsert(&test_job(Uuid::new_v4(), JobStatus::Running))
        .await
        .unwrap();
    store
        .upsert(&test_job(Uuid::new_v4(), JobStatus::Completed))
        .await
        .unwrap();

    let queued = store
        .list(Some(JobStatus::Queued), None)
        .await
        .expect("list with filter should succeed");
    assert_eq!(
        queued.len(),
        2,
        "expected 2 queued jobs, got {}",
        queued.len()
    );
    for job in &queued {
        assert_eq!(job.status, JobStatus::Queued);
    }
}

/// `list(None, Some(n))` limits the number of returned rows.
///
/// Inserts five jobs, then calls `list(None, Some(2))` and asserts the
/// result contains at most 2 rows.
#[tokio::test]
async fn test_list_with_limit() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    // Insert 5 jobs.
    for _ in 0..5 {
        store
            .upsert(&test_job(Uuid::new_v4(), JobStatus::Queued))
            .await
            .unwrap();
    }

    let limited = store
        .list(None, Some(2))
        .await
        .expect("list with limit should succeed");
    assert!(
        limited.len() <= 2,
        "expected at most 2 jobs, got {}",
        limited.len()
    );
}

/// `reset_ghost_jobs()` transitions `Queued` jobs to `Failed` with
/// `error = "server_restart"`.
///
/// Inserts a single `Queued` job, calls `reset_ghost_jobs()`, then fetches
/// the job and verifies it is now `Failed` with the correct error message.
#[tokio::test]
async fn test_reset_ghost_jobs_queued_becomes_failed() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    let job_id = Uuid::new_v4();
    let job = test_job(job_id, JobStatus::Queued);
    store.upsert(&job).await.expect("upsert should succeed");

    let count = store
        .reset_ghost_jobs()
        .await
        .expect("reset should succeed");
    assert_eq!(count, 1, "expected 1 ghost job reset, got {}", count);

    let fetched = store
        .get(job_id)
        .await
        .expect("get should succeed")
        .expect("row should exist");

    assert_eq!(fetched.status, JobStatus::Failed);
    assert_eq!(
        fetched.error,
        Some("server_restart".to_string()),
        "error should be 'server_restart'"
    );
}

/// `reset_ghost_jobs()` transitions `Running` jobs to `Failed` with
/// `error = "server_restart"`.
///
/// Inserts a single `Running` job, calls `reset_ghost_jobs()`, then fetches
/// the job and verifies it is now `Failed` with the correct error message.
#[tokio::test]
async fn test_reset_ghost_jobs_running_becomes_failed() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    let job_id = Uuid::new_v4();
    let job = test_job(job_id, JobStatus::Running);
    store.upsert(&job).await.expect("upsert should succeed");

    let count = store
        .reset_ghost_jobs()
        .await
        .expect("reset should succeed");
    assert_eq!(count, 1, "expected 1 ghost job reset, got {}", count);

    let fetched = store
        .get(job_id)
        .await
        .expect("get should succeed")
        .expect("row should exist");

    assert_eq!(fetched.status, JobStatus::Failed);
    assert_eq!(
        fetched.error,
        Some("server_restart".to_string()),
        "error should be 'server_restart'"
    );
}

/// `reset_ghost_jobs()` does not affect `Completed` or `Cancelled` jobs.
///
/// Inserts a `Completed` job, a `Cancelled` job, and a `Queued` job,
/// calls `reset_ghost_jobs()`, then verifies only the `Queued` job changed
/// status to `Failed` with `error = "server_restart"`, while the others
/// are untouched.
#[tokio::test]
async fn test_reset_ghost_jobs_completed_not_affected() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    let completed_id = Uuid::new_v4();
    let cancelled_id = Uuid::new_v4();
    let queued_id = Uuid::new_v4();

    store
        .upsert(&test_job(completed_id, JobStatus::Completed))
        .await
        .unwrap();
    store
        .upsert(&test_job(cancelled_id, JobStatus::Cancelled))
        .await
        .unwrap();
    store
        .upsert(&test_job(queued_id, JobStatus::Queued))
        .await
        .unwrap();

    let count = store
        .reset_ghost_jobs()
        .await
        .expect("reset should succeed");
    assert_eq!(count, 1, "expected 1 ghost job reset, got {}", count);

    // The completed job should be unchanged.
    let fetched_completed = store
        .get(completed_id)
        .await
        .expect("get should succeed")
        .expect("completed row should exist");
    assert_eq!(fetched_completed.status, JobStatus::Completed);
    // Completed jobs have no error message.
    assert!(fetched_completed.error.is_none());

    // The cancelled job should be unchanged.
    let fetched_cancelled = store
        .get(cancelled_id)
        .await
        .expect("get should succeed")
        .expect("cancelled row should exist");
    assert_eq!(fetched_cancelled.status, JobStatus::Cancelled);
    assert!(fetched_cancelled.error.is_none());

    // The queued job should be reset to failed.
    let fetched_queued = store
        .get(queued_id)
        .await
        .expect("get should succeed")
        .expect("queued row should exist");
    assert_eq!(fetched_queued.status, JobStatus::Failed);
    assert_eq!(fetched_queued.error, Some("server_restart".to_string()));
}

/// `reset_ghost_jobs()` on an empty table returns 0.
///
/// Does not insert any rows; directly calls `reset_ghost_jobs()` and
/// asserts that the return value is 0.
#[tokio::test]
async fn test_reset_ghost_jobs_empty_table() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    let count = store
        .reset_ghost_jobs()
        .await
        .expect("reset should succeed");
    assert_eq!(count, 0, "expected 0 ghost jobs reset on empty table");
}

/// `get()` on a non-existent UUID returns `None`.
///
/// Does not insert any rows; directly queries for a nonexistent UUID and
/// asserts that the result is `Ok(None)` rather than an error.
#[tokio::test]
async fn test_get_missing_id_returns_none() {
    let pool = make_pool().await;
    let store = JobStore::new(pool);

    let missing_id = Uuid::new_v4();
    let result = store
        .get(missing_id)
        .await
        .expect("get should not error for missing ID");
    assert!(
        result.is_none(),
        "expected None for nonexistent ID, got {:?}",
        result
    );
}
