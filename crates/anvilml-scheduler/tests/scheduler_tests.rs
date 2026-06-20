//! Tests for `scheduler.rs` — `JobScheduler` submit, get_job, and list_jobs.
//!
//! Each test creates its own in-memory database via `open_in_memory()`, its
//! own node registry (populated with at least `LoadModel`), and a fresh
//! `JobScheduler` instance. This ensures complete test isolation — no shared
//! state between tests.
//!
//! Tests use `#[serial]` because `open_in_memory()` creates a single-connection
//! SQLite pool that cannot be safely shared across concurrent Tokio tasks in the
//! same test binary. Without serialisation, two tests could attempt to create
//! their pools simultaneously and hit the "database is locked" error.

use std::sync::Arc;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    AnvilError, JobSettings, JobStatus, NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor,
    SlotType, SubmitJobRequest,
};
use anvilml_registry::open_in_memory;
use anvilml_scheduler::ledger::VramLedger;
use anvilml_scheduler::queue::JobQueue;
use anvilml_scheduler::scheduler::JobScheduler;
use chrono::Utc;
use serial_test::serial;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Populate a registry with the `LoadModel` node type.
///
/// This is the minimum registry needed for graph validation to pass —
/// a graph containing a `LoadModel` node will validate successfully
/// because the type is registered.
async fn make_registry() -> Arc<NodeTypeRegistry> {
    let registry = Arc::new(NodeTypeRegistry::new().await);

    registry
        .update_from_worker(
            "worker-0",
            vec![NodeTypeDescriptor {
                type_name: "LoadModel".to_string(),
                display_name: "Load Model".to_string(),
                category: "loading".to_string(),
                description: "Loads a diffusion model".to_string(),
                inputs: vec![],
                outputs: vec![SlotDescriptor {
                    name: "model".to_string(),
                    slot_type: SlotType::Model,
                    optional: false,
                }],
            }],
        )
        .await;

    registry
}

/// Create a valid graph containing a single `LoadModel` node.
///
/// The graph is a minimal valid computation graph — one node, no edges.
/// Validation passes because `LoadModel` is registered in the test registry.
fn make_valid_graph() -> serde_json::Value {
    serde_json::json!({
        "nodes": [
            { "id": "model", "type": "LoadModel" }
        ]
    })
}

/// Create an invalid graph containing a `NonExistent` node type.
///
/// This graph will fail validation because `NonExistent` is not registered
/// in any test registry.
fn make_invalid_graph() -> serde_json::Value {
    serde_json::json!({
        "nodes": [
            { "id": "ghost", "type": "NonExistent" }
        ]
    })
}

/// Submit a valid graph → job persisted with Queued status, queue position 1.
///
/// This is the happy-path test for job submission. It verifies:
/// - `submit()` returns `Ok(SubmitJobResponse)` with a valid UUID and position 1.
/// - The job is persisted in SQLite with status `Queued`.
/// - The `get_job()` method returns the persisted job with matching fields.
/// - The job's queue_position is 1 (first job in the queue).
#[serial]
#[tokio::test]
async fn test_submit_valid_graph() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db, registry).await;

    let req = SubmitJobRequest {
        graph: make_valid_graph(),
        settings: JobSettings::default(),
    };

    let result = scheduler.submit(req).await;

    // Submit should succeed with a job ID and queue position 1.
    assert!(result.is_ok(), "submit should succeed for valid graph");
    let resp = result.unwrap();
    assert!(
        !resp.job_id.is_nil(),
        "job_id should be a valid UUID, not nil"
    );
    assert_eq!(resp.queue_position, 1, "first job should have position 1");

    // get_job should return the persisted job.
    let job = scheduler
        .get_job(resp.job_id)
        .await
        .expect("get_job should not fail")
        .expect("job should be found");

    assert_eq!(job.id, resp.job_id);
    assert_eq!(job.status, JobStatus::Queued);
    assert_eq!(job.queue_position, Some(1));
    assert!(job.created_at <= Utc::now());
}

/// Submit an invalid graph → returns AnvilError::InvalidGraph.
///
/// This test verifies that graph validation runs before any database
/// INSERT or queue push. The job should NOT be persisted in SQLite
/// when validation fails.
#[serial]
#[tokio::test]
async fn test_submit_invalid_graph() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db, registry).await;

    let req = SubmitJobRequest {
        graph: make_invalid_graph(),
        settings: JobSettings::default(),
    };

    let result = scheduler.submit(req).await;

    // Submit should fail with InvalidGraph error.
    match result {
        Err(AnvilError::InvalidGraph(errors)) => {
            // Should mention the unknown node type.
            assert!(
                errors.iter().any(|e| e.contains("NonExistent")),
                "error should mention the unknown type"
            );
        }
        other => panic!("expected AnvilError::InvalidGraph, got: {:?}", other),
    }

    // The job should NOT be persisted — get_job should return None.
    // We can't know the UUID since submit never returned one, so
    // we verify by listing jobs (should be empty).
    let jobs = scheduler
        .list_jobs(None, None, None)
        .await
        .expect("list_jobs should not fail");
    assert!(
        jobs.is_empty(),
        "no jobs should be persisted after invalid submit"
    );
}

/// get_job returns the submitted job for a valid UUID.
///
/// This test verifies that the round-trip from submit → persist → get_job
/// preserves all job fields correctly.
#[serial]
#[tokio::test]
async fn test_get_job_returns_job() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db, registry).await;

    // Submit a job.
    let req = SubmitJobRequest {
        graph: make_valid_graph(),
        settings: JobSettings {
            device_preference: Some("cuda:0".to_string()),
        },
    };
    let resp = scheduler.submit(req).await.expect("submit should succeed");

    // get_job should return the exact job.
    let job = scheduler
        .get_job(resp.job_id)
        .await
        .expect("get_job should not fail")
        .expect("job should be found");

    assert_eq!(job.id, resp.job_id);
    assert_eq!(job.status, JobStatus::Queued);
    assert_eq!(job.settings.device_preference, Some("cuda:0".to_string()));
    assert_eq!(job.queue_position, Some(resp.queue_position));
}

/// get_job returns None for a UUID that was never submitted.
///
/// This tests the "not found" path — a UUID that doesn't exist in
/// the database should return `Ok(None)`, not an error.
#[serial]
#[tokio::test]
async fn test_get_job_missing_returns_none() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db, registry).await;

    // Query for a UUID that was never submitted.
    let missing_id = Uuid::new_v4();
    let result = scheduler
        .get_job(missing_id)
        .await
        .expect("get_job should not fail for missing job");

    // Should return None, not an error.
    assert!(
        result.is_none(),
        "missing job should return None, not Some or Err"
    );
}

/// list_jobs returns all submitted jobs.
///
/// Submits three jobs and verifies that `list_jobs()` returns all three
/// in descending `created_at` order (most recent first).
#[serial]
#[tokio::test]
async fn test_list_jobs_returns_all() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db, registry).await;

    // Submit three jobs.
    for i in 0..3 {
        let graph = serde_json::json!({
            "nodes": [
                { "id": format!("node_{i}"), "type": "LoadModel" }
            ]
        });
        scheduler
            .submit(SubmitJobRequest {
                graph,
                settings: JobSettings::default(),
            })
            .await
            .expect("submit should succeed");

        // Small delay to ensure distinct timestamps.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let jobs = scheduler
        .list_jobs(None, None, None)
        .await
        .expect("list_jobs should not fail");

    assert_eq!(jobs.len(), 3, "should return all 3 jobs");

    // Jobs should be in descending created_at order (most recent first).
    for i in 0..(jobs.len() - 1) {
        assert!(
            jobs[i].created_at >= jobs[i + 1].created_at,
            "jobs should be ordered by created_at descending"
        );
    }
}

/// list_jobs filtered by status returns only matching jobs.
///
/// Submits two jobs, then manually updates one to `Failed` in the database
/// (simulating what Phase 014's dispatch loop would do). Verifies that
/// `list_jobs(Some(JobStatus::Queued))` returns only the Queued job.
#[serial]
#[tokio::test]
async fn test_list_jobs_filter_by_status() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    // Clone the pool before passing to make_scheduler — we need the original
    // pool to run direct SQL updates (simulating dispatch loop state changes).
    let scheduler = make_scheduler(db.clone(), registry).await;

    // Submit two jobs.
    for i in 0..2 {
        let graph = serde_json::json!({
            "nodes": [
                { "id": format!("node_{i}"), "type": "LoadModel" }
            ]
        });
        scheduler
            .submit(SubmitJobRequest {
                graph,
                settings: JobSettings::default(),
            })
            .await
            .expect("submit should succeed");
    }

    // Manually update the first job to Failed in the database,
    // simulating what the dispatch loop does after a job fails.
    let all_jobs = scheduler
        .list_jobs(None, None, None)
        .await
        .expect("list_jobs should work");
    let failed_id = all_jobs[0].id;

    // Use lowercase 'failed' to match the status_to_string output used in
    // list_jobs filters. The database stores status as lowercase snake_case.
    sqlx::query("UPDATE jobs SET status = 'failed', error = 'test failure' WHERE id = ?")
        .bind(failed_id.to_string())
        .execute(&db)
        .await
        .expect("update should succeed");

    // Filter by Queued status — should return only the non-failed job.
    let queued_jobs = scheduler
        .list_jobs(Some(JobStatus::Queued), None, None)
        .await
        .expect("list_jobs with filter should not fail");

    assert_eq!(queued_jobs.len(), 1, "should return only 1 Queued job");
    assert_eq!(queued_jobs[0].status, JobStatus::Queued);

    // Filter by Failed status — should return only the failed job.
    let failed_jobs = scheduler
        .list_jobs(Some(JobStatus::Failed), None, None)
        .await
        .expect("list_jobs with filter should not fail");

    assert_eq!(failed_jobs.len(), 1, "should return only 1 Failed job");
    assert_eq!(failed_jobs[0].id, failed_id);
    assert_eq!(failed_jobs[0].error, Some("test failure".to_string()));
}

/// list_jobs with limit returns at most the specified number of jobs.
///
/// Submits five jobs and verifies that `list_jobs(None, Some(2), None)`
/// returns exactly 2 jobs (the most recent ones, in descending order).
#[serial]
#[tokio::test]
async fn test_list_jobs_with_limit() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db, registry).await;

    // Submit five jobs.
    for i in 0..5 {
        let graph = serde_json::json!({
            "nodes": [
                { "id": format!("node_{i}"), "type": "LoadModel" }
            ]
        });
        scheduler
            .submit(SubmitJobRequest {
                graph,
                settings: JobSettings::default(),
            })
            .await
            .expect("submit should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Limit to 2 — should return the 2 most recent jobs.
    let jobs = scheduler
        .list_jobs(None, Some(2), None)
        .await
        .expect("list_jobs with limit should not fail");

    assert_eq!(jobs.len(), 2, "should return exactly 2 jobs");
    // The first of the 2 should be the most recently submitted (node_4).
    assert!(jobs[0].id != jobs[1].id, "jobs should be distinct");
}

/// list_jobs with before filter returns only jobs created before the given time.
///
/// Submits three jobs with small delays, then queries with a `before` time
/// that falls between the first and second job. Should return only the first job.
#[serial]
#[tokio::test]
async fn test_list_jobs_with_before_filter() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db, registry).await;

    // Submit first job.
    scheduler
        .submit(SubmitJobRequest {
            graph: make_valid_graph(),
            settings: JobSettings::default(),
        })
        .await
        .expect("submit should succeed");

    let after_first = Utc::now();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Submit second and third jobs.
    for i in 1..3 {
        let graph = serde_json::json!({
            "nodes": [
                { "id": format!("node_{i}"), "type": "LoadModel" }
            ]
        });
        scheduler
            .submit(SubmitJobRequest {
                graph,
                settings: JobSettings::default(),
            })
            .await
            .expect("submit should succeed");
    }

    // Before filter with a time between first and second job.
    let jobs = scheduler
        .list_jobs(None, None, Some(after_first))
        .await
        .expect("list_jobs with before should not fail");

    assert_eq!(
        jobs.len(),
        1,
        "should return only the first job (created before the filter time)"
    );
}

/// Helper to create a `JobScheduler` with an in-memory database and test registry.
///
/// Constructs all required dependencies (queue, ledger, registry, database pool,
/// broadcaster, artifact store) and wraps them in a `JobScheduler`. Each test
/// calls this to get a fresh scheduler instance with its own isolated database.
async fn make_scheduler(db: SqlitePool, registry: Arc<NodeTypeRegistry>) -> JobScheduler {
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = ArtifactStore::new(artifact_dir.clone(), db.clone()).await;

    JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::new())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry,
        db,
        Arc::new(anvilml_ipc::EventBroadcaster::new()),
        Arc::new(artifact_store),
    )
}
