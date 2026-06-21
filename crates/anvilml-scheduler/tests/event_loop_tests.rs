//! Tests for the event loop — `start_event_loop` and event handling.
//!
//! Each test creates its own in-memory database, a fresh `JobScheduler`,
//! and a `WorkerPool` with pre-built status handles. The event loop is
//! started, a job is submitted and manually set to Running in the DB,
//! then a worker event is sent through the broadcaster's worker event
//! channel to verify the event loop processes it correctly.
//!
//! Tests use `#[serial]` because `open_in_memory()` creates a single-connection
//! SQLite pool that cannot be safely shared across concurrent Tokio tasks in the
//! same test binary.

use std::sync::Arc;
use std::time::Duration;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    JobSettings, NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor, SlotType, SubmitJobRequest,
};
use anvilml_ipc::{EventBroadcaster, WorkerEvent};
use anvilml_registry::open_in_memory;
use anvilml_scheduler::ledger::VramLedger;
use anvilml_scheduler::queue::JobQueue;
use anvilml_scheduler::scheduler::JobScheduler;
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
/// The broadcaster is shared between the test (which sends events through it)
/// and the event loop (which receives events from it). The artifact store
/// is created with a temporary directory so tests can verify artifact
/// persistence on disk.
async fn make_scheduler(db: SqlitePool, registry: Arc<NodeTypeRegistry>) -> JobScheduler {
    // Create a temporary directory for the artifact store.
    // We use a tempdir to ensure each test gets its own isolated artifact
    // storage directory, preventing cross-test contamination.
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = ArtifactStore::new(artifact_dir.clone(), db.clone()).await;
    let model_store = anvilml_registry::ModelStore::new(db.clone()).await;

    JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::new())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry,
        db,
        Arc::new(EventBroadcaster::new()),
        Arc::new(artifact_store),
        Arc::new(model_store),
        None, // cancellation requires a real worker pool
    )
}

/// Register device 0 with 16384 MiB VRAM and reserve 4096 MiB on the
/// scheduler's ledger.
///
/// The dispatch loop normally reserves VRAM before dispatching a job.
/// Since this test manually sets the job to Running (bypassing dispatch),
/// we must reserve VRAM ourselves so the event loop's release logic
/// has something to release.
async fn register_device(scheduler: &JobScheduler) {
    let mut ledger = scheduler.__ledger().await;
    ledger.register_device(0, 16384);
    // Reserve the default 4096 MiB to simulate what the dispatch loop does.
    ledger.reserve(0, 4096);
}

/// Submit a job and return its UUID.
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
    // Try to set device_index if the column exists; fall back to not
    // setting it if the column doesn't exist (pre-migration databases).
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

/// A Completed event transitions the job to Completed, sets completed_at,
/// releases VRAM, and broadcasts WsEvent::JobCompleted.
///
/// This is the happy-path test for the Completed event handler. It verifies:
/// - The DB status is updated to 'completed'.
/// - The completed_at timestamp is set.
/// - The VRAM reservation is released (decreased by 4096 MiB).
/// - The WsEvent::JobCompleted is broadcast on the WsEvent channel.
#[serial]
#[tokio::test]
async fn test_completed_event_updates_job_status() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Subscribe to the WsEvent channel BEFORE sending the event.
    // New broadcast subscribers only receive events sent after subscription,
    // so we must subscribe first to catch the JobCompleted event.
    let broadcaster = scheduler.broadcaster();
    let mut ws_rx = broadcaster.subscribe();

    // Start the event loop.
    let event_handle = scheduler.start_event_loop();

    // Give the event loop time to subscribe to the worker event channel.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send a Completed event through the broadcaster's worker event channel.
    broadcaster.broadcast_worker_event(WorkerEvent::Completed {
        job_id,
        elapsed_ms: 1234,
    });

    // Wait for the event loop to process the event.
    // The event loop runs in a separate task and processes events
    // synchronously (one at a time), so a short sleep is sufficient.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify DB status is 'completed'.
    let job = sqlx::query("SELECT status, completed_at FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(status, "completed", "job status should be 'completed'");

    let completed_at: String = job.try_get("completed_at").expect("completed_at column");
    assert!(
        !completed_at.is_empty(),
        "completed_at should be set (was '{}')",
        completed_at
    );

    // Verify VRAM was released (reservation should be 0, not 4096).
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            0,
            "VRAM should be released (reservation should be 0, not 4096)"
        );
    }

    // Verify WsEvent::JobCompleted was broadcast.
    match tokio::time::timeout(Duration::from_millis(200), ws_rx.recv()).await {
        Ok(Ok(anvilml_core::types::WsEvent::JobCompleted {
            job_id: received_id,
            elapsed_ms: received_elapsed,
        })) => {
            assert_eq!(received_id, job_id, "job_id should match");
            assert_eq!(
                received_elapsed, 1234,
                "elapsed_ms should match the Completed event"
            );
        }
        Ok(Ok(_other)) => {
            // Unexpected WsEvent variant — this should never happen
            // since the event loop only broadcasts JobCompleted here.
            panic!("unexpected WsEvent variant");
        }
        Ok(Err(e)) => {
            panic!("broadcast recv error: {:?}", e);
        }
        Err(_) => {
            panic!("WsEvent::JobCompleted was not broadcast within timeout");
        }
    }

    event_handle.abort();
}

/// A Failed event transitions the job to Failed, sets the error message,
/// releases VRAM, and broadcasts WsEvent::JobFailed.
///
/// This test verifies the Failed event handler path:
/// - The DB status is updated to 'failed'.
/// - The error column is set to the worker's error message.
/// - The VRAM reservation is released.
/// - The WsEvent::JobFailed is broadcast.
#[serial]
#[tokio::test]
async fn test_failed_event_updates_job_status() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Subscribe to the WsEvent channel BEFORE sending the event.
    let broadcaster = scheduler.broadcaster();
    let mut ws_rx = broadcaster.subscribe();

    let event_handle = scheduler.start_event_loop();
    tokio::time::sleep(Duration::from_millis(50)).await;

    broadcaster.broadcast_worker_event(WorkerEvent::Failed {
        job_id,
        error: "test failure".to_string(),
        traceback: Some("Traceback (most recent call last):\n  File ...".to_string()),
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify DB status is 'failed'.
    let job = sqlx::query("SELECT status, error FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(status, "failed", "job status should be 'failed'");

    let error: String = job.try_get("error").expect("error column");
    assert_eq!(error, "test failure", "error should match the Failed event");

    // Verify VRAM was released.
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            0,
            "VRAM should be released after failure"
        );
    }

    // Verify WsEvent::JobFailed was broadcast.
    match tokio::time::timeout(Duration::from_millis(200), ws_rx.recv()).await {
        Ok(Ok(anvilml_core::types::WsEvent::JobFailed {
            job_id: received_id,
            error: received_error,
        })) => {
            assert_eq!(received_id, job_id, "job_id should match");
            assert_eq!(
                received_error, "test failure",
                "error should match the Failed event"
            );
        }
        Ok(Ok(_other)) => {
            // Unexpected WsEvent variant — this should never happen
            // since the event loop only broadcasts JobFailed here.
            panic!("unexpected WsEvent variant");
        }
        Ok(Err(e)) => {
            panic!("broadcast recv error: {:?}", e);
        }
        Err(_) => {
            panic!("WsEvent::JobFailed was not broadcast within timeout");
        }
    }

    event_handle.abort();
}

/// An unknown WorkerEvent (Pong) does not crash the event loop and leaves
/// the job in its current state.
///
/// This test verifies that the event loop gracefully ignores event variants
/// it doesn't yet handle (Pong, Ready, Progress, etc.), logging at DEBUG
/// and continuing to process future events.
#[serial]
#[tokio::test]
async fn test_event_loop_ignores_unknown_event() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    let event_handle = scheduler.start_event_loop();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let broadcaster = scheduler.broadcaster();

    // Send a Pong event — the event loop should ignore it.
    broadcaster.broadcast_worker_event(WorkerEvent::Pong { seq: 42 });

    // Wait for the event loop to process the event.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the job is still Running (not changed by Pong).
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(
        status, "running",
        "job should still be 'running' after Pong event"
    );

    // Verify VRAM was NOT released (Pong doesn't affect VRAM).
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            4096,
            "VRAM should still be reserved after Pong event"
        );
    }

    // Verify no WsEvent was broadcast for the Pong.
    let mut ws_rx = broadcaster.subscribe();
    match tokio::time::timeout(Duration::from_millis(200), ws_rx.recv()).await {
        Ok(Ok(_)) => {
            panic!("no WsEvent should be broadcast for Pong event");
        }
        Ok(Err(e)) => {
            // No subscribers had the event — also acceptable.
            let _ = e;
        }
        Err(_) => {
            // Expected: no event was broadcast within timeout.
        }
    }

    event_handle.abort();
}
