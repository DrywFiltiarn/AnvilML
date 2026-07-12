//! Integration tests for the `GET /v1/workers` handler.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use std::sync::Arc;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::types::hardware::{CapabilitySource, EnumerationSource};
use anvilml_core::types::worker::WorkerStatus;
use anvilml_core::{
    DeviceType, EnvReport, GpuDevice, HardwareInfo, InferenceCaps, NodeTypeRegistry,
    ProvisioningState, ServerConfig,
};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::{JobStore, ModelStore};
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::{WorkerHandle, WorkerPool};
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle as TokioJoinHandle;
use tower::util::ServiceExt;

/// Build a `GpuDevice` stub with the given index/type — the fields beyond
/// index and device_type don't matter for these tests, only that the struct
/// is valid.
fn make_test_device(index: u32, device_type: DeviceType) -> GpuDevice {
    GpuDevice {
        index,
        name: format!("Mock GPU {index}"),
        device_type,
        vram_total_mib: 16384,
        vram_free_mib: 16384,
        driver_version: "550.54".into(),
        pci_vendor_id: 0x10de,
        pci_device_id: 0x2204,
        arch: Some("Ada Lovelace".into()),
        caps: InferenceCaps::default(),
        enumeration_source: EnumerationSource::Mock,
        capabilities_source: CapabilitySource::DeviceTable,
    }
}

/// Build a `WorkerHandle` with a controllable, pre-set status. Mirrors the
/// helper pattern established in `crates/anvilml-server/tests/stats_tick_tests.rs`.
fn make_test_handle(worker_id: &str, status: WorkerStatus) -> WorkerHandle {
    let status = Arc::new(RwLock::new(status));
    let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
    let (force_shutdown_tx, _force_shutdown_rx) = tokio::sync::oneshot::channel();
    let join_handle: Arc<Mutex<Option<TokioJoinHandle<()>>>> = Arc::new(Mutex::new(None));
    WorkerHandle::new(
        worker_id.into(),
        status,
        Some(shutdown_tx),
        Some(force_shutdown_tx),
        join_handle,
    )
}

/// Helper to create an in-memory SQLite pool with migrations applied.
async fn make_test_pool() -> sqlx::SqlitePool {
    let connect_opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&pool)
        .await
        .expect("migrations must apply to in-memory pool");

    pool
}

/// Construct a minimal `AppState` suitable for HTTP handler tests.
///
/// Creates stub values for `scheduler`, `workers`, and `db`. The `workers`
/// field is left as an empty pool (no workers injected).
async fn make_test_state(node_registry: Arc<NodeTypeRegistry>) -> AppState {
    let db = make_test_pool().await;
    let job_store = JobStore::new(db.clone());

    // Construct the artifact store before the scheduler so it can be passed
    // to JobScheduler::new(). The artifact directory is a temp path and the
    // pool is in-memory, so no real files are created.
    let artifact_store = Arc::new(ArtifactStore::new(
        std::env::temp_dir().join("anvilml-test-artifacts"),
        db.clone(),
    ));

    // Empty pool — no workers injected. This is sufficient for tests that
    // only care about the handler returning 200 with an empty array.
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

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db: db.clone(),
        artifact_store,
        broadcaster: Arc::new(EventBroadcaster::new()),
        hardware: Arc::new(RwLock::new(HardwareInfo {
            host: anvilml_core::HostInfo {
                hostname: "test-host".to_string(),
                os: "Linux".to_string(),
            },
            gpus: vec![],
            inference_caps: anvilml_core::InferenceCaps::default(),
        })),
        env_report: Arc::new(RwLock::new(EnvReport {
            python_path: Some("./worker/.venv/bin/python3".to_string()),
            python_version: None,
            torch_version: None,
            provisioning: ProvisioningState::NotStarted,
            preflight_ok: false,
            reason: None,
            node_types: Vec::new(),
        })),
        model_store: Arc::new(ModelStore::new(db.clone())),
    }
}

/// Construct an `AppState` pre-populated with mock workers via
/// `WorkerPool::set_up_test_workers()` (`test-utils` feature).
async fn make_test_state_with_workers(
    node_registry: Arc<NodeTypeRegistry>,
    workers: Vec<(WorkerHandle, GpuDevice)>,
) -> AppState {
    let db = make_test_pool().await;
    let job_store = JobStore::new(db.clone());

    let artifact_store = Arc::new(ArtifactStore::new(
        std::env::temp_dir().join("anvilml-test-artifacts"),
        db.clone(),
    ));

    // Build the pool, inject mock workers, then wrap in Arc for AppState.
    let mut pool = WorkerPool::new()
        .await
        .expect("WorkerPool::new() must succeed in test");
    pool.set_up_test_workers(workers);
    let workers = Arc::new(pool);

    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        artifact_store.clone(),
        Arc::clone(&workers).transport().clone(),
    ));

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db: db.clone(),
        artifact_store,
        broadcaster: Arc::new(EventBroadcaster::new()),
        hardware: Arc::new(RwLock::new(HardwareInfo {
            host: anvilml_core::HostInfo {
                hostname: "test-host".to_string(),
                os: "Linux".to_string(),
            },
            gpus: vec![],
            inference_caps: anvilml_core::InferenceCaps::default(),
        })),
        env_report: Arc::new(RwLock::new(EnvReport {
            python_path: Some("./worker/.venv/bin/python3".to_string()),
            python_version: None,
            torch_version: None,
            provisioning: ProvisioningState::NotStarted,
            preflight_ok: false,
            reason: None,
            node_types: Vec::new(),
        })),
        model_store: Arc::new(ModelStore::new(db.clone())),
    }
}

/// Verify that `GET /v1/workers` returns 200 OK with a JSON array whose
/// elements match the injected mock workers' `worker_id`, `status`,
/// `device_index`, and `device_type`.
///
/// Constructs an `AppState` with two mock workers (Idle/Cuda and Busy/Cpu),
/// builds the router, sends a `GET /v1/workers` request, and asserts the
/// response status is `StatusCode::OK` and each element's fields match the
/// injected worker handles.
#[tokio::test]
async fn test_workers_list_returns_current_pool_state() {
    let node_registry = Arc::new(NodeTypeRegistry::new());

    let workers = vec![
        (
            make_test_handle("0", WorkerStatus::Idle),
            make_test_device(0, DeviceType::Cuda),
        ),
        (
            make_test_handle("1", WorkerStatus::Busy),
            make_test_device(1, DeviceType::Cpu),
        ),
    ];

    let state = make_test_state_with_workers(node_registry, workers).await;
    let router = build_router(state);
    let req = Request::get("/v1/workers").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    assert!(body.is_array());
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2);

    // Verify each worker's fields match the injected handles.
    let w0 = &items[0];
    assert_eq!(w0["worker_id"], "0");
    assert_eq!(w0["status"], "idle");
    assert_eq!(w0["device_index"], 0);
    assert_eq!(w0["device_type"], "cuda");

    let w1 = &items[1];
    assert_eq!(w1["worker_id"], "1");
    assert_eq!(w1["status"], "busy");
    assert_eq!(w1["device_index"], 1);
    assert_eq!(w1["device_type"], "cpu");
}

/// Verify that `GET /v1/workers` returns 200 OK with an empty JSON array
/// `[]` when the pool has zero workers — not `null`, not an error body.
///
/// Constructs an `AppState` with an empty `WorkerPool` (no workers injected
/// via `set_up_test_workers()`), builds the router, sends a `GET /v1/workers`
/// request, and asserts the response status is `StatusCode::OK` and the body
/// is an empty JSON array. This confirms the handler returns an empty array,
/// not a 404 or error response.
#[tokio::test]
async fn test_workers_list_empty_returns_empty_array() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/v1/workers").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

/// Verify that the `GET /v1/workers` JSON response contains exactly the
/// fields `worker_id` (string), `status` (string matching a `WorkerStatus`
/// variant in snake_case), `device_index` (integer), `device_type` (string
/// matching a `DeviceType` variant in snake_case), `pid` (null), and
/// `current_job_id` (null).
///
/// Constructs an `AppState` with one mock worker, sends a `GET /v1/workers`
/// request, and asserts that the single response element contains exactly
/// the six expected fields with the correct types. This verifies the
/// `WorkerInfo` serde representation is correct.
#[tokio::test]
async fn test_workers_response_shape_matches_workerinfo() {
    let node_registry = Arc::new(NodeTypeRegistry::new());

    let workers = vec![(
        make_test_handle("0", WorkerStatus::Idle),
        make_test_device(0, DeviceType::Cuda),
    )];

    let state = make_test_state_with_workers(node_registry, workers).await;
    let router = build_router(state);
    let req = Request::get("/v1/workers").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let items = body.as_array().expect("response must be a JSON array");
    assert_eq!(items.len(), 1);

    let item = &items[0];

    // Verify each field's type and content.
    assert!(
        item["worker_id"].is_string(),
        "worker_id must be a string, got {:?}",
        item["worker_id"]
    );
    assert_eq!(item["worker_id"], "0");

    assert!(
        item["status"].is_string(),
        "status must be a string, got {:?}",
        item["status"]
    );
    assert_eq!(item["status"], "idle");

    assert!(
        item["device_index"].is_i64(),
        "device_index must be an integer, got {:?}",
        item["device_index"]
    );
    assert_eq!(item["device_index"], 0);

    assert!(
        item["device_type"].is_string(),
        "device_type must be a string, got {:?}",
        item["device_type"]
    );
    assert_eq!(item["device_type"], "cuda");

    // pid and current_job_id are always None in WorkerPool::list() —
    // neither is tracked at the WorkerHandle/WorkerPool layer.
    assert!(
        item["pid"].is_null(),
        "pid must be null (not tracked at pool layer), got {:?}",
        item["pid"]
    );
    assert!(
        item["current_job_id"].is_null(),
        "current_job_id must be null (not tracked at pool layer), got {:?}",
        item["current_job_id"]
    );

    // Assert no extra fields exist — the response must contain exactly
    // the six fields defined by `WorkerInfo`. serde_json sorts keys
    // alphabetically, so compare as sets rather than by order.
    let expected_keys: std::collections::HashSet<&str> = [
        "worker_id",
        "status",
        "device_index",
        "device_type",
        "pid",
        "current_job_id",
    ]
    .into_iter()
    .collect();
    let actual_keys: std::collections::HashSet<&str> = item
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(
        actual_keys, expected_keys,
        "response must contain exactly the WorkerInfo fields, got {:?}",
        actual_keys
    );
}
