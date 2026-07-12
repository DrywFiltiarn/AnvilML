//! Integration tests for the system information handlers.
//!
//! Tests verify that `GET /v1/system` returns the hardware snapshot,
//! `GET /v1/system/env` returns the Python environment report, and
//! `GET /v1/system/versions` returns per-component version info,
//! all as JSON bodies with correct status codes and field values.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{EnvReport, HardwareInfo, NodeTypeRegistry, ProvisioningState, ServerConfig};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::{JobStore, ModelStore};
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::util::ServiceExt;

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
/// Creates stub values for `scheduler`, `workers`, and `db` — these
/// fields are not used by the system handlers, so minimal construction
/// is sufficient. The `hardware` and `env_report` fields are populated
/// with sentinel values that tests assert on.
async fn make_test_state(node_registry: Arc<NodeTypeRegistry>) -> AppState {
    let db = make_test_pool().await;
    let job_store = JobStore::new(db.clone());

    let artifact_store = Arc::new(ArtifactStore::new(
        std::env::temp_dir().join("anvilml-test-artifacts"),
        db.clone(),
    ));

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

/// Verify that GET /v1/system returns 200 OK with a JSON body containing
/// `HardwareInfo` fields matching the sentinel values set in the test state.
///
/// Constructs a `GET /v1/system` request, sends it through the router
/// built by `build_router()` with a captured `AppState`, and asserts
/// the response status is `StatusCode::OK` plus the JSON body contains
/// `host.hostname == "test-host"` and an empty `gpus` array.
#[tokio::test]
async fn test_get_system_returns_200() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/v1/system").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    // Parse response body and assert on the hardware snapshot fields.
    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // The hostname must match the sentinel value set in make_test_state.
    assert_eq!(body["host"]["hostname"], "test-host");
    assert_eq!(body["host"]["os"], "Linux");
    // The gpus array must be empty (no GPUs detected in test state).
    assert_eq!(body["gpus"].as_array().map(|a| a.len()), Some(0));
}

/// Verify that GET /v1/system reflects hardware updates written through
/// the `RwLock` write lock between requests.
///
/// Constructs a `GET /v1/system` request, then acquires a write lock
/// on the `hardware` field to update the hostname, sends a second
/// request, and asserts the new hostname appears in the response.
#[tokio::test]
async fn test_get_system_reflects_hardware_update() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let state = make_test_state(node_registry).await;
    let router = build_router(state.clone());

    // First request — should return the original sentinel hostname.
    let req = Request::get("/v1/system").body(Body::empty()).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");
    assert_eq!(body["host"]["hostname"], "test-host");

    // Update the hardware snapshot through the write lock.
    // This simulates a VRAM refresh or hardware re-detection
    // that would occur in production between requests.
    let mut hw = state.hardware.write().await;
    hw.host.hostname = "updated-host".to_string();
    hw.host.os = "Windows".to_string();
    // Write lock is released here, allowing the next read lock.
    drop(hw);

    // Second request — should return the updated hostname.
    let req2 = Request::get("/v1/system").body(Body::empty()).unwrap();
    let res2 = router.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), axum::http::StatusCode::OK);

    let body_bytes2 = to_bytes(res2.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body2: Value =
        serde_json::from_slice(&body_bytes2).expect("response body must be valid JSON");
    assert_eq!(body2["host"]["hostname"], "updated-host");
    assert_eq!(body2["host"]["os"], "Windows");
}

/// Verify that GET /v1/system/env returns 200 OK with a JSON body
/// containing `EnvReport` fields matching the sentinel values.
///
/// Constructs a `GET /v1/system/env` request, sends it through the
/// router built by `build_router()`, and asserts the response status
/// is `StatusCode::OK` plus the JSON body contains `python_path`
/// and `preflight_ok == false` from the test state.
#[tokio::test]
async fn test_get_system_env_returns_200() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/v1/system/env").body(Body::empty()).unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    // Parse response body and assert on the environment report fields.
    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // The python_path must match the sentinel value set in make_test_state.
    assert_eq!(body["python_path"], "./worker/.venv/bin/python3");
    // preflight_ok must be false (best-effort, no full preflight at startup).
    assert_eq!(body["preflight_ok"], false);
}

/// Verify that GET /v1/system/env reflects environment report updates
/// written through the `RwLock` write lock between requests.
///
/// Constructs a `GET /v1/system/env` request, then acquires a write
/// lock on the `env_report` field to update the python_version and
/// provisioning state, sends a second request, and asserts the updated
/// values appear in the response.
#[tokio::test]
async fn test_get_system_env_reflects_env_report_update() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let state = make_test_state(node_registry).await;
    let router = build_router(state.clone());

    // First request — should return the original sentinel values.
    let req = Request::get("/v1/system/env").body(Body::empty()).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");
    assert_eq!(body["python_version"], Value::Null);
    assert_eq!(body["provisioning"], "not_started");

    // Update the env_report through the write lock.
    // This simulates the preflight subsystem updating the report
    // after completing its checks.
    let mut report = state.env_report.write().await;
    report.python_version = Some("3.12.3".to_string());
    report.torch_version = Some("2.5.0".to_string());
    report.provisioning = ProvisioningState::Ready;
    report.preflight_ok = true;
    // Write lock is released here, allowing the next read lock.
    drop(report);

    // Second request — should return the updated values.
    let req2 = Request::get("/v1/system/env").body(Body::empty()).unwrap();
    let res2 = router.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), axum::http::StatusCode::OK);

    let body_bytes2 = to_bytes(res2.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body2: Value =
        serde_json::from_slice(&body_bytes2).expect("response body must be valid JSON");
    assert_eq!(body2["python_version"], "3.12.3");
    assert_eq!(body2["torch_version"], "2.5.0");
    assert_eq!(body2["provisioning"], "ready");
    assert_eq!(body2["preflight_ok"], true);
}

/// Verify that GET /v1/system/versions returns 200 OK with a JSON body
/// containing non-empty `anvilml_version` and `rust_version` fields.
///
/// Constructs a `GET /v1/system/versions` request, sends it through the
/// router built by `build_router()`, and asserts the response status is
/// `StatusCode::OK` plus both version strings are present and non-empty.
/// `python_version` and `torch_version` are `null` (the `make_test_state`
/// default).
#[tokio::test]
async fn test_get_system_versions_returns_200() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/v1/system/versions")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    // Parse response body and assert on the version fields.
    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // anvilml_version must be present and non-empty (from CARGO_PKG_VERSION).
    let anvilml_ver = body["anvilml_version"]
        .as_str()
        .expect("anvilml_version must be a string");
    assert!(!anvilml_ver.is_empty(), "anvilml_version must not be empty");

    // rust_version must be present and non-empty (from rustc_version_runtime).
    let rust_ver = body["rust_version"]
        .as_str()
        .expect("rust_version must be a string");
    assert!(!rust_ver.is_empty(), "rust_version must not be empty");

    // python_version and torch_version are null in make_test_state defaults.
    assert_eq!(body["python_version"], Value::Null);
    assert_eq!(body["torch_version"], Value::Null);
}

/// Verify that GET /v1/system/versions reflects env_report values set
/// in the test state — specifically `python_version` and `torch_version`.
///
/// Constructs state with `python_version = Some("3.12.3")` and
/// `torch_version = Some("2.5.0")`, sends a request, and asserts both
/// fields match in the JSON response.
#[tokio::test]
async fn test_get_system_versions_reflects_env_report_values() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let state = make_test_state(node_registry).await;
    let router = build_router(state.clone());

    // Update the env_report with sentinel version strings.
    let mut report = state.env_report.write().await;
    report.python_version = Some("3.12.3".to_string());
    report.torch_version = Some("2.5.0".to_string());
    // Write lock is released here.
    drop(report);

    let req = Request::get("/v1/system/versions")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // The python_version and torch_version must match the sentinel values.
    assert_eq!(body["python_version"], "3.12.3");
    assert_eq!(body["torch_version"], "2.5.0");
}

/// Verify that GET /v1/system/versions returns null for python_version
/// and torch_version when the env_report has `None` for those fields
/// (the default in `make_test_state`).
///
/// Constructs state without modifying env_report (so both fields are
/// `None`), sends a request, and asserts both are `null` in the JSON.
#[tokio::test]
async fn test_get_system_versions_null_when_env_report_unset() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);
    let req = Request::get("/v1/system/versions")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // Both fields must be null when the env_report has None for them.
    assert_eq!(body["python_version"], Value::Null);
    assert_eq!(body["torch_version"], Value::Null);
}
