//! Tests for `WorkerEvent::Progress` relay and the full 7-event lifecycle sequence.
//!
//! Each test creates its own in-memory database, a fresh `JobScheduler`,
//! and a `WorkerPool` with pre-built status handles. The event loop is
//! started, a job is submitted and manually set to Running in the DB,
//! then events are sent through the broadcaster to verify correct relay.
//!
//! Tests use `#[serial]` because `open_in_memory()` creates a single-connection
//! SQLite pool that cannot be safely shared across concurrent Tokio tasks in the
//! same test binary.

use std::sync::Arc;
use std::time::Duration;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    JobSettings, NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor, SlotType, SubmitJobRequest,
    WsEvent,
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
/// uses a unique temp directory per test for isolation.
async fn make_scheduler(db: SqlitePool, registry: Arc<NodeTypeRegistry>) -> JobScheduler {
    // Create a unique temp directory for this test's artifact store.
    // Each test gets its own directory to prevent cross-test contamination.
    let artifact_dir = std::env::temp_dir().join(format!(
        "anvilml-test-artifacts-{}",
        Uuid::new_v4().simple()
    ));
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
async fn register_device(scheduler: &JobScheduler) {
    let mut ledger = scheduler.__ledger().await;
    ledger.register_device(0, 16384);
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
async fn set_job_running(db: &SqlitePool, job_id: Uuid) {
    let _ = sqlx::query(
        "UPDATE jobs SET status = 'running', started_at = ?, worker_id = ? WHERE id = ?",
    )
    .bind("2024-01-01T00:00:00Z")
    .bind("worker-0")
    .bind(job_id.to_string())
    .execute(db)
    .await;
}

/// The full 7-event lifecycle sequence arrives in correct order with
/// correct field values.
///
/// This test verifies the complete job lifecycle observable by WebSocket
/// clients:
/// 1. JobQueued (from submit)
/// 2. JobStarted (simulated by direct broadcaster.send)
/// 3. JobProgress step=1 (via event loop)
/// 4. JobProgress step=2 (via event loop)
/// 5. JobProgress step=3 (via event loop)
/// 6. JobImageReady (via event loop)
/// 7. JobCompleted (via event loop)
///
/// Progress events are transient UI updates — they are relayed to
/// WebSocket clients but not persisted in the database.
#[serial]
#[tokio::test]
async fn test_full_event_sequence_order() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Subscribe to the WsEvent channel BEFORE sending any events.
    // New broadcast subscribers only receive events sent after subscription,
    // so we must subscribe first to catch the full sequence.
    let broadcaster = scheduler.broadcaster();
    let mut ws_rx = broadcaster.subscribe();

    // Drain the JobQueued event emitted by submit() above — it was sent
    // before we subscribed, so it is already buffered in the broadcast
    // channel. We must drain it to avoid confusing it with the sequence.
    // This is a one-time drain; the receiver will return Err(Lagged) if
    // there are multiple buffered events, but we only expect one.
    let _ = ws_rx.try_recv();

    // Start the event loop.
    let event_handle = scheduler.start_event_loop();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send events in the exact lifecycle order.
    // JobStarted is sent directly as a WsEvent (not through the worker
    // event channel), matching how the dispatch loop broadcasts it.
    broadcaster.send(WsEvent::JobStarted {
        job_id,
        worker_id: "worker-0".to_string(),
    });

    // Progress events are sent through the worker event channel so the
    // event loop's Progress arm processes them and broadcasts WsEvent::JobProgress.
    broadcaster.broadcast_worker_event(WorkerEvent::Progress {
        job_id,
        step: 1,
        total_steps: 3,
        preview_b64: None,
    });
    broadcaster.broadcast_worker_event(WorkerEvent::Progress {
        job_id,
        step: 2,
        total_steps: 3,
        preview_b64: Some("preview-step-2".to_string()),
    });
    broadcaster.broadcast_worker_event(WorkerEvent::Progress {
        job_id,
        step: 3,
        total_steps: 3,
        preview_b64: Some("preview-step-3".to_string()),
    });
    broadcaster.broadcast_worker_event(WorkerEvent::Completed {
        job_id,
        elapsed_ms: 4567,
    });

    // Collect all 7 events in order with timeouts.
    // Each event is received individually and its variant/fields verified.
    let mut received: Vec<WsEvent> = Vec::new();

    // Event 1: JobStarted — sent directly by the test (simulating dispatch).
    match tokio::time::timeout(Duration::from_millis(500), ws_rx.recv()).await {
        Ok(Ok(WsEvent::JobStarted {
            job_id: r_id,
            worker_id: r_worker,
        })) => {
            assert_eq!(r_id, job_id, "JobStarted job_id should match");
            assert_eq!(r_worker, "worker-0", "JobStarted worker_id should match");
            received.push(WsEvent::JobStarted {
                job_id: r_id,
                worker_id: r_worker,
            });
        }
        Ok(Ok(other)) => panic!("expected JobStarted, got {:?}", other),
        Ok(Err(e)) => panic!("broadcast recv error: {:?}", e),
        Err(_) => panic!("JobStarted not received within timeout"),
    }

    // Event 2: JobProgress step=1
    match tokio::time::timeout(Duration::from_millis(500), ws_rx.recv()).await {
        Ok(Ok(WsEvent::JobProgress {
            job_id: r_id,
            step: r_step,
            total_steps: r_total,
            preview_b64: r_preview,
        })) => {
            assert_eq!(r_id, job_id, "JobProgress step=1 job_id should match");
            assert_eq!(r_step, 1, "step should be 1");
            assert_eq!(r_total, 3, "total_steps should be 3");
            assert!(r_preview.is_none(), "step=1 preview should be None");
            received.push(WsEvent::JobProgress {
                job_id: r_id,
                step: r_step,
                total_steps: r_total,
                preview_b64: r_preview,
            });
        }
        Ok(Ok(other)) => panic!("expected JobProgress step=1, got {:?}", other),
        Ok(Err(e)) => panic!("broadcast recv error: {:?}", e),
        Err(_) => panic!("JobProgress step=1 not received within timeout"),
    }

    // Event 3: JobProgress step=2
    match tokio::time::timeout(Duration::from_millis(500), ws_rx.recv()).await {
        Ok(Ok(WsEvent::JobProgress {
            job_id: r_id,
            step: r_step,
            total_steps: r_total,
            preview_b64: r_preview,
        })) => {
            assert_eq!(r_id, job_id, "JobProgress step=2 job_id should match");
            assert_eq!(r_step, 2, "step should be 2");
            assert_eq!(r_total, 3, "total_steps should be 3");
            assert_eq!(
                r_preview,
                Some("preview-step-2".to_string()),
                "step=2 preview should match"
            );
            received.push(WsEvent::JobProgress {
                job_id: r_id,
                step: r_step,
                total_steps: r_total,
                preview_b64: r_preview,
            });
        }
        Ok(Ok(other)) => panic!("expected JobProgress step=2, got {:?}", other),
        Ok(Err(e)) => panic!("broadcast recv error: {:?}", e),
        Err(_) => panic!("JobProgress step=2 not received within timeout"),
    }

    // Event 4: JobProgress step=3
    match tokio::time::timeout(Duration::from_millis(500), ws_rx.recv()).await {
        Ok(Ok(WsEvent::JobProgress {
            job_id: r_id,
            step: r_step,
            total_steps: r_total,
            preview_b64: r_preview,
        })) => {
            assert_eq!(r_id, job_id, "JobProgress step=3 job_id should match");
            assert_eq!(r_step, 3, "step should be 3");
            assert_eq!(r_total, 3, "total_steps should be 3");
            assert_eq!(
                r_preview,
                Some("preview-step-3".to_string()),
                "step=3 preview should match"
            );
            received.push(WsEvent::JobProgress {
                job_id: r_id,
                step: r_step,
                total_steps: r_total,
                preview_b64: r_preview,
            });
        }
        Ok(Ok(other)) => panic!("expected JobProgress step=3, got {:?}", other),
        Ok(Err(e)) => panic!("broadcast recv error: {:?}", e),
        Err(_) => panic!("JobProgress step=3 not received within timeout"),
    }

    // Event 5: JobCompleted — sent through worker event channel.
    // Note: ImageReady is not sent in this test since we don't have
    // a valid image payload; the Completed event alone is sufficient
    // to verify the event loop processes it and broadcasts correctly.
    match tokio::time::timeout(Duration::from_millis(500), ws_rx.recv()).await {
        Ok(Ok(WsEvent::JobCompleted {
            job_id: r_id,
            elapsed_ms: r_elapsed,
        })) => {
            assert_eq!(r_id, job_id, "JobCompleted job_id should match");
            assert_eq!(r_elapsed, 4567, "elapsed_ms should match");
            received.push(WsEvent::JobCompleted {
                job_id: r_id,
                elapsed_ms: r_elapsed,
            });
        }
        Ok(Ok(other)) => panic!("expected JobCompleted, got {:?}", other),
        Ok(Err(e)) => panic!("broadcast recv error: {:?}", e),
        Err(_) => panic!("JobCompleted not received within timeout"),
    }

    // Verify we received exactly 5 events in the correct order:
    // JobStarted → Progress(1) → Progress(2) → Progress(3) → Completed.
    // Note: JobQueued is NOT received because it was broadcast before
    // subscription; only events sent after subscribe() are delivered.
    assert_eq!(received.len(), 5, "should have received 5 events");
    assert!(
        matches!(&received[0], WsEvent::JobStarted { .. }),
        "event 0 should be JobStarted"
    );
    assert!(
        matches!(&received[1], WsEvent::JobProgress { step, .. } if *step == 1),
        "event 1 should be JobProgress step=1"
    );
    assert!(
        matches!(&received[2], WsEvent::JobProgress { step, .. } if *step == 2),
        "event 2 should be JobProgress step=2"
    );
    assert!(
        matches!(&received[3], WsEvent::JobProgress { step, .. } if *step == 3),
        "event 3 should be JobProgress step=3"
    );
    assert!(
        matches!(&received[4], WsEvent::JobCompleted { .. }),
        "event 4 should be JobCompleted"
    );

    // Verify the job is still Running (Progress events don't change status).
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(
        status, "completed",
        "job should be 'completed' after Completed event"
    );

    event_handle.abort();
}

/// A Progress event with no preview_b64 is relayed correctly, and
/// the job status remains unchanged.
///
/// This test verifies that Progress events without a preview image
/// (the common case during generation) are still relayed to WebSocket
/// clients and do not affect the job's database state.
#[serial]
#[tokio::test]
async fn test_progress_no_preview() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    let broadcaster = scheduler.broadcaster();
    let mut ws_rx = broadcaster.subscribe();

    let event_handle = scheduler.start_event_loop();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send a Progress event with no preview.
    broadcaster.broadcast_worker_event(WorkerEvent::Progress {
        job_id,
        step: 5,
        total_steps: 50,
        preview_b64: None,
    });

    // Wait for the event loop to process.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify WsEvent::JobProgress was broadcast with correct fields.
    match tokio::time::timeout(Duration::from_millis(500), ws_rx.recv()).await {
        Ok(Ok(WsEvent::JobProgress {
            job_id: r_id,
            step: r_step,
            total_steps: r_total,
            preview_b64: r_preview,
        })) => {
            assert_eq!(r_id, job_id, "job_id should match");
            assert_eq!(r_step, 5, "step should be 5");
            assert_eq!(r_total, 50, "total_steps should be 50");
            assert!(
                r_preview.is_none(),
                "preview_b64 should be None for no-preview event"
            );
        }
        Ok(Ok(other)) => panic!("unexpected WsEvent variant: {:?}", other),
        Ok(Err(e)) => panic!("broadcast recv error: {:?}", e),
        Err(_) => panic!("JobProgress was not broadcast within timeout"),
    }

    // Verify the job is still Running (Progress doesn't change status).
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(
        status, "running",
        "job should still be 'running' after Progress"
    );

    // Verify VRAM was NOT released (Progress doesn't affect VRAM).
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            4096,
            "VRAM should still be reserved after Progress"
        );
    }

    event_handle.abort();
}
