/// Integration tests for `db.rs` — `open()` and `open_in_memory()`.
///
/// These tests verify:
/// - `open()` creates the database file on disk with all five tables.
/// - WAL mode is active after `open()`.
/// - `open_in_memory()` creates an in-memory pool with all five tables.
/// - Ghost-job reset changes Queued/Running jobs to Failed.
/// - Ghost-job reset leaves Completed/Failed jobs unchanged.
///
/// Each test uses its own `open()` or `open_in_memory()` call — no shared
/// database connections. Temp file tests use `tempfile::tempdir()` for
/// unique paths, ensuring complete isolation.
use anvilml_registry::{open, open_in_memory};

use sqlx::Row;

/// Verifies that `open()` creates the database file on disk and all five
/// tables (jobs, models, artifacts, seed_history, device_capabilities)
/// are present in `sqlite_master`.
///
/// Uses a unique temporary directory so no other test interferes.
/// Cleans up the temp directory after the test completes.
#[tokio::test]
async fn test_open_creates_file() {
    let tmpdir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmpdir.path().join("test.db");

    // open() creates the file and runs migrations.
    let pool = open(&db_path).await.expect("open database");

    // The file must exist on disk.
    assert!(db_path.exists(), "database file must exist on disk");

    // Verify all five tables exist via sqlite_master.
    let tables: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master \
         WHERE type='table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("query sqlite_master");

    let expected = [
        "artifacts",
        "device_capabilities",
        "jobs",
        "models",
        "seed_history",
    ];
    for exp in &expected {
        assert!(
            tables.contains(&(*exp).to_string()),
            "table '{}' must exist in sqlite_master",
            exp
        );
    }
    // sqlite_sequence is auto-created by SQLite for AUTOINCREMENT tables
    // (artifacts.id uses AUTOINCREMENT), so we expect 6 tables total.
    assert_eq!(
        tables.len(),
        expected.len() + 1,
        "expected {} tables (5 user + sqlite_sequence), found {}",
        expected.len() + 1,
        tables.len()
    );
}

/// Verifies that WAL journal mode is active after `open()`.
///
/// Queries `PRAGMA journal_mode` and asserts the result is `"wal"`.
/// WAL mode is enabled via `SqliteConnectOptions::journal_mode(Wal)`
/// to provide better concurrent read performance.
#[tokio::test]
async fn test_open_wal_mode() {
    let tmpdir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmpdir.path().join("wal_test.db");

    let pool = open(&db_path).await.expect("open database");

    // Query the journal mode pragma.
    let mode: String = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("query PRAGMA journal_mode");

    assert_eq!(
        mode.to_lowercase(),
        "wal",
        "journal mode must be 'wal', got '{}'",
        mode
    );
}

/// Verifies that `open_in_memory()` creates an in-memory pool with all
/// five tables present.
///
/// This is the primary test for the in-memory pool used by other tests
/// and by test suites that need a clean database.
#[tokio::test]
async fn test_open_in_memory() {
    let pool = open_in_memory().await.expect("open in-memory database");

    // Verify all five tables exist via sqlite_master.
    // sqlite_sequence is auto-created by SQLite for AUTOINCREMENT tables
    // (artifacts.id uses AUTOINCREMENT), so we expect 6 tables total.
    let tables: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master \
         WHERE type='table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("query sqlite_master");

    let expected = [
        "artifacts",
        "device_capabilities",
        "jobs",
        "models",
        "seed_history",
    ];
    for exp in &expected {
        assert!(
            tables.contains(&(*exp).to_string()),
            "table '{}' must exist in sqlite_master",
            exp
        );
    }
    assert_eq!(
        tables.len(),
        expected.len() + 1,
        "expected {} tables (5 user + sqlite_sequence), found {}",
        expected.len() + 1,
        tables.len()
    );
}

/// Verifies that ghost-job reset changes jobs with status `Queued` to
/// `Failed` with `error = 'server_restart'`.
///
/// Uses a file-backed database in a temp directory so that the same
/// database is accessible across pool connections. Inserts a ghost job,
/// then opens a fresh pool on the same file to trigger migrations +
/// ghost-job reset, and verifies the job status changed.
#[tokio::test]
async fn test_ghost_job_reset() {
    let tmpdir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmpdir.path().join("ghost_reset.db");

    // Open a pool and insert a ghost job (Queued status).
    let pool = open(&db_path).await.expect("open database");

    let job_id = "00000000-0000-0000-0000-000000000001";
    sqlx::query(
        "INSERT INTO jobs (id, status, graph, settings, created_at, \
         started_at, completed_at, worker_id, error, queue_position) \
         VALUES (?, 'Queued', '{}', '{}', '2024-01-01T00:00:00Z', \
         '2024-01-01T00:00:01Z', NULL, 'worker-0', NULL, 1)",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .expect("insert ghost job");

    // Drop the pool so the file is flushed and can be re-opened.
    drop(pool);

    // Open a fresh pool on the same file — this triggers migrations
    // (no-op since already applied) + ghost-job reset.
    let pool2 = open(&db_path).await.expect("open fresh database");

    // Verify the job status changed to Failed with error='server_restart'.
    let row = sqlx::query("SELECT status, error FROM jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(&pool2)
        .await
        .expect("query ghost job");

    let status: String = row.get("status");
    let error: Option<String> = row.get("error");

    assert_eq!(status, "Failed", "ghost job status must be 'Failed'");
    assert_eq!(
        error,
        Some("server_restart".to_string()),
        "ghost job error must be 'server_restart'"
    );
}

/// Verifies that ghost-job reset does NOT affect jobs with status
/// `Completed` or `Failed` — only `Queued` and `Running` are targeted.
///
/// Uses a file-backed database in a temp directory so that the same
/// database is accessible across pool connections. Inserts Completed
/// and Failed jobs, then opens a fresh pool on the same file to trigger
/// ghost-job reset, and verifies those jobs are unchanged.
#[tokio::test]
async fn test_ghost_job_noop() {
    let tmpdir = tempfile::tempdir().expect("create temp dir");
    let db_path = tmpdir.path().join("ghost_noop.db");

    let pool = open(&db_path).await.expect("open database");

    let completed_id = "00000000-0000-0000-0000-000000000002";
    let failed_id = "00000000-0000-0000-0000-000000000003";

    // Insert a Completed job and a Failed job.
    sqlx::query(
        "INSERT INTO jobs (id, status, graph, settings, created_at, \
         started_at, completed_at, worker_id, error, queue_position) \
         VALUES (?, 'Completed', '{}', '{}', '2024-01-01T00:00:00Z', \
         '2024-01-01T00:00:01Z', '2024-01-01T00:00:05Z', 'worker-0', \
         NULL, NULL)",
    )
    .bind(completed_id)
    .execute(&pool)
    .await
    .expect("insert completed job");

    sqlx::query(
        "INSERT INTO jobs (id, status, graph, settings, created_at, \
         started_at, completed_at, worker_id, error, queue_position) \
         VALUES (?, 'Failed', '{}', '{}', '2024-01-01T00:00:00Z', \
         '2024-01-01T00:00:01Z', '2024-01-01T00:00:05Z', 'worker-0', \
         'disk full', NULL)",
    )
    .bind(failed_id)
    .execute(&pool)
    .await
    .expect("insert failed job");

    // Drop the pool so the file is flushed and can be re-opened.
    drop(pool);

    // Open a fresh pool on the same file — triggers ghost-job reset.
    let pool2 = open(&db_path).await.expect("open fresh database");

    // Verify the Completed job is unchanged.
    let row = sqlx::query("SELECT status, error FROM jobs WHERE id = ?")
        .bind(completed_id)
        .fetch_one(&pool2)
        .await
        .expect("query completed job");

    let status: String = row.get("status");
    assert_eq!(status, "Completed", "Completed job must remain Completed");

    // Verify the Failed job is unchanged.
    let row = sqlx::query("SELECT status, error FROM jobs WHERE id = ?")
        .bind(failed_id)
        .fetch_one(&pool2)
        .await
        .expect("query failed job");

    let status: String = row.get("status");
    let error: Option<String> = row.get("error");
    assert_eq!(status, "Failed", "Failed job must remain Failed");
    assert_eq!(
        error,
        Some("disk full".to_string()),
        "Failed job error must be unchanged"
    );
}
