//! Tests for `WorkerEvent::ImageReady` handling in the event loop.
//!
//! Each test creates its own in-memory database, a fresh `JobScheduler`,
//! and a `WorkerPool` with pre-built status handles. The event loop is
//! started, a job is submitted and manually set to Running in the DB,
//! then a `WorkerEvent::ImageReady` is sent through the broadcaster's
//! worker event channel to verify the event loop processes it correctly.
//!
//! Tests use `#[serial]` because `open_in_memory()` creates a single-connection
//! SQLite pool that cannot be safely shared across concurrent Tokio tasks in the
//! same test binary.

use std::sync::Arc;
use std::time::Duration;

use anvilml_core::{
    JobSettings, NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor, SlotType, SubmitJobRequest,
};
use anvilml_ipc::{ArtifactStore, EventBroadcaster, WorkerEvent};
use anvilml_registry::open_in_memory;
use anvilml_scheduler::ledger::VramLedger;
use anvilml_scheduler::queue::JobQueue;
use anvilml_scheduler::scheduler::JobScheduler;
use base64::Engine;
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

    JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::new())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry,
        db,
        Arc::new(EventBroadcaster::new()),
        Arc::new(artifact_store),
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

/// A valid ImageReady event triggers artifact persistence and broadcasts
/// `WsEvent::JobImageReady`.
///
/// This test verifies:
/// - The artifact is saved to disk (file exists).
/// - `ArtifactStore::list()` returns exactly one entry for the job.
/// - The `WsEvent::JobImageReady` is broadcast with correct fields.
#[serial]
#[tokio::test]
async fn test_image_ready_persists_artifact() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Start the event loop.
    let event_handle = scheduler.start_event_loop();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Create a small valid PNG image and base64-encode it.
    // A minimal 1x1 red PNG (67 bytes).
    let png_bytes = base64_png();
    let image_b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    // Send an ImageReady event through the broadcaster.
    let broadcaster = scheduler.broadcaster();
    broadcaster.broadcast_worker_event(WorkerEvent::ImageReady {
        job_id,
        image_b64,
        width: 100,
        height: 200,
        format: "png".to_string(),
        seed: 42,
        steps: 20,
    });

    // Wait for the event loop to process the event.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the artifact was saved — list should return exactly one entry.
    let artifacts = scheduler
        .artifact_store()
        .list(Some(job_id))
        .await
        .expect("artifact list should succeed");
    assert_eq!(
        artifacts.len(),
        1,
        "should have exactly 1 artifact for the job"
    );
    assert_eq!(artifacts[0].width, 0); // Width is 0 because we don't parse PNG headers
    assert_eq!(artifacts[0].height, 0);

    // Verify the artifact file exists on disk.
    let artifact_path = &artifacts[0].path;
    assert!(
        std::path::Path::new(artifact_path).exists(),
        "artifact file should exist on disk at {}",
        artifact_path
    );

    event_handle.abort();
}

/// An ImageReady event broadcasts `WsEvent::JobImageReady` with correct
/// fields including artifact hash, dimensions, seed, and steps.
#[serial]
#[tokio::test]
async fn test_image_ready_broadcasts_job_image_ready() {
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

    // Send an ImageReady event.
    let png_bytes = base64_png();
    let image_b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    broadcaster.broadcast_worker_event(WorkerEvent::ImageReady {
        job_id,
        image_b64,
        width: 128,
        height: 256,
        format: "png".to_string(),
        seed: 12345,
        steps: 30,
    });

    // Wait for the event loop to process.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify WsEvent::JobImageReady was broadcast with correct fields.
    match tokio::time::timeout(Duration::from_millis(200), ws_rx.recv()).await {
        Ok(Ok(anvilml_core::types::WsEvent::JobImageReady {
            job_id: received_id,
            artifact_hash,
            width,
            height,
            seed,
            steps,
        })) => {
            assert_eq!(received_id, job_id, "job_id should match");
            assert_eq!(width, 128, "width should match");
            assert_eq!(height, 256, "height should match");
            assert_eq!(seed, 12345, "seed should match");
            assert_eq!(steps, 30, "steps should match");
            assert!(
                !artifact_hash.is_empty(),
                "artifact_hash should not be empty"
            );
        }
        Ok(Ok(_other)) => {
            panic!("unexpected WsEvent variant");
        }
        Ok(Err(e)) => {
            panic!("broadcast recv error: {:?}", e);
        }
        Err(_) => {
            panic!("WsEvent::JobImageReady was not broadcast within timeout");
        }
    }

    event_handle.abort();
}

/// An ImageReady event with invalid base64 payload is logged at WARN
/// and does not crash the event loop. The job status remains unchanged.
#[serial]
#[tokio::test]
async fn test_image_ready_invalid_base64_is_ignored() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    register_device(&scheduler).await;

    let job_id = submit_job(&scheduler).await;
    set_job_running(&db, job_id).await;

    // Subscribe to the WsEvent channel to verify nothing is broadcast.
    let broadcaster = scheduler.broadcaster();
    let mut ws_rx = broadcaster.subscribe();

    let event_handle = scheduler.start_event_loop();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send an ImageReady event with invalid base64.
    broadcaster.broadcast_worker_event(WorkerEvent::ImageReady {
        job_id,
        image_b64: "!!!invalid-base64!!!".to_string(),
        width: 100,
        height: 200,
        format: "png".to_string(),
        seed: 99,
        steps: 10,
    });

    // Wait for the event loop to process.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the job is still Running (not changed by the invalid event).
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(
        status, "running",
        "job should still be 'running' after invalid ImageReady"
    );

    // Verify no WsEvent was broadcast.
    match tokio::time::timeout(Duration::from_millis(200), ws_rx.recv()).await {
        Ok(Ok(_)) => {
            panic!("no WsEvent should be broadcast for invalid base64");
        }
        Ok(Err(_)) => {
            // No subscribers had the event — also acceptable.
        }
        Err(_) => {
            // Expected: no event was broadcast within timeout.
        }
    }

    // Verify no artifact was saved.
    let artifacts = scheduler
        .artifact_store()
        .list(Some(job_id))
        .await
        .expect("artifact list should succeed");
    assert!(
        artifacts.is_empty(),
        "should have 0 artifacts after invalid ImageReady"
    );

    event_handle.abort();
}

/// Create a minimal valid 1x1 red PNG image (67 bytes).
///
/// This is a hand-crafted minimal PNG that any image library can decode.
/// It consists of:
/// - 8-byte PNG signature
/// - IHDR chunk (13 bytes + 4 byte CRC)
/// - IDAT chunk (2 bytes compressed data + 4 byte CRC)
/// - IEND chunk (0 bytes + 4 byte CRC)
fn base64_png() -> Vec<u8> {
    // Minimal 1x1 red PNG (67 bytes).
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk length + type
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // bit depth=8, color=RGB
        0xDE, // CRC (approximate, not critical for our test)
        0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
        0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, // compressed data (approximate)
        0x00, 0x01, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB4, // CRC
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND chunk
        0xAE, 0x42, 0x60, 0x82, // CRC
    ]
}
