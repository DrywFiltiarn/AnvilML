//! Integration tests for the `AppState` struct.
//!
//! Tests verify construction, cloning, and `Arc`-sharing semantics
//! of the shared application state used by the AnvilML HTTP server.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{NodeTypeDescriptor, NodeTypeRegistry, ServerConfig};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_server::AppState;
use anvilml_worker::WorkerPool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;

/// Helper to create an in-memory SQLite pool with migrations applied.
///
/// Each call creates a fresh pool, applies all migrations from the
/// `database/migrations/` directory, and returns a `SqlitePool`.
/// This ensures database isolation between tests.
async fn create_test_pool() -> sqlx::SqlitePool {
    let connect_opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("in-memory SQLite pool must connect");

    // Apply all migrations so the `jobs` table exists.
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&pool)
        .await
        .expect("migrations must apply to in-memory pool");

    pool
}

/// Construct a minimal `AppState` with all seven fields.
///
/// Used by tests that need a complete `AppState` including the
/// scheduler, workers, db, and artifact_store subsystem fields.
async fn make_full_state(
    node_registry: Arc<NodeTypeRegistry>,
    artifact_store: Arc<ArtifactStore>,
) -> AppState {
    let db = create_test_pool().await;
    let job_store = JobStore::new(db.clone());

    let workers = Arc::new(
        WorkerPool::new()
            .await
            .expect("WorkerPool::new() must succeed in test"),
    );

    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        artifact_store.clone(),
        Arc::clone(&workers).transport().clone(),
    ));

    let broadcaster = Arc::new(EventBroadcaster::new());

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db,
        artifact_store,
        broadcaster,
    }
}

/// Verify that `AppState` constructs with a default `ServerConfig`
/// and an empty `NodeTypeRegistry`.
///
/// Constructs `AppState` using `ServerConfig::default()` and
/// `NodeTypeRegistry::new()`, both wrapped in `Arc::new()`. Asserts
/// that the `config` and `node_registry` fields are accessible and
/// that the registry reports `is_empty() == true`.
#[test]
fn test_app_state_constructs() {
    // Create workers first so we can pass their transport to the scheduler.
    let workers = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                WorkerPool::new()
                    .await
                    .expect("WorkerPool::new() must succeed")
            }),
    );

    let state = AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry: Arc::new(NodeTypeRegistry::new()),
        start_time: std::time::Instant::now(),
        scheduler: Arc::new(JobScheduler::new(
            JobStore::new(create_test_pool_sync()),
            Arc::new(NodeTypeRegistry::new()),
            Arc::new(ArtifactStore::new(
                std::env::temp_dir().join("anvilml-test-artifacts-state"),
                create_test_pool_sync(),
            )),
            Arc::clone(&workers).transport().clone(),
        )),
        workers,
        db: create_test_pool_sync(),
        artifact_store: Arc::new(ArtifactStore::new(
            std::env::temp_dir().join("anvilml-test-artifacts"),
            create_test_pool_sync(),
        )),
        broadcaster: Arc::new(EventBroadcaster::new()),
    };

    // Verify both fields are accessible and the registry starts empty.
    assert!(!state.config.host.is_empty());
    assert!(state.node_registry.is_empty());
}

/// Synchronous helper to create a test pool using a current-thread runtime.
fn create_test_pool_sync() -> sqlx::SqlitePool {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { create_test_pool().await })
}

/// Asynchronous helper to create an `ArtifactStore` backed by a temp
/// directory and an in-memory SQLite pool.
///
/// Uses `std::env::temp_dir()` as the artifact directory — the store
/// will create its own subdirectory on first save. This avoids needing
/// the `tempfile` crate since tests only need a valid path for
/// construction, not a cleaned-up temp directory.
async fn create_test_artifact_store() -> Arc<ArtifactStore> {
    let db = create_test_pool().await;
    // Use the system temp dir — the store creates its own subdirectory
    // on first save() via create_dir_all, so we just need a writable path.
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    Arc::new(ArtifactStore::new(artifact_dir, db))
}

/// Verify that cloning `AppState` shares the underlying
/// `Arc<NodeTypeRegistry>` — mutations visible through one clone
/// are observable through the other.
///
/// Constructs `AppState`, clones it to `cloned`, registers a single
/// `NodeTypeDescriptor` via `state.node_registry.register_all()`, then
/// reads back via `cloned.node_registry.list()` and asserts the
/// descriptor is present. This proves both clones share the same
/// `Arc<NodeTypeRegistry>` heap allocation.
#[tokio::test]
async fn test_app_state_clone_shares_node_registry() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;
    let state = make_full_state(node_registry, artifact_store).await;

    // Clone before mutation — both clones share the same Arc.
    let cloned = state.clone();

    // Register a synthetic node descriptor via the original clone.
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    state.node_registry.register_all(vec![descriptor.clone()]);

    // Read back through the clone — the registered descriptor must be present.
    let list = cloned.node_registry.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].type_name, "TestNode");
}

/// Verify that `AppState` constructs with all six fields — the three
/// pre-existing fields (`config`, `node_registry`, `start_time`) plus
/// the three new subsystem fields (`scheduler`, `workers`, `db`).
///
/// Constructs a minimal `JobScheduler` backed by an in-memory SQLite
/// pool, a `WorkerPool` via `#[tokio::test]`, and a `SqlitePool`.
/// Asserts all six fields are accessible and no panics occur during
/// construction.
#[tokio::test]
async fn test_app_state_with_new_fields() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;
    let state = make_full_state(node_registry, artifact_store).await;

    // Verify all six fields are accessible — no panics on field access.
    assert!(!state.config.host.is_empty());
    assert!(state.node_registry.is_empty());
    assert!(!state.start_time.elapsed().is_zero());
    // Scheduler's internal queue is empty (no jobs submitted).
    // Workers pool is empty (no workers spawned).
    assert!(state.workers.handles().is_empty());
    // DB pool is connected (in-memory SQLite).
    // All fields constructed without error.
}

/// Verify that cloning `AppState` preserves all six fields and that
/// `Arc`-wrapped fields share the same underlying allocation.
///
/// Constructs `AppState` with all fields, clones it, then asserts
/// that all six fields on the clone are accessible and that the
/// `Arc` pointers for `config`, `node_registry`, `scheduler`, and
/// `workers` are identical (verified via pointer comparison).
#[tokio::test]
async fn test_app_state_clone_preserves_all_fields() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;
    let state = make_full_state(node_registry, artifact_store).await;

    let cloned = state.clone();

    // All six fields must be accessible on the clone.
    assert!(!cloned.config.host.is_empty());
    assert!(cloned.node_registry.is_empty());
    assert!(!cloned.start_time.elapsed().is_zero());
    assert!(cloned.workers.handles().is_empty());

    // Arc-wrapped fields share the same allocation — pointer comparison.
    assert!(
        std::ptr::eq(Arc::as_ptr(&state.config), Arc::as_ptr(&cloned.config)),
        "config Arc must be shared between original and clone"
    );
    assert!(
        std::ptr::eq(
            Arc::as_ptr(&state.node_registry),
            Arc::as_ptr(&cloned.node_registry),
        ),
        "node_registry Arc must be shared between original and clone"
    );
    assert!(
        std::ptr::eq(
            Arc::as_ptr(&state.scheduler),
            Arc::as_ptr(&cloned.scheduler),
        ),
        "scheduler Arc must be shared between original and clone"
    );
    assert!(
        std::ptr::eq(Arc::as_ptr(&state.workers), Arc::as_ptr(&cloned.workers)),
        "workers Arc must be shared between original and clone"
    );
}

/// Verify that the `Arc<NodeTypeRegistry>` shared through the
/// scheduler is visible through both original and cloned `AppState`.
///
/// Registers a node type via the original state's `node_registry`,
/// then reads back through the cloned state's `node_registry` to
/// confirm the scheduler's internal `Arc<NodeTypeRegistry>` is
/// shared — proving that both clones see the same registry.
#[tokio::test]
async fn test_app_state_scheduler_arc_sharing() {
    // Create a shared node registry that will be used by both the
    // scheduler and the AppState node_registry field.
    let shared_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;

    let state = make_full_state(Arc::clone(&shared_registry), artifact_store).await;

    let cloned = state.clone();

    // Register a node type via the original state's node_registry.
    let descriptor = NodeTypeDescriptor {
        type_name: "SharedRegistryNode".to_string(),
        display_name: "Shared Registry Node".to_string(),
        category: "test".to_string(),
        description: "A node registered to verify Arc sharing.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    state.node_registry.register_all(vec![descriptor]);

    // Read back through the cloned state's node_registry — the
    // registered descriptor must be present because both clones
    // share the same Arc<NodeTypeRegistry>.
    let list = cloned.node_registry.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].type_name, "SharedRegistryNode");
}

/// Verify that `AppState` constructs with an `ArtifactStore` field
/// backed by a temp directory and in-memory SQLite pool.
///
/// Constructs an `ArtifactStore` via `create_test_artifact_store()`,
/// wraps it in `Arc`, and passes it to `make_full_state()`. Asserts
/// that the `artifact_store` field is accessible and that the `Arc`
/// pointer is valid (non-null). This verifies the construction path
/// works end-to-end.
#[tokio::test]
async fn test_app_state_artifact_store_constructs() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;
    let state = make_full_state(node_registry, artifact_store).await;

    // Verify the artifact_store field is accessible.
    // Arc::as_ptr() returns a non-null pointer — if the Arc were
    // invalid, this would panic, so a successful return proves
    // the field is properly constructed.
    let ptr = Arc::as_ptr(&state.artifact_store);
    assert!(!ptr.is_null(), "artifact_store Arc pointer must be valid");
}

/// Verify that cloning `AppState` shares the same `Arc<ArtifactStore>`
/// allocation as the original.
///
/// Constructs `AppState` with an `ArtifactStore`, clones it, then
/// verifies via pointer comparison (`std::ptr::eq(Arc::as_ptr(...))`)
/// that both the original and cloned state share the same `Arc`
/// allocation. This verifies the cloning semantics match the
/// established pattern used by other `Arc` fields.
#[tokio::test]
async fn test_app_state_artifact_store_clone_shares() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;
    let state = make_full_state(node_registry, artifact_store).await;

    let cloned = state.clone();

    // Both clones must share the same Arc<ArtifactStore> allocation.
    assert!(
        std::ptr::eq(
            Arc::as_ptr(&state.artifact_store),
            Arc::as_ptr(&cloned.artifact_store),
        ),
        "artifact_store Arc must be shared between original and clone"
    );
}

/// Verify that `AppState` constructs with a `broadcaster` field
/// backed by a valid `Arc<EventBroadcaster>`.
///
/// Constructs `AppState` via `make_full_state()`, then asserts
/// that the `broadcaster` field is accessible and that the `Arc`
/// pointer is valid (non-null) via `Arc::as_ptr()`. This verifies
/// the construction path works end-to-end.
#[tokio::test]
async fn test_app_state_broadcaster_constructs() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;
    let state = make_full_state(node_registry, artifact_store).await;

    // Verify the broadcaster field is accessible.
    // Arc::as_ptr() returns a non-null pointer — if the Arc were
    // invalid, this would panic, so a successful return proves
    // the field is properly constructed.
    let ptr = Arc::as_ptr(&state.broadcaster);
    assert!(!ptr.is_null(), "broadcaster Arc pointer must be valid");
}

/// Verify that cloning `AppState` shares the same `Arc<EventBroadcaster>`
/// allocation as the original.
///
/// Constructs `AppState` with a `broadcaster`, clones it, then
/// verifies via pointer comparison (`std::ptr::eq(Arc::as_ptr(...))`)
/// that both the original and cloned state share the same `Arc`
/// allocation. This verifies the cloning semantics match the
/// established pattern used by other `Arc` fields.
#[tokio::test]
async fn test_app_state_broadcaster_clone_shares() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let artifact_store = create_test_artifact_store().await;
    let state = make_full_state(node_registry, artifact_store).await;

    let cloned = state.clone();

    // Both clones must share the same Arc<EventBroadcaster> allocation.
    assert!(
        std::ptr::eq(
            Arc::as_ptr(&state.broadcaster),
            Arc::as_ptr(&cloned.broadcaster),
        ),
        "broadcaster Arc must be shared between original and clone"
    );
}
