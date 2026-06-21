//! Tests for the dispatch loop — `start_dispatch_loop`, `select_worker`,
//! and `dispatch_once`.
//!
//! Each test creates its own in-memory database, a fresh `JobScheduler`,
//! and a `WorkerPool` with pre-built status handles. The dispatch loop
//! is started, a job is submitted (triggering the `Notify`), and the test
//! verifies the dispatch outcome after a brief wait.
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
async fn make_scheduler(db: SqlitePool, registry: Arc<NodeTypeRegistry>) -> JobScheduler {
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

/// Create a `WorkerPool` with one idle worker for testing.
///
/// The pool is built with `WorkerPool::new()` using a pre-constructed
/// status handle set to `Idle`. The transport is a bound ROUTER socket
/// that has no connected workers — `send_execute` will fail gracefully.
async fn make_one_idle_pool() -> (WorkerPool, SqlitePool, Arc<NodeTypeRegistry>) {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;

    let transport = anvilml_ipc::RouterTransport::bind()
        .await
        .expect("bind transport");
    let pool = WorkerPool::new(
        vec![(
            Arc::new(tokio::sync::RwLock::new(WorkerStatus::Idle)),
            "worker-0".to_string(),
            "mock-device".to_string(),
        )],
        Arc::new(transport),
        Arc::new(EventBroadcaster::new()),
    );

    (pool, db, registry)
}

/// A job dispatch completes: job is removed from queue, VRAM reserved,
/// and DB updated to Running status.
///
/// This is the happy-path test for the dispatch loop. It verifies:
/// - The job is removed from the queue after dispatch.
/// - The VRAM ledger shows a reservation for the device.
/// - The database status transitions from Queued to Running.
#[serial]
#[tokio::test]
async fn test_dispatch_to_idle_worker() {
    let (pool, db, registry) = make_one_idle_pool().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    // Register the device in the scheduler's ledger.
    {
        let mut ledger_guard = scheduler.__ledger().await;
        ledger_guard.register_device(0, 16384);
    }

    let job_id = submit_job(&scheduler).await;

    // Start the dispatch loop.
    let dispatch_handle = scheduler.start_dispatch_loop(Arc::new(pool));

    // Wait for the dispatch loop to process the job.
    // The notify from submit() should wake the loop quickly.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let queue = scheduler.__queue().await;
            if queue.is_empty() {
                break;
            }
            drop(queue);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("dispatch should complete within timeout");

    // Verify the job was removed from the queue.
    let queue = scheduler.__queue().await;
    assert!(queue.is_empty(), "queue should be empty after dispatch");
    drop(queue);

    // Verify VRAM was reserved.
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            4096,
            "VRAM should be reserved on device 0"
        );
    }

    // Verify DB status is Running.
    let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist");
    let status: String = job.try_get("status").expect("status column");
    assert_eq!(status, "running", "job status should be 'running'");

    // Clean up the dispatch loop.
    dispatch_handle.abort();
}

/// VRAM ledger reserve() is called with the correct device index and amount.
///
/// Verifies that when a job is dispatched, the ledger's `reserve` method
/// is called with the device index 0 and the default 4096 MiB amount.
/// This is a focused test on the VRAM tracking aspect of dispatch.
#[serial]
#[tokio::test]
async fn test_vram_reserved_on_dispatch() {
    let (pool, db, registry) = make_one_idle_pool().await;
    let scheduler = make_scheduler(db, registry).await;

    // Register the device with 8192 MiB total VRAM.
    {
        let mut ledger_guard = scheduler.__ledger().await;
        ledger_guard.register_device(0, 8192);
    }

    submit_job(&scheduler).await;

    let dispatch_handle = scheduler.start_dispatch_loop(Arc::new(pool));

    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let queue = scheduler.__queue().await;
            if queue.is_empty() {
                break;
            }
            drop(queue);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("dispatch should complete within timeout");

    // Verify the reservation is exactly 4096 MiB.
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            4096,
            "VRAM reservation should be 4096 MiB"
        );
        // Total VRAM should still be 8192.
        assert_eq!(
            ledger_guard.total_vram(0),
            Some(8192),
            "total VRAM should be 8192 MiB"
        );
    }

    dispatch_handle.abort();
}

/// No job is dispatched when all workers are Busy.
///
/// Creates a pool where the only worker is in the `Busy` state, submits
/// a job, starts the dispatch loop, and verifies the job remains queued.
/// This tests the idle-worker check in the dispatch loop.
#[serial]
#[tokio::test]
async fn test_no_dispatch_when_no_idle_workers() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;

    // Create a pool with a Busy worker.
    let transport = anvilml_ipc::RouterTransport::bind()
        .await
        .expect("bind transport");
    let pool = WorkerPool::new(
        vec![(
            Arc::new(tokio::sync::RwLock::new(WorkerStatus::Busy)),
            "worker-0".to_string(),
            "mock-device".to_string(),
        )],
        Arc::new(transport),
        Arc::new(EventBroadcaster::new()),
    );

    let scheduler = make_scheduler(db, registry).await;

    // Register the device.
    {
        let mut ledger_guard = scheduler.__ledger().await;
        ledger_guard.register_device(0, 16384);
    }

    submit_job(&scheduler).await;

    let dispatch_handle = scheduler.start_dispatch_loop(Arc::new(pool));

    // Wait a bit longer since the dispatch loop needs to wake and check.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify the job is still in the queue.
    let queue = scheduler.__queue().await;
    assert!(
        !queue.is_empty(),
        "job should still be queued when no idle workers"
    );
    drop(queue);

    // Verify VRAM was NOT reserved (reservation should be 0, not 4096).
    {
        let ledger_guard = scheduler.__ledger().await;
        let reservations = ledger_guard.reservations();
        assert_eq!(
            *reservations.get(&0).unwrap_or(&0),
            0,
            "no VRAM should be reserved when no dispatch occurred"
        );
    }

    dispatch_handle.abort();
}

/// Dispatch loop wakes on Notify signal from submit().
///
/// Verifies that the dispatch loop processes a job promptly after
/// `submit()` triggers the `Notify`. The test measures that the job
/// is dispatched within a short timeout (2 seconds), proving the
/// notify mechanism works correctly.
#[serial]
#[tokio::test]
async fn test_dispatch_wakes_on_notify() {
    let (pool, db, registry) = make_one_idle_pool().await;
    let scheduler = make_scheduler(db.clone(), registry).await;

    {
        let mut ledger_guard = scheduler.__ledger().await;
        ledger_guard.register_device(0, 16384);
    }

    // Start the dispatch loop before submitting the job.
    let dispatch_handle = scheduler.start_dispatch_loop(Arc::new(pool));

    // Submit a job — this triggers the Notify.
    let job_id = submit_job(&scheduler).await;

    // The dispatch loop should process the job quickly (within 2 seconds).
    // Use a timeout to make the test deterministic.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let queue = scheduler.__queue().await;
            let empty = queue.is_empty();
            drop(queue);
            if empty {
                // Verify the job was actually dispatched (not just removed).
                let job = sqlx::query("SELECT status FROM jobs WHERE id = ?")
                    .bind(job_id.to_string())
                    .fetch_optional(&db)
                    .await
                    .expect("query should succeed");
                if let Some(row) = job {
                    let status: String = row.try_get("status").expect("status");
                    assert_eq!(status, "running", "job should be running");
                }
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("dispatch should wake on notify within timeout");

    dispatch_handle.abort();
}

/// Device preference is respected: a job requesting device 1 is dispatched
/// to the worker on device 1, even if the worker on device 0 has more VRAM.
///
/// Creates two idle workers on different devices. The job specifies
/// `device_preference = "cuda:1"`. Verifies that the dispatch loop selects
/// the worker on device 1.
#[serial]
#[tokio::test]
async fn test_device_preference_respected() {
    let db = open_in_memory().await.expect("open in-memory DB");
    let registry = make_registry().await;

    let transport = anvilml_ipc::RouterTransport::bind()
        .await
        .expect("bind transport");
    let pool = WorkerPool::new(
        vec![
            (
                Arc::new(tokio::sync::RwLock::new(WorkerStatus::Idle)),
                "worker-0".to_string(),
                "device-0".to_string(),
            ),
            (
                Arc::new(tokio::sync::RwLock::new(WorkerStatus::Idle)),
                "worker-1".to_string(),
                "device-1".to_string(),
            ),
        ],
        Arc::new(transport),
        Arc::new(EventBroadcaster::new()),
    );

    let scheduler = make_scheduler(db.clone(), registry).await;

    // Register both devices.
    {
        let mut ledger_guard = scheduler.__ledger().await;
        ledger_guard.register_device(0, 8192); // device 0 has more VRAM
        ledger_guard.register_device(1, 4096); // device 1 has less VRAM
    }

    // Submit a job with device preference for device 1.
    let resp = scheduler
        .submit(SubmitJobRequest {
            graph: make_valid_graph(),
            settings: JobSettings {
                device_preference: Some("cuda:1".to_string()),
            },
        })
        .await
        .expect("submit should succeed");
    let job_id = resp.job_id;

    let dispatch_handle = scheduler.start_dispatch_loop(Arc::new(pool));

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let queue = scheduler.__queue().await;
            if queue.is_empty() {
                break;
            }
            drop(queue);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("dispatch should complete within timeout");

    // Verify VRAM was reserved on device 1 (not device 0).
    {
        let ledger_guard = scheduler.__ledger().await;
        assert_eq!(
            *ledger_guard.reservations().get(&1).unwrap_or(&0),
            4096,
            "VRAM should be reserved on device 1 (preferred)"
        );
        assert_eq!(
            *ledger_guard.reservations().get(&0).unwrap_or(&0),
            0,
            "VRAM should NOT be reserved on device 0"
        );
    }

    // Verify the DB was updated with the correct worker.
    let worker_id: String = sqlx::query("SELECT worker_id FROM jobs WHERE id = ?")
        .bind(job_id.to_string())
        .fetch_one(&db)
        .await
        .expect("job row should exist")
        .try_get("worker_id")
        .expect("worker_id column");
    assert_eq!(
        worker_id, "worker-1",
        "job should be assigned to worker-1 (device 1)"
    );

    dispatch_handle.abort();
}

/// Helper to submit a valid graph and return the job ID.
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
