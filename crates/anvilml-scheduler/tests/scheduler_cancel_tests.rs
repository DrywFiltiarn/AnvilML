//! Tests for job cancellation — `cancel_job` and `handle_cancelled`.
//!
//! Each test creates its own in-memory database, a fresh `JobScheduler`,
//! and a `WorkerPool` with pre-built status handles. The event loop is
//! started for tests that verify event loop behavior.
//!
//! Tests use `#[serial]` because `open_in_memory()` creates a single-connection
//! SQLite pool that cannot be safely shared across concurrent Tokio tasks in the
//! same test binary.

use std::sync::Arc;
use std::time::Duration;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    JobSettings, NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor, SlotType, SubmitJobRequest,
    WorkerStatus,
};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::open_in_memory;
use anvilml_scheduler::ledger::VramLedger;
use anvilml_scheduler::queue::JobQueue;
use anvilml_scheduler::scheduler::JobScheduler;
use anvilml_worker::pool::WorkerPool;
use serial_test::serial;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Populate a registry with the `LoadModel` node type.
///
/// This is the minimum registry needed for graph validation to pass.
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
fn make_valid_graph() -> serde_json::Value {
    serde_json::json!({
        "nodes": [
            { "id": "model", "type": "LoadModel" }
        ]
    })
}

/// Create a scheduler with an in-memory database and test registry.
///
/// The workers parameter is `None` for tests that don't need cancellation
/// support. Tests that need a worker pool pass `Some(Arc::new(pool))`.
async fn make_scheduler(
    db: SqlitePool,
    registry: Arc<NodeTypeRegistry>,
    workers: Option<Arc<WorkerPool>>,
) -> JobScheduler {
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = ArtifactStore::new(artifact_dir.clone(), db.clone()).await;

    JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::new())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry,
        db,
        Arc::new(EventBroadcaster::new()),
        Arc::new(artifact_store),
        workers,
    )
}

/// Create a `WorkerPool` with one idle worker for testing.
///
/// The pool is built with `WorkerPool::new()` using a pre-constructed
/// status handle. The transport is a bound ROUTER socket with no
/// connected workers.
async fn make_pool(status: WorkerStatus) -> WorkerPool {
    let transport = anvilml_ipc::RouterTransport::bind()
        .await
        .expect("bind transport");
    WorkerPool::new(
        vec![(
            Arc::new(tokio::sync::RwLock::new(status)),
            "worker-0".to_string(),
            "mock-device".to_string(),
        )],
        Arc::new(transport),
        Arc::new(EventBroadcaster::new()),
    )
}

/// Register device 0 with 16384 MiB VRAM and reserve 4096 MiB on the
/// scheduler's ledger.
///
/// The dispatch loop normally reserves VRAM before dispatching a job.
/// Since this test manually sets the job state, we must reserve VRAM
/// ourselves so the event loop's release logic has something to release.
async fn register_device(scheduler: &JobScheduler) {
    let mut ledger = scheduler.__ledger().await;
    ledger.register_device(0, 16384);
    ledger.reserve(0, 4096);
}

/// Submit a valid graph and return the job ID.
///
/// This reduces boilerplate in tests that only need to trigger a
/// job submission and get back the UUID.
async fn submit_job(scheduler: &JobScheduler) -> Uuid {
    let resp = scheduler
        .submit(SubmitJobRequest {
            graph: make_valid_graph(),
            settings: JobSettings::default(),
        })
        .await
        .expect("submit should succeed");
    resp.job_id
}

/// Manually set a job's status to Running in the DB.
///
/// This simulates what the dispatch loop does when it assigns a worker
/// to a job. The event loop derives the device index from the worker_id
/// ("worker-0" → 0) for VRAM release, so setting device_index is
/// optional — it only matters for databases that have migration 002.
async fn set_job_running(db: &SqlitePool, job_id: Uuid) {
    let _ = sqlx::query(
        "UPDATE jobs SET status = 'running', started_at = ?, worker_id = ?, device_index = ? WHERE id = ?",
    )
    .bind("2024-01-01T00:00:00Z")
    .bind("worker-0")
    .bind(0i64)
    .bind(job_id.to_string())
    .execute(db)
    .await;

    // Fallback: update without device_index (for in-memory / pre-migration DBs).
    let _ = sqlx::query(
        "UPDATE jobs SET status = 'running', started_at = ?, worker_id = ? WHERE id = ?",
    )
    .bind("2024-01-01T00:00:00Z")
    .bind("worker-0")
    .bind(job_id.to_string())
    .execute(db)
    .await;
}

/// Cancel a Queued job: removes from queue, updates DB to cancelled,
/// broadcasts WsEvent::JobCancelled.
///
/// This is the happy-path test for cancelling a job that hasn't been
/// dispatched yet. It verifies:
/// - The job is removed from the in-memory queue.
/// - The DB status is updated to 'cancelled'.
/// - The completed_at timestamp is set.
/// - The WsEvent::JobCancelled is broadcast.
#[serial]
#[tokio::test]
async fn test_cancel_queued_job() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry, None).await;

    let job_id = submit_job(&scheduler).await;

    // Verify the job is Queued in the DB.
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(status, "queued", "job should be queued initially");

    // Subscribe to the WsEvent channel BEFORE cancelling.
    let broadcaster = scheduler.broadcaster();
    let mut ws_rx = broadcaster.subscribe();

    // Cancel the queued job.
    scheduler
        .cancel_job(job_id)
        .await
        .expect("cancel should succeed");

    // Verify the job is removed from the queue.
    let queue = scheduler.__queue().await;
    assert!(queue.is_empty(), "queue should be empty after cancel");
    drop(queue);

    // Verify DB status is 'cancelled'.
    let job = sqlx::query("SELECT status, completed_at FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(status, "cancelled", "job status should be 'cancelled'");

    let completed_at: String = job.try_get("completed_at").expect("completed_at column");
    assert!(
        !completed_at.is_empty(),
        "completed_at should be set (was '{}')",
        completed_at
    );

    // Verify WsEvent::JobCancelled was broadcast.
    match tokio::time::timeout(Duration::from_millis(200), ws_rx.recv()).await {
        Ok(Ok(anvilml_core::types::WsEvent::JobCancelled {
            job_id: received_id,
        })) => {
            assert_eq!(received_id, job_id, "job_id should match");
        }
        Ok(Ok(_other)) => {
            panic!("unexpected WsEvent variant");
        }
        Ok(Err(e)) => {
            panic!("broadcast recv error: {:?}", e);
        }
        Err(_) => {
            panic!("WsEvent::JobCancelled was not broadcast within timeout");
        }
    }
}

/// Cancel a Running job: sends CancelJob IPC to owning worker,
/// returns Ok(()) immediately, DB status remains running.
///
/// This test verifies the running-job cancellation path. The job is
/// manually set to Running in the DB (simulating dispatch). The worker
/// pool has a Busy worker (mimicking an active worker). Since the
/// worker pool's transport has no connected workers, `send_cancel`
/// will fail with an IPC error, which is expected — the test verifies
/// that the error is propagated back to the caller.
#[serial]
#[tokio::test]
async fn test_cancel_running_job_fails_without_worker() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;

    // Create a pool with a Busy worker to simulate an active worker.
    let pool = make_pool(WorkerStatus::Busy).await;

    let scheduler = make_scheduler(db.clone(), registry, Some(Arc::new(pool))).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Cancel the running job. Since there's no connected worker,
    // send_cancel will fail with an IPC error.
    let result = scheduler.cancel_job(job_id).await;
    assert!(
        result.is_err(),
        "cancel should fail when worker is not connected"
    );

    // Verify the error is an IPC error (not InvalidOperation or JobNotFound).
    let err = result.unwrap_err();
    let error_kind = err.error_kind();
    assert_eq!(
        error_kind, "ipc",
        "error kind should be 'ipc', got '{}'",
        error_kind
    );

    // Verify DB status is still 'running' — the cancel was not
    // confirmed because the IPC send failed.
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(
        status, "running",
        "job should still be running (cancel not confirmed)"
    );
}

/// Cancel a Completed job: returns AnvilError::InvalidOperation (409).
///
/// This test verifies that cancelling a terminal-state job returns
/// the correct error. The job is manually set to Completed in the DB,
/// then we attempt to cancel it.
#[serial]
#[tokio::test]
async fn test_cancel_terminal_job_returns_error() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry, None).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Manually set the job to Completed.
    let _ = sqlx::query("UPDATE jobs SET status = 'completed', completed_at = ? WHERE id = ?")
        .bind("2024-01-01T01:00:00Z")
        .bind(job_id.to_string())
        .execute(&db)
        .await
        .expect("update should succeed");

    // Attempt to cancel — should return InvalidOperation.
    let result = scheduler.cancel_job(job_id).await;
    assert!(result.is_err(), "cancel should fail for completed job");

    let err = result.unwrap_err();
    let status_code = err.status_code();
    assert_eq!(
        status_code,
        axum::http::StatusCode::CONFLICT,
        "status code should be 409 Conflict, got {}",
        status_code
    );

    let error_kind = err.error_kind();
    assert_eq!(
        error_kind, "invalid_operation",
        "error kind should be 'invalid_operation', got '{}'",
        error_kind
    );

    // Verify DB status is still 'completed' — the cancel was rejected.
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(
        status, "completed",
        "job should still be completed (cancel rejected)"
    );
}

/// Cancel a non-existent job: returns AnvilError::JobNotFound (404).
///
/// This test verifies that cancelling a job that doesn't exist in
/// the database returns the correct 404 error.
#[serial]
#[tokio::test]
async fn test_cancel_unknown_job_returns_404() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry, None).await;

    // Use a random UUID that doesn't exist in the database.
    let unknown_id = Uuid::new_v4();

    let result = scheduler.cancel_job(unknown_id).await;
    assert!(result.is_err(), "cancel should fail for unknown job");

    let err = result.unwrap_err();
    let status_code = err.status_code();
    assert_eq!(
        status_code,
        axum::http::StatusCode::NOT_FOUND,
        "status code should be 404 Not Found, got {}",
        status_code
    );

    let error_kind = err.error_kind();
    assert_eq!(
        error_kind, "job_not_found",
        "error kind should be 'job_not_found', got '{}'",
        error_kind
    );
}

/// WorkerEvent::Cancelled handler: sets status=cancelled, releases VRAM,
/// broadcasts WsEvent::JobCancelled.
///
/// This test verifies the event loop's Cancelled event handler. A job
/// is manually set to Running with VRAM reserved, then a Cancelled event
/// is sent through the broadcaster. The test verifies that the event
/// loop processes it correctly.
#[serial]
#[tokio::test]
async fn test_cancelled_event_releases_vram() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry, None).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Subscribe to the WsEvent channel BEFORE sending the event.
    let broadcaster = scheduler.broadcaster();
    let mut ws_rx = broadcaster.subscribe();

    // Start the event loop.
    let event_handle = scheduler.start_event_loop();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send a Cancelled event through the broadcaster's worker event channel.
    broadcaster.broadcast_worker_event(anvilml_ipc::WorkerEvent::Cancelled { job_id });

    // Wait for the event loop to process the event.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify DB status is 'cancelled'.
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(status, "cancelled", "job status should be 'cancelled'");

    // Verify VRAM was released (reservation should be 0, not 4096).
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            0,
            "VRAM should be released after cancellation"
        );
    }

    // Verify WsEvent::JobCancelled was broadcast.
    match tokio::time::timeout(Duration::from_millis(200), ws_rx.recv()).await {
        Ok(Ok(anvilml_core::types::WsEvent::JobCancelled {
            job_id: received_id,
        })) => {
            assert_eq!(received_id, job_id, "job_id should match");
        }
        Ok(Ok(_other)) => {
            panic!("unexpected WsEvent variant");
        }
        Ok(Err(e)) => {
            panic!("broadcast recv error: {:?}", e);
        }
        Err(_) => {
            panic!("WsEvent::JobCancelled was not broadcast within timeout");
        }
    }

    event_handle.abort();
}
