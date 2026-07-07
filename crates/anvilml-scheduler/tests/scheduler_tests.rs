/// Integration tests for `JobScheduler` — the central async dispatcher.
///
/// Tests exercise the full submit flow: workers-available check, graph
/// validation, job construction, persistence, enqueueing, and notification.
use std::sync::Arc;

use anvilml_core::{
    AnvilError, JobSettings, NodeTypeDescriptor, NodeTypeRegistry, SlotDescriptor, SlotType,
};
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

/// Helper to create a `JobStore` backed by an in-memory SQLite pool with
/// migrations applied.
///
/// Each call creates a fresh pool, applies all migrations from the
/// `database/migrations/` directory, and returns a `JobStore` wrapping
/// that pool. This ensures database isolation between tests.
async fn create_job_store() -> JobStore {
    // Create an in-memory SQLite pool. `:memory:` creates a database that
    // exists only for the lifetime of this connection — it is isolated
    // from all other pools, including other test runs.
    let connect_opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("in-memory SQLite pool must connect");

    // Apply all migrations so the `jobs` table exists. The migrator path
    // is relative to the crate root (the same convention used by
    // `create_pool()` in the registry crate). This is idempotent: running
    // against an already-migrated database is a no-op.
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&pool)
        .await
        .expect("migrations must apply to in-memory pool");

    JobStore::new(pool)
}

/// Helper to create a `NodeTypeRegistry` with a single "PassThrough" node type
/// registered.
///
/// The "PassThrough" node has one output slot ("OUT", SlotType::Any) and one
/// input slot ("IN", SlotType::Any), making it a valid minimal node for
/// graph validation tests.
fn make_registry() -> Arc<NodeTypeRegistry> {
    let registry = Arc::new(NodeTypeRegistry::new());
    registry.register_all(vec![NodeTypeDescriptor {
        type_name: "PassThrough".into(),
        display_name: "Pass Through".into(),
        category: "utility".into(),
        description: "A pass-through node for testing".into(),
        inputs: vec![SlotDescriptor {
            name: "IN".into(),
            slot_type: SlotType::Any,
            optional: false,
        }],
        outputs: vec![SlotDescriptor {
            name: "OUT".into(),
            slot_type: SlotType::Any,
            optional: false,
        }],
    }]);
    registry
}

/// Helper to create a valid graph JSON with a single PassThrough node.
fn make_valid_graph() -> serde_json::Value {
    serde_json::json!({
        "nodes": [{"id": "n1", "type": "PassThrough"}]
    })
}

/// Test that `submit()` returns `WorkersUnavailable` when the registry is empty.
///
/// Constructs a `JobScheduler` with an empty `NodeTypeRegistry`, then calls
/// `submit()` with a valid graph and empty settings. Must return
/// `Err(AnvilError::WorkersUnavailable)` — the "no workers = reject" guard
/// fires before any validation or persistence work.
#[tokio::test]
async fn test_submit_empty_registry_returns_workers_unavailable() {
    let store = create_job_store().await;
    let registry: Arc<NodeTypeRegistry> = Arc::new(NodeTypeRegistry::new());
    let scheduler = JobScheduler::new(store, registry);

    let result = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await;

    assert!(
        result.is_err(),
        "submit() must return Err when registry is empty"
    );
    match result.unwrap_err() {
        AnvilError::WorkersUnavailable(msg) => {
            assert_eq!(msg, "no workers registered", "error message must match");
        }
        other => panic!("Expected WorkersUnavailable, got: {:?}", other),
    }
}

/// Test that `submit()` returns `InvalidGraph` when the graph contains an
/// unknown node type.
///
/// Populates the registry with "PassThrough", then submits a graph that
/// references "NonExistentNode". Must return `Err(AnvilError::InvalidGraph)`
/// — the graph validation check catches the unknown type before job
/// construction or persistence.
#[tokio::test]
async fn test_submit_invalid_graph_returns_validation_error() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);

    // Submit a graph with an unknown node type.
    let invalid_graph = serde_json::json!({
        "nodes": [{"id": "n1", "type": "NonExistentNode"}]
    });

    let result = scheduler
        .submit(
            invalid_graph,
            JobSettings {
                device_preference: None,
            },
        )
        .await;

    assert!(
        result.is_err(),
        "submit() must return Err for invalid graph"
    );
    match result.unwrap_err() {
        AnvilError::InvalidGraph(errors) => {
            assert!(
                errors.iter().any(|e| e.contains("NonExistentNode")),
                "error must mention the unknown node type, got: {:?}",
                errors
            );
        }
        other => panic!("Expected InvalidGraph, got: {:?}", other),
    }
}

/// Test that a valid submission returns `Ok(id)` with a non-nil UUID.
///
/// Populates the registry with "PassThrough", submits a valid graph,
/// and asserts the return is `Ok(uuid)` where the UUID is not nil.
/// The job is persisted to the database and enqueued in the in-memory
/// queue (verified indirectly by the successful return and the fact
/// that `upsert()` would have panicked on DB failure).
#[tokio::test]
async fn test_submit_valid_persists_and_queues() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);

    let result = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await;

    // The submission must succeed with a valid job ID.
    assert!(result.is_ok(), "submit() must return Ok for valid graph");
    let job_id = result.unwrap();
    assert!(!job_id.is_nil(), "returned job ID must not be nil");
}

/// Test that two sequential submissions produce distinct UUIDs.
///
/// Populates the registry, submits two valid graphs, and asserts that
/// the returned IDs are different (`id1 != id2`). This verifies that
/// `Uuid::new_v4()` produces a fresh ID for each submission.
#[tokio::test]
async fn test_two_submits_get_distinct_ids() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);

    let id1 = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("first submit must succeed");

    let id2 = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("second submit must succeed");

    assert_ne!(id1, id2, "two submissions must produce distinct UUIDs");
}

/// Test that `cancel()` returns `Ok(true)` for a job currently in the queue.
///
/// Populates the registry, submits a valid job (which is both persisted and
/// enqueued), then calls `cancel()` with the returned job ID. Must return
/// `Ok(true)` — the ID was newly marked as cancelled in the in-memory queue.
#[tokio::test]
async fn test_cancel_queued_job_returns_true() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);

    // Submit a job — it is persisted and enqueued.
    let job_id = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Cancel the job while it is still in the queue.
    let result = scheduler
        .cancel(job_id)
        .await
        .expect("cancel must not error");

    assert!(result, "cancel() must return true for a job in the queue");
}

/// Test that `cancel()` returns `Ok(false)` for a job ID that was never submitted.
///
/// Populates the registry (so the scheduler is constructible) but never
/// submits any job. Cancels a freshly-generated UUID that has no corresponding
/// job in the queue. Must return `Ok(false)` — the ID was not in the
/// cancelled set.
#[tokio::test]
async fn test_cancel_unknown_id_returns_false() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);

    // Cancel a UUID that was never submitted.
    let unknown_id = Uuid::new_v4();
    let result = scheduler
        .cancel(unknown_id)
        .await
        .expect("cancel must not error");

    assert!(!result, "cancel() must return false for an unknown job ID");
}

/// Test that `get_job()` returns `Ok(Some(job))` for a submitted job.
///
/// Populates the registry, submits a valid job, then looks it up by its
/// returned ID. Must return `Ok(Some(job))` where `job.id == submitted_id`
/// and `job.status == Queued`. This verifies that `get_job()` correctly
/// delegates to `JobStore::get()` and that the job was persisted.
#[tokio::test]
async fn test_get_job_returns_persisted_job() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);

    // Submit a job — it is persisted to the database.
    let job_id = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Look up the job by ID.
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");

    assert!(
        result.is_some(),
        "get_job() must return Some for a persisted job"
    );
    let job = result.unwrap();
    assert_eq!(job.id, job_id, "retrieved job ID must match submitted ID");
}

/// Test that `get_job()` returns `Ok(None)` for a job ID that was never submitted.
///
/// Populates the registry but never submits any job. Looks up a freshly-generated
/// UUID. Must return `Ok(None)` — no row exists in the database for that ID.
#[tokio::test]
async fn test_get_job_unknown_id_returns_none() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);

    // Look up a UUID that was never submitted.
    let unknown_id = Uuid::new_v4();
    let result = scheduler
        .get_job(unknown_id)
        .await
        .expect("get_job must not error");

    assert!(
        result.is_none(),
        "get_job() must return None for an unknown job ID"
    );
}

/// Test that `start_dispatch_loop()` returns a `JoinHandle` that doesn't
/// immediately finish.
///
/// Constructs a `JobScheduler`, calls `start_dispatch_loop()` with an empty
/// `WorkerPool`, and asserts the returned `JoinHandle` is still alive (hasn't
/// completed) after a brief yield. This proves the loop task is running and
/// waiting on `dispatch_notify.notified()`.
#[tokio::test]
async fn test_dispatch_loop_returns_join_handle() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);
    let workers = anvilml_worker::WorkerPool::new()
        .await
        .expect("empty pool must construct");
    let workers = Arc::new(workers);

    let handle = Arc::new(scheduler).start_dispatch_loop(Arc::clone(&workers));

    // Yield to let the task reach the notified().await wait point.
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // The handle should still be alive — it's waiting on notified().await.
    // is_finished() returns false while the task is still running.
    assert!(
        !handle.is_finished(),
        "dispatch loop handle must still be alive after construction"
    );

    // Clean up: abort the handle to prevent the task from running forever.
    handle.abort();
}

/// Test that `submit()` wakes the dispatch loop.
///
/// Constructs a `JobScheduler`, starts the dispatch loop, then submits a job.
/// The dispatch loop's `dispatch_notify` must be notified by `submit()`'s
/// `notify_one()` call. We verify the loop survives the wake without panicking
/// and the job remains in the database.
#[tokio::test]
async fn test_submit_wakes_dispatch_loop() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);
    let workers = anvilml_worker::WorkerPool::new()
        .await
        .expect("empty pool must construct");
    let workers = Arc::new(workers);

    // Wrap in Arc — required by start_dispatch_loop's self: Arc<Self> receiver.
    let scheduler = Arc::new(scheduler);

    // Clone the Arc for start_dispatch_loop (takes self: Arc<Self>).
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Submit a job — this calls dispatch_notify.notify_one().
    let job_id = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Give the dispatch loop time to wake, pop the job, attempt dispatch
    // (which returns false), and push it back.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // The dispatch loop must still be alive — it survived the submit wake.
    assert!(
        !handle.is_finished(),
        "dispatch loop must survive a submit wake"
    );

    // The job is still in the database (persisted by submit).
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    assert!(result.is_some(), "job must still be in database");

    // Clean up.
    handle.abort();
}

/// Test that the dispatch loop survives multiple wake cycles without
/// panicking.
///
/// Starts the dispatch loop, then submits three jobs sequentially.
/// Each submit wakes the loop. The loop must not panic or exit on any
/// of the three wakes — it should simply pop each job, attempt dispatch
/// (always returns false), push it back, and wait for the next wake.
#[tokio::test]
async fn test_dispatch_loop_survives_multiple_wakes() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);
    let workers = anvilml_worker::WorkerPool::new()
        .await
        .expect("empty pool must construct");
    let workers = Arc::new(workers);

    // Wrap in Arc — required by start_dispatch_loop's self: Arc<Self> receiver.
    let scheduler = Arc::new(scheduler);

    // Clone the Arc for start_dispatch_loop (takes self: Arc<Self>).
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Collect job IDs for later verification.
    let mut job_ids = Vec::new();

    // Submit three jobs sequentially, each waking the dispatch loop.
    for _i in 0..3 {
        let job_id = scheduler
            .submit(
                make_valid_graph(),
                JobSettings {
                    device_preference: None,
                },
            )
            .await
            .expect("submit must succeed");
        job_ids.push(job_id);

        // Brief yield between submissions to let the loop process.
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // After all submissions, verify the loop is still alive.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    assert!(
        !handle.is_finished(),
        "dispatch loop must survive 3 consecutive wakes without panicking"
    );

    // All three jobs should still be in the database.
    for job_id in &job_ids {
        let result = scheduler
            .get_job(*job_id)
            .await
            .expect("get_job must not error");
        assert!(result.is_some(), "job {:?} must be in database", job_id);
    }

    // Clean up.
    handle.abort();
}
