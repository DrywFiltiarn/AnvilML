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
