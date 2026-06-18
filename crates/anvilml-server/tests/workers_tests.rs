//! Integration tests for the workers HTTP handler.
//!
//! Tests cover: empty workers list when no pool is configured, and
//! worker info returned when a pool with a mock worker is present.

use anvilml_core::WorkerStatus;
use anvilml_ipc::{EventBroadcaster, RouterTransport};
use anvilml_server::{build_router, AppState};
use anvilml_worker::{ManagedWorker, WorkerPool};
use axum::body::to_bytes;
use axum::http::{Method, Request};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tower::util::ServiceExt;

/// Create a minimal WorkerPool with one mock worker for tests.
///
/// The mock worker is created with `WorkerStatus::Idle` so that
/// `GET /v1/workers` returns a valid WorkerInfo entry.
/// The transport is bound to port 0 (OS-assigned) to avoid conflicts.
fn mock_pool_with_one_worker() -> WorkerPool {
    // Bind a transport with OS-assigned port to avoid conflicts.
    let transport = futures::executor::block_on(async {
        RouterTransport::bind().await.expect("bind mock transport")
    });

    // Create a fresh broadcaster for the mock pool.
    let broadcaster = Arc::new(EventBroadcaster::new());

    // Create a mock ManagedWorker in Idle status.
    // The channels and handles are dummy values — they are never
    // used because the mock worker is only queried for status.
    let (msg_tx, _msg_rx) = mpsc::channel(16);
    let (event_tx, _event_rx) = broadcast::channel(16);

    let mock_worker = ManagedWorker::new(
        WorkerStatus::Idle,
        msg_tx,
        event_tx,
        None, // child — no subprocess for mock
        None, // bridge_handles
        None, // keepalive_handle
        None, // heartbeat_handle
        "worker-0".to_string(),
        "mock-device".to_string(),
        0,    // device_index
        None, // routes — no real demux task in this test
        None, // route_key
    );

    // Build the pool with one mock worker.
    WorkerPool::new(
        vec![(
            mock_worker.get_status(),
            "worker-0".to_string(),
            "mock-device".to_string(),
        )],
        Arc::new(transport),
        broadcaster,
    )
}

/// Verify that GET /v1/workers returns an empty JSON array when
/// AppState.workers is None (test/stub mode).
///
/// Uses `AppState::new()` which sets workers to None. The handler
/// should return `[]` without panicking or erroring.
#[tokio::test]
async fn test_list_workers_returns_empty_when_no_pool() {
    let state = AppState::new("test-version").await;

    // Build the router via the production `build_router` function.
    let router = build_router(state);

    // Dispatch a GET request to /v1/workers.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/workers")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 status.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the body is an empty JSON array.
    assert!(
        json.is_array(),
        "GET /v1/workers must return a JSON array, got: {}",
        json
    );
    assert_eq!(json.as_array().unwrap().len(), 0);
}

/// Verify that GET /v1/workers returns worker info when the pool
/// contains workers.
///
/// Creates a `WorkerPool` with one mock `ManagedWorker` in `Idle`
/// status, builds the router with `AppState::new_with_hardware()`,
/// and verifies that the handler returns a JSON array with one entry
/// containing `status: "idle"`.
#[tokio::test]
async fn test_list_workers_returns_pool_data() {
    let pool = mock_pool_with_one_worker();

    // Build AppState with the mock worker pool.
    // Using new_with_hardware to inject the workers pool.
    let hardware = Arc::new(tokio::sync::RwLock::new(
        anvilml_core::types::HardwareInfo::default(),
    ));
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
    );

    let router = build_router(state);

    // Dispatch a GET request to /v1/workers.
    let request = Request::builder()
        .method(Method::GET)
        .uri("/v1/workers")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    // Assert HTTP 200 status.
    assert_eq!(response.status(), 200);

    // Read and parse the response body as JSON.
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Assert the body is a JSON array with one entry.
    assert!(
        json.is_array(),
        "GET /v1/workers must return a JSON array, got: {}",
        json
    );
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1, "should return exactly 1 worker");

    // Assert the worker has status "idle".
    assert_eq!(arr[0]["status"], "idle");

    // Assert the worker has the expected identity.
    assert_eq!(arr[0]["id"], "worker-0");
}
