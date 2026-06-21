//! Integration tests for the system HTTP handlers.
//!
//! Tests cover: system env stub and system hardware info.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::NodeTypeRegistry;
use anvilml_registry::{open_in_memory, ModelStore};
use anvilml_scheduler::scheduler::JobScheduler;
use anvilml_server::{build_router, AppState};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

/// Build a JobScheduler and ArtifactStore for tests.
async fn test_state(registry: Arc<NodeTypeRegistry>) -> (Arc<JobScheduler>, Arc<ArtifactStore>) {
    let pool = open_in_memory().await.unwrap();
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = Arc::new(ArtifactStore::new(artifact_dir, pool.clone()).await);
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
        None, // cancellation requires a real worker pool
    ));
    (scheduler, artifact_store)
}

/// Verify that the system env handler returns HTTP 200 with a JSON body
/// containing the default `EnvReport` values: `preflight_ok` is `false`
/// and `provisioning` is `"not_started"`.
///
/// Exercises the production `build_router` path rather than duplicating
/// the routing logic inline. Uses `Router::oneshot` to exercise the full
/// handler pipeline (state extraction, handler execution, response
/// serialization) without binding a live TCP listener.
#[tokio::test]
async fn test_system_env_returns_200_with_default_report() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Build a GET request to /v1/system/env.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/system/env")
        .body(axum::body::Body::empty())
        .unwrap();

    // Dispatch the request through the router.
    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 status.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the `preflight_ok` field is false (default stub value).
    assert_eq!(json["preflight_ok"], false);

    // Assert the `provisioning` field is "not_started" (default stub value).
    assert_eq!(json["provisioning"], "not_started");
}

/// Verify that the system hardware info handler returns HTTP 200 with a
/// JSON body containing a valid `HardwareInfo` structure with a non-empty
/// `gpus` array.
///
/// Exercises the production `build_router` path via `new_with_hardware`
/// which populates `AppState.hardware` with a `HardwareInfo` containing
/// one synthetic GPU device. Uses `Router::oneshot` to exercise the full
/// handler pipeline without binding a live TCP listener.
#[tokio::test]
async fn test_system_returns_200_with_hardware_info() {
    // Build a HardwareInfo with one synthetic GPU device to match the
    // expected output of detect_all_devices() (which always returns at
    // least one CPU device, and GPU devices when present).
    let hardware_info = anvilml_core::types::HardwareInfo {
        host: anvilml_core::types::HostInfo {
            os: "Linux 6.1.0".to_string(),
            cpu: "Test CPU".to_string(),
            ram_total_mib: 16384,
        },
        gpus: vec![anvilml_core::types::GpuDevice {
            index: 0,
            name: "Test GPU".to_string(),
            db_name: None,
            device_type: anvilml_core::types::DeviceType::Cuda,
            vram_total_mib: 8192,
            vram_free_mib: 7000,
            driver_version: "535.00".to_string(),
            pci_vendor_id: 0x10de,
            pci_device_id: 0x2204,
            arch: Some("Ampere".to_string()),
            caps: anvilml_core::types::InferenceCaps {
                fp32: true,
                fp16: true,
                bf16: true,
                fp8: true,
                fp4: false,
                flash_attention: true,
            },
            enumeration_source: anvilml_core::types::EnumerationSource::Mock,
            capabilities_source: anvilml_core::types::CapabilitySource::Fallback,
        }],
        inference_caps: anvilml_core::types::InferenceCaps {
            fp32: true,
            fp16: true,
            bf16: true,
            fp8: true,
            fp4: false,
            flash_attention: true,
        },
    };

    // Wrap the HardwareInfo in Arc<RwLock<>> to match the
    // AppState::new_with_hardware constructor signature.
    let hardware = Arc::new(tokio::sync::RwLock::new(hardware_info));

    // Open an in-memory database pool for the test.
    let pool = open_in_memory().await.unwrap();

    // Construct the ModelStore from the pool.
    let registry = Arc::new(ModelStore::new(pool.clone()).await);

    let node_registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(node_registry.clone()).await;

    let state = AppState::new_with_hardware_no_workers(
        "test-version",
        hardware,
        pool,
        registry,
        Vec::new(),
        node_registry,
        scheduler,
        artifact_store,
    );

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Build a GET request to /v1/system.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/system")
        .body(axum::body::Body::empty())
        .unwrap();

    // Dispatch the request through the router.
    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 status.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the `gpus` key exists and is an array with at least one entry.
    let gpus = json["gpus"]
        .as_array()
        .expect("gpus field must be a JSON array");
    assert!(
        gpus.len() >= 1,
        "gpus array must have at least one entry, got {}",
        gpus.len()
    );

    // Assert the first GPU entry has the expected fields.
    assert_eq!(gpus[0]["name"], "Test GPU");
    assert_eq!(gpus[0]["device_type"], "cuda");
}
