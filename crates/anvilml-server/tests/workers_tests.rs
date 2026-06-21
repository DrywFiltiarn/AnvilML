//! Integration tests for the workers HTTP handler.
//!
//! Tests cover: empty workers list when no pool is configured, and
//! worker info returned when a pool with a mock worker is present.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{GpuDevice, NodeTypeRegistry, ServerConfig, WorkerStatus};
use anvilml_registry::ModelStore;
use anvilml_scheduler::scheduler::JobScheduler;
use anvilml_server::{build_router, AppState};
use anvilml_worker::{ManagedWorker, WorkerPool};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tower::util::ServiceExt;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn stub_cfg() -> ServerConfig {
    ServerConfig::default()
}

fn stub_device() -> GpuDevice {
    GpuDevice {
        index: 0,
        name: "stub-device".to_string(),
        db_name: None,
        device_type: anvilml_core::DeviceType::Cpu,
        vram_total_mib: 0,
        vram_free_mib: 0,
        driver_version: String::new(),
        pci_vendor_id: 0,
        pci_device_id: 0,
        arch: None,
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Vulkan,
        capabilities_source: anvilml_core::CapabilitySource::DeviceTable,
    }
}

/// Create a minimal WorkerPool with one mock worker for tests.
fn mock_pool_with_one_worker() -> WorkerPool {
    let transport = Arc::new(futures::executor::block_on(async {
        anvilml_ipc::RouterTransport::bind()
            .await
            .expect("bind mock transport")
    }));

    let broadcaster = Arc::new(anvilml_ipc::EventBroadcaster::new());

    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    let (timeout_tx, timeout_rx) = tokio::sync::oneshot::channel::<()>();
    drop(timeout_tx);

    let (_restart_tx, restart_rx) = tokio::sync::watch::channel(0u64);

    let mock_worker = ManagedWorker::new(
        WorkerStatus::Idle,
        msg_tx,
        event_tx,
        None,
        None,
        None,
        None,
        stub_cfg(),
        stub_device(),
        transport.clone(),
        timeout_rx,
        restart_rx,
        "worker-0".to_string(),
        "mock-device".to_string(),
        0,
        None,
        None,
        None,
        None,
    );

    WorkerPool::new(
        vec![(
            mock_worker.get_status(),
            "worker-0".to_string(),
            "mock-device".to_string(),
        )],
        transport,
        broadcaster,
    )
}

/// Build a JobScheduler and ArtifactStore for tests.
async fn test_state(registry: Arc<NodeTypeRegistry>) -> (Arc<JobScheduler>, Arc<ArtifactStore>) {
    let pool = anvilml_registry::open_in_memory().await.unwrap();
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = Arc::new(ArtifactStore::new(artifact_dir, pool.clone()).await);
    let model_store = Arc::new(ModelStore::new(pool.clone()).await);
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(
            anvilml_scheduler::queue::JobQueue::default(),
        )),
        Arc::new(tokio::sync::Mutex::new(
            anvilml_scheduler::ledger::VramLedger::new(),
        )),
        registry.clone(),
        pool,
        Arc::new(anvilml_ipc::EventBroadcaster::new()),
        Arc::clone(&artifact_store),
        model_store,
        None, // cancellation requires a real worker pool
    ));
    (scheduler, artifact_store)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify that GET /v1/workers returns an empty JSON array when
/// AppState.workers is None (test/stub mode).
#[tokio::test]
async fn test_list_workers_returns_empty_when_no_pool() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;

    let router = build_router(state);

    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/workers")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), 200);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(
        json.is_array(),
        "GET /v1/workers must return a JSON array, got: {}",
        json
    );
    assert_eq!(json.as_array().unwrap().len(), 0);
}

/// Verify that GET /v1/workers returns worker info when the pool
/// contains workers.
#[tokio::test]
async fn test_list_workers_returns_pool_data() {
    let pool = mock_pool_with_one_worker();

    let hardware = Arc::new(tokio::sync::RwLock::new(
        anvilml_core::types::HardwareInfo::default(),
    ));
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;

    let state = AppState::new_with_hardware(
        "test-version",
        hardware,
        anvilml_registry::open_in_memory().await.unwrap(),
        Arc::new(
            anvilml_registry::ModelStore::new(anvilml_registry::open_in_memory().await.unwrap())
                .await,
        ),
        Vec::new(),
        Arc::new(pool),
        registry,
        scheduler,
        artifact_store,
    );

    let router = build_router(state);

    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/workers")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), 200);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(
        json.is_array(),
        "GET /v1/workers must return a JSON array, got: {}",
        json
    );
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1, "should return exactly 1 worker");

    assert_eq!(arr[0]["status"], "idle");
    assert_eq!(arr[0]["id"], "worker-0");
}
