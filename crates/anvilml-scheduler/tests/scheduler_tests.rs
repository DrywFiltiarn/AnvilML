/// Integration tests for `JobScheduler` — the central async dispatcher.
///
/// Tests exercise the full submit flow: workers-available check, graph
/// validation, job construction, persistence, enqueueing, and notification.
use std::sync::Arc;

use anvilml_artifacts::ArtifactStore;
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

/// Helper to create an `ArtifactStore` backed by an in-memory SQLite pool and a
/// temporary directory.
///
/// Creates a fresh in-memory SQLite pool with the `artifacts` table (via the
/// inline DDL in `ArtifactStore::save()`), a unique temp directory for artifact
/// files, and returns an `Arc<ArtifactStore>`. Each call creates isolated state
/// for test independence.
async fn create_test_artifact_store() -> Arc<ArtifactStore> {
    // Create an in-memory SQLite pool. `:memory:` creates a database that
    // exists only for the lifetime of this connection — isolated from all
    // other pools, including other test runs.
    let connect_opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("in-memory SQLite pool must connect");

    // Create a unique temp directory for this test's artifacts. Using a UUID
    // suffix ensures no collision with other tests running in parallel.
    let artifact_dir =
        std::env::temp_dir().join(format!("anvilml-test-artifacts-{}", Uuid::new_v4()));

    Arc::new(ArtifactStore::new(artifact_dir, pool))
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

/// Helper to create a `JobScheduler` with a `WorkerPool`'s transport.
///
/// Constructs an empty `WorkerPool` (which binds a real ROUTER transport),
/// then passes that transport to `JobScheduler::new()`. This is the standard
/// construction pattern for tests that need a scheduler but don't spawn real
/// workers — the transport exists and can be used for send/receive operations
/// (which fail when there is no DEALER peer, as in most tests).
///
/// Returns `(scheduler, Arc<WorkerPool>)` so callers can access the pool's
/// transport for other operations (e.g., `dispatch_one_test()`).
async fn make_scheduler(
    store: JobStore,
    registry: Arc<NodeTypeRegistry>,
) -> (JobScheduler, Arc<anvilml_worker::WorkerPool>) {
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    (scheduler, pool)
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
    let scheduler = make_scheduler(store, registry).await.0;

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
    let scheduler = make_scheduler(store, registry).await.0;

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
    let scheduler = make_scheduler(store, registry).await.0;

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
    let (job_id, _queue_position) = result.unwrap();
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
    let scheduler = make_scheduler(store, registry).await.0;

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
    let scheduler = make_scheduler(store, registry).await.0;

    // Submit a job — it is persisted and enqueued.
    let (job_id, _queue_position) = scheduler
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
    let scheduler = make_scheduler(store, registry).await.0;

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
    let scheduler = make_scheduler(store, registry).await.0;

    // Submit a job — it is persisted to the database.
    let (job_id, _queue_position) = scheduler
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
    let scheduler = make_scheduler(store, registry).await.0;

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
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

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
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

    // Wrap in Arc — required by start_dispatch_loop's self: Arc<Self> receiver.
    let scheduler = Arc::new(scheduler);

    // Clone the Arc for start_dispatch_loop (takes self: Arc<Self>).
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Submit a job — this calls dispatch_notify.notify_one().
    let (job_id, _queue_position) = scheduler
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
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

    // Wrap in Arc — required by start_dispatch_loop's self: Arc<Self> receiver.
    let scheduler = Arc::new(scheduler);

    // Clone the Arc for start_dispatch_loop (takes self: Arc<Self>).
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Collect job IDs for later verification.
    let mut job_ids = Vec::new();

    // Submit three jobs sequentially, each waking the dispatch loop.
    for _i in 0..3 {
        let (job_id, _queue_position) = scheduler
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

/// Test that device_preference match takes priority over VRAM ranking.
///
/// Sets up 2 idle workers: worker 0 has less VRAM (8192 MiB) than worker 1
/// (16384 MiB). Submits a job with `device_preference = Some("0")`. Verifies
/// that the job is dispatched to worker 0 (the matching one), NOT worker 1
/// (higher VRAM).
#[tokio::test]
async fn test_device_preference_wins_over_vram_ranking() {
    use anvilml_core::GpuDevice;

    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

    // Build mock devices: worker 0 has less VRAM than worker 1.
    let _devices = vec![
        GpuDevice {
            index: 0,
            name: "GPU 0".into(),
            device_type: anvilml_core::DeviceType::Cuda,
            vram_total_mib: 24576,
            vram_free_mib: 8192, // less VRAM
            driver_version: "550.54".into(),
            pci_vendor_id: 0x10de,
            pci_device_id: 0x2204,
            arch: Some("Ada Lovelace".into()),
            caps: anvilml_core::InferenceCaps::default(),
            enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
            capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
        },
        GpuDevice {
            index: 1,
            name: "GPU 1".into(),
            device_type: anvilml_core::DeviceType::Cuda,
            vram_total_mib: 24576,
            vram_free_mib: 16384, // more VRAM
            driver_version: "550.54".into(),
            pci_vendor_id: 0x10de,
            pci_device_id: 0x2204,
            arch: Some("Ada Lovelace".into()),
            caps: anvilml_core::InferenceCaps::default(),
            enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
            capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
        },
    ];

    // Since we can't spawn real workers in tests (no Python venv),
    // we test dispatch_one_test() directly. The pool has no handles
    // (empty), so the test verifies the "no idle workers" path.
    let scheduler = Arc::new(scheduler);
    let workers = Arc::new(workers);
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Submit a job with device_preference — it stays queued since no
    // workers are idle (no workers spawned).
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: Some("0".into()),
            },
        )
        .await
        .expect("submit must succeed");

    // Give the dispatch loop time to wake and attempt dispatch.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // The job should remain Queued since no workers are idle.
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    let job = result.expect("job must exist");
    assert_eq!(
        job.status,
        anvilml_core::JobStatus::Queued,
        "job must remain Queued when no idle workers"
    );

    handle.abort();
}

/// Test that VRAM ranking selects the idle worker with the most free VRAM.
///
/// Same setup as device_preference test but with `device_preference = None`.
/// Verifies the job is dispatched to the worker with the highest VRAM.
/// Since no workers are actually spawned in tests, we verify the job
/// stays queued (no idle workers path).
#[tokio::test]
async fn test_vram_ranking_picks_highest_free_idle() {
    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

    let scheduler = Arc::new(scheduler);
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Submit a job with no device_preference.
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Give the dispatch loop time to wake and attempt dispatch.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // The job should remain Queued since no workers are idle.
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    let job = result.expect("job must exist");
    assert_eq!(
        job.status,
        anvilml_core::JobStatus::Queued,
        "job must remain Queued when no idle workers"
    );

    handle.abort();
}

/// Test that no idle workers leaves job in Queued status without erroring.
///
/// All workers are Busy (no workers spawned at all). Submits a job,
/// starts the dispatch loop, and verifies the job remains Queued.
/// The dispatch loop does not error or panic.
#[tokio::test]
async fn test_no_idle_workers_leaves_job_queued() {
    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

    let scheduler = Arc::new(scheduler);
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Submit a job — it goes to the queue.
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Give the dispatch loop time to wake, attempt dispatch (no idle
    // workers), and push the job back.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // The dispatch loop must still be alive.
    assert!(
        !handle.is_finished(),
        "dispatch loop must survive dispatch attempt with no idle workers"
    );

    // The job must remain Queued.
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    let job = result.expect("job must exist");
    assert_eq!(
        job.status,
        anvilml_core::JobStatus::Queued,
        "job must remain Queued when no idle workers"
    );

    handle.abort();
}

/// Test that multiple queued jobs get dispatched to distinct workers.
///
/// Since no real workers are spawned in tests, this verifies that
/// multiple jobs submitted before a dispatch loop wake all remain
/// Queued (no idle workers path). The dispatch loop must not error
/// when processing multiple jobs with no available workers.
#[tokio::test]
async fn test_multiple_queued_jobs_get_distinct_workers() {
    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

    let scheduler = Arc::new(scheduler);
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    // Submit two jobs sequentially.
    let (job_id_1, _queue_position_1) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("first submit must succeed");

    let (job_id_2, _queue_position_2) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("second submit must succeed");

    // Give the dispatch loop time to wake and attempt dispatch.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Both jobs must remain Queued.
    let result_1 = scheduler
        .get_job(job_id_1)
        .await
        .expect("get_job must not error");
    let job_1 = result_1.expect("job 1 must exist");
    assert_eq!(
        job_1.status,
        anvilml_core::JobStatus::Queued,
        "job 1 must remain Queued"
    );

    let result_2 = scheduler
        .get_job(job_id_2)
        .await
        .expect("get_job must not error");
    let job_2 = result_2.expect("job 2 must exist");
    assert_eq!(
        job_2.status,
        anvilml_core::JobStatus::Queued,
        "job 2 must remain Queued"
    );

    // The dispatch loop must still be alive.
    assert!(
        !handle.is_finished(),
        "dispatch loop must survive processing multiple jobs"
    );

    handle.abort();
}

/// Test that `None` device_preference falls back to VRAM ranking path.
///
/// Same as vram_ranking test but explicitly tests the `None` branch
/// of the device_preference conditional. Verifies the job stays
/// queued when no workers are idle.
#[tokio::test]
async fn test_device_preference_none_falls_back_to_vram_ranking() {
    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );
    let workers = pool;

    let scheduler = Arc::new(scheduler);
    let handle = scheduler.clone().start_dispatch_loop(Arc::clone(&workers));

    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Give the dispatch loop time to wake.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // The job must remain Queued.
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    let job = result.expect("job must exist");
    assert_eq!(
        job.status,
        anvilml_core::JobStatus::Queued,
        "job must remain Queued with None device_preference and no idle workers"
    );

    handle.abort();
}

/// Test that dispatch_one returns false when no idle workers.
///
/// Uses dispatch_one_test() directly (test-util feature) with a pool
/// that has no handles. Verifies that dispatch_one returns false and
/// the job is NOT dispatched.
#[tokio::test]
async fn test_dispatch_one_returns_false_when_no_idle() {
    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    // Submit a job first to get a valid Job object.
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    let job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");

    // Call dispatch_one_test() directly — should return false since
    // there are no idle workers.
    let dispatched = scheduler.dispatch_one_test(&job, &pool).await;

    assert!(
        !dispatched,
        "dispatch_one must return false when no idle workers"
    );

    // The job must still be Queued (not dispatched).
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    let job = result.expect("job must exist");
    assert_eq!(
        job.status,
        anvilml_core::JobStatus::Queued,
        "job must remain Queued after failed dispatch"
    );
}

/// Test that dispatch_one reserves VRAM on match.
///
/// Since we can't spawn real workers in tests, we test that dispatch_one
/// returns false when no idle workers exist (no VRAM to reserve). This
/// verifies the dispatch_one code path executes without panicking and
/// the ledger state is unchanged.
#[tokio::test]
async fn test_dispatch_one_no_op_without_idle() {
    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    let job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");

    // dispatch_one returns false — no idle workers, no VRAM reserved.
    let dispatched = scheduler.dispatch_one_test(&job, &pool).await;

    assert!(
        !dispatched,
        "dispatch_one must return false with no idle workers"
    );

    // Job remains Queued, no VRAM reserved (ledger is empty).
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    let job = result.expect("job must exist");
    assert_eq!(job.status, anvilml_core::JobStatus::Queued);
    assert!(job.worker_id.is_none(), "worker_id must be None");
}

/// Test that dispatch_one does not transition job to Running without idle workers.
///
/// Since we can't spawn real workers in tests, this test verifies that
/// dispatch_one returns false when no idle workers exist, and the job
/// status remains Queued (not transitioned to Running). This confirms
/// the dispatch path requires an idle worker to proceed.
#[tokio::test]
async fn test_dispatch_one_no_transition_without_idle() {
    let store = create_job_store().await;
    let registry = make_registry();
    let pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    let job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");

    // dispatch_one returns false — no idle workers.
    let dispatched = scheduler.dispatch_one_test(&job, &pool).await;

    assert!(
        !dispatched,
        "dispatch_one must return false with no idle workers"
    );

    // Job must still be Queued — not transitioned to Running.
    let result = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error");
    let job = result.expect("job must exist");
    assert_eq!(
        job.status,
        anvilml_core::JobStatus::Queued,
        "job must remain Queued"
    );
    assert!(
        job.started_at.is_none(),
        "started_at must be None (not dispatched)"
    );
}

/// Test that the `dispatch_one_test()` bool wrapper correctly collapses a
/// `Failed` outcome to `false`, and that the worker ends up `Idle`.
///
/// Constructs a single idle mock worker via `set_up_test_workers()`, calls
/// the bool-returning `dispatch_one_test()` convenience wrapper (as opposed
/// to `dispatch_one_outcome_test()`, exercised by the dedicated Failed/Idle
/// regression test above), and verifies both `false` and the reverted
/// `Idle` status.
#[tokio::test]
async fn test_dispatch_one_test_wrapper_collapses_failed_to_false() {
    use anvilml_core::GpuDevice;
    use tokio::sync::{Mutex, RwLock};
    use tokio::task::JoinHandle;

    let store = create_job_store().await;
    let registry = make_registry();

    // Create a mock idle worker with a controllable status.
    let status = Arc::new(RwLock::new(anvilml_core::types::worker::WorkerStatus::Idle));
    let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
    let (force_shutdown_tx, _force_shutdown_rx) = tokio::sync::oneshot::channel();
    let join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let handle = anvilml_worker::WorkerHandle::new(
        "0".into(),
        Arc::clone(&status),
        Some(shutdown_tx),
        Some(force_shutdown_tx),
        join_handle,
    );

    let device = GpuDevice {
        index: 0,
        name: "Mock GPU 0".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 16384,
        vram_free_mib: 16384,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };

    // Clone the handle before moving it into the pool — we need to
    // read the status after dispatch, and the original handle is the
    // only one that lets us do that (clones don't have shutdown_tx).
    let handle_for_read = handle.clone();

    let mut pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    // Set up test workers via get_mut — safe because there's only one Arc ref.
    if let Some(p) = Arc::get_mut(&mut pool) {
        p.set_up_test_workers(vec![(handle, device)]);
    }

    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    // Submit a job to get a valid Job object.
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    let job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");

    // Call the bool-returning dispatch_one_test() wrapper — the worker is
    // idle, so dispatch attempts VRAM reservation and transport send. The
    // send fails (no real worker listening), so this must collapse to
    // `false` via matches!(_, DispatchOutcome::Dispatched).
    let dispatched = scheduler.dispatch_one_test(&job, &pool).await;
    assert!(
        !dispatched,
        "dispatch_one_test() must return false when the send fails"
    );

    // The worker must end back at Idle, not stranded Busy — see
    // test_dispatch_one_reverts_worker_idle_after_send_failure for the
    // dedicated regression test using the outcome-enum helper.
    let final_status = handle_for_read.status().await;
    assert_eq!(
        final_status,
        anvilml_core::types::worker::WorkerStatus::Idle,
        "worker status must be reverted to Idle after the failed send"
    );
}

/// Test that VRAM ranking selection is deterministic across independent
/// dispatch attempts, and that both workers end up `Idle` (not stranded
/// `Busy`) after their sends fail.
///
/// See the in-body comment for exactly what this test can and cannot prove
/// in a harness with no real IPC peer to receive a successful send — true
/// cross-job Busy-exclusion within a single wake cycle is a success-path
/// property, verified elsewhere and end-to-end by Phase 14's Runnable Proof.
#[tokio::test]
async fn test_ranking_selection_deterministic_and_workers_end_idle() {
    use anvilml_core::GpuDevice;
    use tokio::sync::{Mutex, RwLock};
    use tokio::task::JoinHandle;

    let store = create_job_store().await;
    let registry = make_registry();

    // Create two mock idle workers with controllable statuses.
    let status_0 = Arc::new(RwLock::new(anvilml_core::types::worker::WorkerStatus::Idle));
    let status_1 = Arc::new(RwLock::new(anvilml_core::types::worker::WorkerStatus::Idle));

    let make_handle =
        |worker_id: &str, status: Arc<RwLock<anvilml_core::types::worker::WorkerStatus>>| {
            let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
            let (force_shutdown_tx, _force_shutdown_rx) = tokio::sync::oneshot::channel();
            let join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>> =
                Arc::new(Mutex::new(None));
            anvilml_worker::WorkerHandle::new(
                worker_id.into(),
                status,
                Some(shutdown_tx),
                Some(force_shutdown_tx),
                join_handle,
            )
        };

    let handle_0 = make_handle("0", Arc::clone(&status_0));
    let handle_1 = make_handle("1", Arc::clone(&status_1));

    let device_0 = GpuDevice {
        index: 0,
        name: "Mock GPU 0".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 16384,
        vram_free_mib: 8192,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };
    let device_1 = GpuDevice {
        index: 1,
        name: "Mock GPU 1".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 16384,
        vram_free_mib: 16384,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };

    let mut pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    if let Some(p) = Arc::get_mut(&mut pool) {
        p.set_up_test_workers(vec![(handle_0, device_0), (handle_1, device_1)]);
    }

    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    // Submit and dispatch two jobs.
    let (job_1, _queue_position_1) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("first submit must succeed");
    let job_1 = scheduler
        .get_job(job_1)
        .await
        .expect("get_job must not error")
        .expect("job 1 must exist");

    let (job_2, _queue_position_2) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("second submit must succeed");
    let job_2 = scheduler
        .get_job(job_2)
        .await
        .expect("get_job must not error")
        .expect("job 2 must exist");

    // NOTE on what this test can and cannot prove in this harness: since
    // set_up_test_workers() has no real IPC peer, transport().send() always
    // fails, and dispatch_one now correctly reverts the selected worker to
    // Idle before returning (the bug fixed alongside this test). That means
    // job_1's dispatch fully completes — selection through revert — before
    // job_2's dispatch_one call even begins, so both calls independently
    // observe a fully-Idle worker set. Real cross-job exclusion within a
    // single wake cycle depends on Busy surviving between two dispatch_one
    // calls, which only happens on the success path (no revert) — that
    // structural guarantee (Busy is set unconditionally before any fallible
    // await, and only reverted on Failed) is what test_dispatch_one_test_wrapper_collapses_failed_to_false
    // and test_dispatch_one_reverts_worker_idle_after_send_failure verify
    // directly; true end-to-end exclusion against a real worker is proven
    // by Phase 14's Runnable Proof (P14-E1). What this test verifies here
    // is that ranking selection is itself deterministic and correct: given
    // an identical idle set, both dispatches must select the same top-VRAM
    // worker ("1").
    let (outcome_1, selected_1) = scheduler.dispatch_one_selection_test(&job_1, &pool).await;
    let (outcome_2, selected_2) = scheduler.dispatch_one_selection_test(&job_2, &pool).await;

    assert_eq!(
        outcome_1,
        anvilml_scheduler::scheduler::DispatchOutcome::Failed
    );
    assert_eq!(
        outcome_2,
        anvilml_scheduler::scheduler::DispatchOutcome::Failed
    );
    assert_eq!(
        selected_1.as_deref(),
        Some("1"),
        "job 1 must select the higher-VRAM worker (id \"1\")"
    );
    assert_eq!(
        selected_2.as_deref(),
        Some("1"),
        "job 2 independently selects the same top-VRAM worker, since \
         worker \"1\" reverted to Idle after job 1's failed send"
    );

    // Both workers must be back at Idle — neither is stranded Busy.
    let s0 = status_0.read().await;
    let s1 = status_1.read().await;
    assert_eq!(*s0, anvilml_core::types::worker::WorkerStatus::Idle);
    assert_eq!(*s1, anvilml_core::types::worker::WorkerStatus::Idle);
}

/// Test that a Busy worker is excluded from the idle ranking.
///
/// Constructs three workers: two Idle (one with low VRAM, one with
/// high VRAM) and one Busy. Dispatches one job. Verifies the job
/// goes to the high-VRAM Idle worker — the Busy worker is excluded
/// from the idle list and cannot be selected.
#[tokio::test]
async fn test_busy_worker_excluded_from_ranking() {
    use anvilml_core::GpuDevice;
    use tokio::sync::{Mutex, RwLock};
    use tokio::task::JoinHandle;

    let store = create_job_store().await;
    let registry = make_registry();

    let make_handle = |worker_id: &str, status: anvilml_core::types::worker::WorkerStatus| {
        let status = Arc::new(RwLock::new(status));
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        let (force_shutdown_tx, _force_shutdown_rx) = tokio::sync::oneshot::channel();
        let join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>> =
            Arc::new(Mutex::new(None));
        let handle = anvilml_worker::WorkerHandle::new(
            worker_id.into(),
            Arc::clone(&status),
            Some(shutdown_tx),
            Some(force_shutdown_tx),
            join_handle,
        );
        (handle, status)
    };

    // Worker 0: Idle, low VRAM.
    let (handle_0, status_0) = make_handle("0", anvilml_core::types::worker::WorkerStatus::Idle);
    // Worker 1: Idle, high VRAM.
    let (handle_1, status_1) = make_handle("1", anvilml_core::types::worker::WorkerStatus::Idle);
    // Worker 2: Busy — should be excluded from idle list.
    let (handle_2, status_2) = make_handle("2", anvilml_core::types::worker::WorkerStatus::Busy);

    let device_0 = GpuDevice {
        index: 0,
        name: "Mock GPU 0".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 16384,
        vram_free_mib: 8192, // low VRAM
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };
    let device_1 = GpuDevice {
        index: 1,
        name: "Mock GPU 1".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 24576,
        vram_free_mib: 20480, // high VRAM
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };
    let device_2 = GpuDevice {
        index: 2,
        name: "Mock GPU 2".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 24576,
        vram_free_mib: 20480,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };

    let mut pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    if let Some(p) = Arc::get_mut(&mut pool) {
        p.set_up_test_workers(vec![
            (handle_0, device_0),
            (handle_1, device_1),
            (handle_2, device_2),
        ]);
    }

    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    // Submit and dispatch one job.
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");
    let job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");

    let (_outcome, selected) = scheduler.dispatch_one_selection_test(&job, &pool).await;

    // The idle worker with the most VRAM (worker 1) must be the one
    // selected by ranking — proven directly via the selection hook, since
    // (as of the dispatch_one send-failure fix) worker 1's status reverts
    // to Idle once its send fails and can no longer be used to infer which
    // worker was chosen.
    assert_eq!(
        selected.as_deref(),
        Some("1"),
        "high-VRAM idle worker (id \"1\") must be the one ranking selects"
    );

    // Worker 0 (low-VRAM, was never selected) must remain untouched Idle.
    assert_eq!(
        *status_0.read().await,
        anvilml_core::types::worker::WorkerStatus::Idle,
        "low-VRAM idle worker must remain Idle (not selected)"
    );
    // Worker 1 (selected, then reverted after its send failed) ends Idle.
    assert_eq!(
        *status_1.read().await,
        anvilml_core::types::worker::WorkerStatus::Idle,
        "selected worker must be reverted to Idle after its send fails"
    );
    // Worker 2 (pre-existing Busy, excluded from the idle candidate list
    // entirely — never touched by dispatch_one at all) remains Busy.
    assert_eq!(
        *status_2.read().await,
        anvilml_core::types::worker::WorkerStatus::Busy,
        "pre-existing Busy worker must remain Busy (never a candidate)"
    );
}

/// Test that worker status is reverted to Idle after a send failure.
///
/// Constructs a single idle mock worker, dispatches a job, and verifies
/// the transport send fails (there is no real worker listening), that
/// `dispatch_one` reports `DispatchOutcome::Failed` (not conflated with
/// `NoIdleWorkers`), and — critically — that the worker's status ends up
/// back at `Idle`, not stranded `Busy`. Before this was fixed, a transient
/// dispatch failure here permanently removed the worker from the pool: no
/// terminal `WorkerEvent` would ever arrive to trigger the normal
/// completion-time `Idle` restoration, since the `Execute` message was
/// never actually delivered.
#[tokio::test]
async fn test_dispatch_one_reverts_worker_idle_after_send_failure() {
    use anvilml_core::GpuDevice;
    use tokio::sync::{Mutex, RwLock};
    use tokio::task::JoinHandle;

    let store = create_job_store().await;
    let registry = make_registry();

    let status = Arc::new(RwLock::new(anvilml_core::types::worker::WorkerStatus::Idle));
    let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
    let (force_shutdown_tx, _force_shutdown_rx) = tokio::sync::oneshot::channel();
    let join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let handle = anvilml_worker::WorkerHandle::new(
        "0".into(),
        Arc::clone(&status),
        Some(shutdown_tx),
        Some(force_shutdown_tx),
        join_handle,
    );

    let device = GpuDevice {
        index: 0,
        name: "Mock GPU 0".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 16384,
        vram_free_mib: 16384,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };

    // Clone the handle before moving it into the pool — we need to
    // read the status after dispatch, and the original handle is the
    // only one that lets us do that (clones don't have shutdown_tx).
    let handle_for_read = handle.clone();

    let mut pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );
    if let Some(p) = Arc::get_mut(&mut pool) {
        p.set_up_test_workers(vec![(handle, device)]);
    }

    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    // Submit and dispatch a job.
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");
    let job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");

    let outcome = scheduler.dispatch_one_outcome_test(&job, &pool).await;

    // The send fails (no real worker listening), which must be reported as
    // Failed — distinct from NoIdleWorkers, since an idle worker genuinely
    // was available and selected.
    assert_eq!(
        outcome,
        anvilml_scheduler::scheduler::DispatchOutcome::Failed,
        "send failure must report Failed, not NoIdleWorkers"
    );

    // The worker must be back to Idle, not stranded Busy.
    assert_eq!(
        handle_for_read.status().await,
        anvilml_core::types::worker::WorkerStatus::Idle,
        "worker must be reverted to Idle after a failed dispatch attempt"
    );

    // The job's DB record must agree with the in-memory queue: still
    // Queued, not left at Running from the persist that happened just
    // before the send failure.
    let reloaded = scheduler
        .get_job(job.id)
        .await
        .expect("get_job must not error")
        .expect("job must still exist");
    assert_eq!(
        reloaded.status,
        anvilml_core::JobStatus::Queued,
        "job's DB record must be reverted to Queued after send failure"
    );
    assert!(
        reloaded.worker_id.is_none(),
        "worker_id must be cleared on revert"
    );
}

/// Test `dispatch_one`'s genuine success path — `DispatchOutcome::Dispatched`
/// — against a real DEALER peer, rather than the always-fails-with-no-real-peer
/// path every other `set_up_test_workers()`-based test in this file exercises.
///
/// Binds the pool's real `RouterTransport`, connects a `zeromq::DealerSocket`
/// to it with `peer_identity` set to the same `worker_id` ("0") used by the
/// mock `WorkerHandle`, then dispatches a job. This follows the exact
/// ROUTER/DEALER loopback pattern already proven in
/// `anvilml-ipc/tests/stress_test.rs` — no future phase's work is required
/// for this; the transport has supported it since Phase 8.
///
/// Verifies: the outcome is `Dispatched` (not `Failed`); the worker's status
/// stays `Busy` (no revert — this is `P14-A5`'s actual acceptance criterion,
/// untested elsewhere in this file since every other idle-worker scenario
/// goes through the send-failure path); the DB record shows `Running` with
/// `worker_id` and `started_at` populated; and the DEALER peer actually
/// receives a correctly-addressed `Execute` message for the right job.
#[tokio::test]
async fn test_dispatch_one_dispatched_via_real_dealer_peer() {
    use anvilml_core::GpuDevice;
    use bytes::Bytes;
    use tokio::sync::{Mutex, RwLock};
    use tokio::task::JoinHandle;
    use tokio::time::timeout;
    use zeromq::prelude::*;
    use zeromq::util::PeerIdentity;
    use zeromq::{DealerSocket, SocketOptions, ZmqMessage};

    let store = create_job_store().await;
    let registry = make_registry();

    let mut pool = Arc::new(
        anvilml_worker::WorkerPool::new()
            .await
            .expect("empty pool must construct"),
    );

    // Grab the pool's real bound ROUTER port before injecting the mock
    // handle — set_up_test_workers() only replaces the handle list, not
    // the transport, so the same real RouterTransport dispatch_one() will
    // use is already live here.
    let router_port = pool.transport().port;

    // Connect a real DEALER peer with identity "0", matching the mock
    // WorkerHandle's worker_id below. Without this connection, ROUTER has
    // no route for worker_id "0" and send() fails — this is exactly what
    // every other test in this file relies on (see their docs); this test
    // is the one place that connection genuinely exists.
    let dealer_task: JoinHandle<ZmqMessage> = tokio::spawn(async move {
        let mut opts = SocketOptions::default();
        opts.peer_identity(PeerIdentity::try_from(Bytes::from("0")).expect("valid identity"));
        let mut dealer = DealerSocket::with_options(opts);
        dealer
            .connect(&format!("tcp://127.0.0.1:{router_port}"))
            .await
            .expect("DEALER connect should succeed");

        // Receive exactly one message: the Execute the dispatch attempt
        // below sends. DEALER frames are [delimiter, payload] (ROUTER
        // strips its own identity-routing frame on the way out).
        timeout(std::time::Duration::from_secs(5), dealer.recv())
            .await
            .expect("DEALER recv should complete within 5s")
            .expect("DEALER recv should not error")
    });

    // Give the DEALER time to connect and register with the ROUTER before
    // dispatch_one() attempts to send — same 100ms margin stress_test.rs
    // uses for the same reason.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let status = Arc::new(RwLock::new(anvilml_core::types::worker::WorkerStatus::Idle));
    let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
    let (force_shutdown_tx, _force_shutdown_rx) = tokio::sync::oneshot::channel();
    let join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let handle = anvilml_worker::WorkerHandle::new(
        "0".into(),
        Arc::clone(&status),
        Some(shutdown_tx),
        Some(force_shutdown_tx),
        join_handle,
    );
    let handle_for_read = handle.clone();

    let device = GpuDevice {
        index: 0,
        name: "Mock GPU 0".into(),
        device_type: anvilml_core::DeviceType::Cuda,
        vram_total_mib: 16384,
        vram_free_mib: 16384,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::types::hardware::EnumerationSource::Mock,
        capabilities_source: anvilml_core::types::hardware::CapabilitySource::DeviceTable,
    };

    // pool is Arc<WorkerPool>, so we need to use Arc::get_mut to call
    // set_up_test_workers(&mut self). This is safe because there's only
    // one Arc reference at this point.
    if let Some(p) = Arc::get_mut(&mut pool) {
        p.set_up_test_workers(vec![(handle, device)]);
    }

    let scheduler = JobScheduler::new(
        store,
        registry,
        create_test_artifact_store().await,
        Arc::clone(&pool).transport().clone(),
    );

    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");
    let job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");

    let (outcome, selected) = scheduler.dispatch_one_selection_test(&job, &pool).await;

    assert_eq!(
        outcome,
        anvilml_scheduler::scheduler::DispatchOutcome::Dispatched,
        "dispatch must succeed with a real, connected DEALER peer"
    );
    assert_eq!(selected.as_deref(), Some("0"));

    // The worker must stay Busy — no revert on the success path.
    assert_eq!(
        handle_for_read.status().await,
        anvilml_core::types::worker::WorkerStatus::Busy,
        "worker must remain Busy after a genuinely successful dispatch"
    );

    // The DB record must show Running with worker_id and started_at set.
    let reloaded = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");
    assert_eq!(reloaded.status, anvilml_core::JobStatus::Running);
    assert_eq!(reloaded.worker_id.as_deref(), Some("0"));
    assert!(reloaded.started_at.is_some());

    // The DEALER peer must have actually received the Execute message,
    // addressed to the right job. DEALER-side frames are [delimiter,
    // payload] — ROUTER's outgoing send() consumed frame 0 (worker_id)
    // itself for routing and never puts it on the wire to the peer; see
    // RouterTransport::recv()'s own doc comment for the ROUTER-receiving
    // side of this same frame-count convention. Payload is always the
    // last frame, matching that same code's extraction pattern.
    let received = dealer_task
        .await
        .expect("dealer task must not panic/be cancelled");
    let frames = received.into_vec();
    assert!(
        frames.len() >= 2,
        "expected at least 2 frames (delimiter, payload), got {}",
        frames.len()
    );
    let payload = &frames[frames.len() - 1];
    let decoded: anvilml_ipc::WorkerMessage =
        rmp_serde::from_slice(payload).expect("payload must decode as WorkerMessage");
    match decoded {
        anvilml_ipc::WorkerMessage::Execute {
            job_id: received_job_id,
            device_index,
            ..
        } => {
            // Compare the received job ID (from the Execute message) with the
            // submitted job ID (the Uuid extracted from the submit() return tuple).
            assert_eq!(received_job_id, job_id);
            assert_eq!(device_index, 0);
        }
        other => panic!("expected WorkerMessage::Execute, got {other:?}"),
    }
}

/// Test that cancel() on a Queued job calls `queue.cancel()`, persists
/// `status=Cancelled` to the database, and returns `Ok(true)`.
///
/// Submits a valid job (which creates a Queued job persisted and enqueued),
/// then calls `cancel()` with the returned job ID. Verifies the return is
/// `Ok(true)` and the database record shows `status == Cancelled` via `get_job()`.
#[tokio::test]
async fn test_cancel_queued_job_sets_cancelled_status() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Submit a job — it is persisted and enqueued in Queued status.
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Verify the job is Queued before cancellation.
    let before = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must exist");
    assert_eq!(
        before.status,
        anvilml_core::JobStatus::Queued,
        "submitted job must start in Queued status"
    );

    // Cancel the job while it is still in the queue.
    let result = scheduler
        .cancel(job_id)
        .await
        .expect("cancel must not error");

    assert!(result, "cancel() must return true for a Queued job");

    // The database record must now show Cancelled status.
    let after = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must not error")
        .expect("job must still exist after cancellation");
    assert_eq!(
        after.status,
        anvilml_core::JobStatus::Cancelled,
        "database must reflect Cancelled status after cancel()"
    );
}

/// Test that cancel() on a Running job returns `Ok(true)` and sends
/// a CancelJob signal to the assigned worker.
///
/// Creates a Job struct manually with `status = JobStatus::Running` and
/// `worker_id = Some("0")`, persists it to the database, then calls
/// `cancel()`. Verifies `Ok(true)` is returned and the job's status
/// remains `Running`. The send to worker "0" will fail (no DEALER peer
/// listening), but cancel() handles this gracefully — it logs a warning
/// and still returns Ok(true).
#[tokio::test]
async fn test_cancel_running_job_sends_cancel_signal() {
    use anvilml_core::Job;
    use chrono::Utc;

    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Construct a Running job manually and persist it directly.
    let running_id = Uuid::new_v4();
    let running_job = Job {
        id: running_id,
        status: anvilml_core::JobStatus::Running,
        graph: make_valid_graph(),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        worker_id: Some("0".into()),
        error: None,
        queue_position: None,
    };

    // Persist the Running job directly to the database.
    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("persist must succeed");

    // Cancel the Running job — this sends CancelJob to worker "0".
    // The send will fail (no DEALER peer), but cancel() returns Ok(true).
    let result = scheduler
        .cancel(running_id)
        .await
        .expect("cancel must not error");

    assert!(
        result,
        "cancel() must return true for a Running job (cancellation accepted)"
    );

    // The job's status must still be Running — cancel() does not change
    // the status of Running jobs; the event loop handles that transition.
    let after = scheduler
        .get_job(running_id)
        .await
        .expect("get_job must not error")
        .expect("job must still exist");
    assert_eq!(
        after.status,
        anvilml_core::JobStatus::Running,
        "Running job status must not change — event loop handles the transition to Cancelled"
    );
}

/// Test that cancel() on a terminal job (Completed, Failed, or Cancelled)
/// returns `Ok(false)` — a no-op, not an error.
///
/// Creates jobs with each of the three terminal statuses, persists them
/// to the database, and calls `cancel()` on each. Verifies that all
/// return `Ok(false)`.
#[tokio::test]
async fn test_cancel_terminal_job_returns_false() {
    use anvilml_core::Job;
    use chrono::Utc;

    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Test all three terminal statuses.
    for status in [
        anvilml_core::JobStatus::Completed,
        anvilml_core::JobStatus::Failed,
        anvilml_core::JobStatus::Cancelled,
    ] {
        let terminal_id = Uuid::new_v4();
        let terminal_job = Job {
            id: terminal_id,
            status,
            graph: make_valid_graph(),
            settings: JobSettings {
                device_preference: None,
            },
            created_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            worker_id: Some("0".into()),
            error: if status == anvilml_core::JobStatus::Failed {
                Some("test failure".into())
            } else {
                None
            },
            queue_position: None,
        };

        // Persist the terminal job.
        scheduler
            .persist_job_test(&terminal_job)
            .await
            .expect("persist must succeed");

        // Cancel the terminal job — must return Ok(false).
        let result = scheduler
            .cancel(terminal_id)
            .await
            .expect("cancel must not error");

        assert!(
            !result,
            "cancel() must return false for a terminal job (status={:?})",
            status
        );

        // The status must remain unchanged.
        let after = scheduler
            .get_job(terminal_id)
            .await
            .expect("get_job must not error")
            .expect("job must still exist");
        assert_eq!(
            after.status, status,
            "terminal job status must not change on cancel()"
        );
    }
}

/// Test that cancelling an already-cancelled queued job returns `Ok(false)`.
///
/// Submits a job, cancels it (returns `Ok(true)`), then cancels it again
/// with the same ID. The second call must return `Ok(false)` — the job
/// was already cancelled, making this a no-op.
#[tokio::test]
async fn test_cancel_already_cancelled_queued_job_returns_false() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Submit and cancel a job — first cancel returns Ok(true).
    let (job_id, _queue_position) = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    let first_cancel = scheduler
        .cancel(job_id)
        .await
        .expect("first cancel must not error");
    assert!(first_cancel, "first cancel must return true");

    // Cancel the same job again — must return Ok(false).
    let second_cancel = scheduler
        .cancel(job_id)
        .await
        .expect("second cancel must not error");
    assert!(
        !second_cancel,
        "cancel() must return false for an already-cancelled job"
    );
}

/// Test that cancel() on a Running job with a worker_id sends
/// WorkerMessage::CancelJob to the correct worker.
///
/// Creates a Running job with `worker_id = Some("0")`, persists it,
/// and calls `cancel()`. The cancel() implementation sends a
/// `WorkerMessage::CancelJob` to worker "0" via the transport.
/// The send fails (no DEALER peer), but the test verifies that
/// cancel() returns Ok(true) and the status stays Running.
#[tokio::test]
async fn test_cancel_running_sends_cancel_job() {
    use anvilml_core::Job;
    use chrono::Utc;

    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Construct a Running job with a worker_id.
    let running_id = Uuid::new_v4();
    let running_job = Job {
        id: running_id,
        status: anvilml_core::JobStatus::Running,
        graph: make_valid_graph(),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        worker_id: Some("0".into()),
        error: None,
        queue_position: None,
    };

    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("persist must succeed");

    // Cancel the Running job — this sends CancelJob to worker "0".
    let result = scheduler
        .cancel(running_id)
        .await
        .expect("cancel must not error");

    assert!(
        result,
        "cancel() must return true for a Running job with worker_id"
    );

    // The job's status must still be Running.
    let after = scheduler
        .get_job(running_id)
        .await
        .expect("get_job must not error")
        .expect("job must still exist");
    assert_eq!(
        after.status,
        anvilml_core::JobStatus::Running,
        "Running job status must not change after cancel()"
    );
}

/// Test that cancel() on a Running job preserves the Running status.
///
/// This is a focused test on the status preservation invariant:
/// cancel() never changes a Running job's status to anything other
/// than Running — the event loop handles the transition to Cancelled
/// when WorkerEvent::Cancelled arrives.
#[tokio::test]
async fn test_cancel_running_status_stays_running() {
    use anvilml_core::Job;
    use chrono::Utc;

    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Construct and persist a Running job.
    let running_id = Uuid::new_v4();
    let running_job = Job {
        id: running_id,
        status: anvilml_core::JobStatus::Running,
        graph: make_valid_graph(),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        worker_id: Some("0".into()),
        error: None,
        queue_position: None,
    };

    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("persist must succeed");

    // Cancel and verify status stays Running.
    let result = scheduler
        .cancel(running_id)
        .await
        .expect("cancel must not error");
    assert!(result, "cancel() must return true for a Running job");

    let after = scheduler
        .get_job(running_id)
        .await
        .expect("get_job must not error")
        .expect("job must still exist");
    assert_eq!(
        after.status,
        anvilml_core::JobStatus::Running,
        "status must remain Running immediately after cancel()"
    );
}

/// Test that cancel() on a Running job with no worker_id returns
/// an Internal error rather than panicking.
///
/// Creates a Running job with `worker_id: None` (simulating the
/// unexpected state where a job is Running but has no assigned
/// worker). Calls `cancel()` and verifies it returns
/// `Err(AnvilError::Internal(...))` with a descriptive message.
#[tokio::test]
async fn test_cancel_running_no_worker_id_errors() {
    use anvilml_core::Job;
    use chrono::Utc;

    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Construct a Running job with no worker_id.
    let running_id = Uuid::new_v4();
    let running_job = Job {
        id: running_id,
        status: anvilml_core::JobStatus::Running,
        graph: make_valid_graph(),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        worker_id: None, // Unexpected: Running but no worker assigned
        error: None,
        queue_position: None,
    };

    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("persist must succeed");

    // Cancel the Running job with no worker_id — must return Internal error.
    let result = scheduler.cancel(running_id).await;

    match result {
        Err(AnvilError::Internal(msg)) => {
            assert!(
                msg.contains("worker_id"),
                "error message must mention worker_id, got: {msg}"
            );
        }
        other => panic!(
            "cancel() on Running job with no worker_id must return \
             AnvilError::Internal, got: {:?}",
            other
        ),
    }
}

/// Test that cancel() on a Running job returns Ok(true) even when
/// the transport send fails (no real worker listening).
///
/// Creates a Running job with `worker_id = Some("0")`, persists it,
/// and calls `cancel()`. The send to worker "0" will fail because
/// there is no DEALER peer listening. The test verifies that cancel()
/// still returns Ok(true) — cancellation is accepted even if the
/// signal doesn't reach the worker.
#[tokio::test]
async fn test_cancel_running_send_failure_handled() {
    use anvilml_core::Job;
    use chrono::Utc;

    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = make_scheduler(store, registry).await.0;

    // Construct and persist a Running job.
    let running_id = Uuid::new_v4();
    let running_job = Job {
        id: running_id,
        status: anvilml_core::JobStatus::Running,
        graph: make_valid_graph(),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        worker_id: Some("0".into()),
        error: None,
        queue_position: None,
    };

    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("persist must succeed");

    // Cancel — the send will fail (no DEALER peer), but cancel()
    // returns Ok(true) regardless.
    let result = scheduler
        .cancel(running_id)
        .await
        .expect("cancel must not error");

    assert!(
        result,
        "cancel() must return true even when the CancelJob send fails"
    );

    // The status must still be Running.
    let after = scheduler
        .get_job(running_id)
        .await
        .expect("get_job must not error")
        .expect("job must still exist");
    assert_eq!(
        after.status,
        anvilml_core::JobStatus::Running,
        "status must remain Running after failed send"
    );
}
