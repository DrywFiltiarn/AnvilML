//! Integration tests for the Python preflight check.
//!
//! Verifies that:
//! - The `/v1/system/env` endpoint returns the correct shape.
//! - Job submission is rejected with 503 when preflight has failed.
//! - In mock mode, job submission proceeds normally.

use std::sync::Arc;

use anvilml_core::EnvReport;
use anvilml_server::artifact::store::ArtifactStore;
use anvilml_server::ws::broadcaster::EventBroadcaster;
use anvilml_server::{build_router, App};
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use http_body_util::Full;
use serde_json::Value;
use tower::ServiceExt;

/// Build a minimal test app.
async fn make_test_app() -> (App, Arc<EventBroadcaster>) {
    let tmp = tempfile::tempdir().unwrap();
    let artifact_store = ArtifactStore::new(
        tmp.path().to_path_buf(),
        anvilml_registry::open_in_memory().await.unwrap(),
    );
    let broadcaster = Arc::new(EventBroadcaster::new(16));
    let state = App::new(
        "0.1.0",
        None,
        None,
        None,
        broadcaster.clone(),
        None,
        None,
        artifact_store,
        anvilml_core::ServerConfig::default(),
    );
    (state, broadcaster)
}

/// When preflight fails, the env endpoint reflects the failure.
///
/// Sets up an App with a simulated failed preflight, then verifies
/// the `GET /v1/system/env` endpoint returns `preflight_ok: false`.
#[tokio::test]
async fn env_endpoint_reflects_failed_preflight() {
    let (state, _broadcaster) = make_test_app().await;

    // Simulate a failed preflight.
    let failed_report = EnvReport {
        python_path: "/nonexistent/venv/bin/python3".to_string(),
        python_version: String::new(),
        torch_version: String::new(),
        preflight_ok: false,
        reason: "python_missing".to_string(),
    };
    state.set_env_report(failed_report);

    let app = build_router(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/system/env")
                .body(Full::<Bytes>::default())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();

    assert_eq!(parsed["preflight_ok"], false);
    assert_eq!(parsed["reason"], "python_missing");
}

/// The env endpoint returns correct shape with stub values when no
/// preflight has run (test context).
#[tokio::test]
async fn env_returns_correct_shape_in_stub_context() {
    let (state, _broadcaster) = make_test_app().await;
    let app = build_router(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/system/env")
                .body(Full::<Bytes>::default())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();

    // Verify all expected fields are present.
    assert!(parsed["python_path"].is_string());
    assert!(parsed["python_version"].is_string());
    assert!(parsed["torch_version"].is_string());
    assert!(parsed["preflight_ok"].is_boolean());
    assert!(parsed["reason"].is_string());
}

/// Job submission returns 503 when preflight has failed (non-mock mode).
///
/// Simulates a failed preflight, then attempts job submission. The
/// handler should reject with 503 `workers_unavailable`.
#[serial_test::serial]
#[tokio::test]
async fn job_submit_rejected_when_preflight_fails() {
    // Ensure mock mode is NOT set for this test.
    std::env::remove_var("ANVILML_WORKER_MOCK");

    let tmp = tempfile::tempdir().unwrap();
    let pool = anvilml_registry::open_in_memory().await.unwrap();
    let artifact_store = ArtifactStore::new(tmp.path().to_path_buf(), pool.clone());
    let broadcaster = Arc::new(EventBroadcaster::new(16));

    // Create a scheduler so the preflight gate is the first check.
    use anvilml_scheduler::{JobQueue, JobScheduler, VramLedger};
    use anvilml_worker::WorkerPool;
    use tokio::sync::broadcast;

    let workers = Arc::new(WorkerPool::new_test_pool());
    let (bcast, _rx) = broadcast::channel::<anvilml_core::types::events::WsEvent>(16);
    let scheduler = Arc::new(JobScheduler::new(
        JobQueue::new(),
        workers,
        pool.clone(),
        bcast,
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        "auto".to_string(),
        artifact_store.clone(),
    ));

    let state = App::new(
        "0.1.0",
        None,
        None,
        None,
        broadcaster.clone(),
        None,
        Some(scheduler),
        artifact_store,
        anvilml_core::ServerConfig::default(),
    );

    // Simulate a failed preflight.
    let failed_report = EnvReport {
        python_path: "/nonexistent/venv/bin/python3".to_string(),
        python_version: String::new(),
        torch_version: String::new(),
        preflight_ok: false,
        reason: "python_missing".to_string(),
    };
    state.set_env_report(failed_report);

    let app = build_router(state);

    // Attempt job submission — should be rejected.
    let req_body = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "n0", "type": "NopeNode", "inputs": {}}
            ],
            "edges": []
        },
        "settings": {}
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/jobs")
                .header("content-type", "application/json")
                .body(Full::<Bytes>::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "expected 503 when preflight fails"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();

    assert_eq!(parsed["error"], "workers_unavailable");
    assert!(
        parsed["message"]
            .as_str()
            .unwrap()
            .contains("python preflight failed"),
        "message should mention preflight failure: {}",
        parsed["message"]
    );
}

/// Job submission proceeds normally in mock mode even when preflight
/// report indicates failure.
///
/// Simulates a failed preflight, sets `ANVILML_WORKER_MOCK=1`, then
/// attempts job submission. The handler should bypass the preflight
/// gate and proceed (the graph validation will reject the bad graph).
#[serial_test::serial]
#[tokio::test]
async fn job_submit_proceeds_in_mock_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let pool = anvilml_registry::open_in_memory().await.unwrap();
    let artifact_store = ArtifactStore::new(tmp.path().to_path_buf(), pool.clone());
    let broadcaster = Arc::new(EventBroadcaster::new(16));

    // Create a scheduler so the preflight gate is the first check.
    use anvilml_scheduler::{JobQueue, JobScheduler, VramLedger};
    use anvilml_worker::WorkerPool;
    use tokio::sync::broadcast;

    let workers = Arc::new(WorkerPool::new_test_pool());
    let (bcast, _rx) = broadcast::channel::<anvilml_core::types::events::WsEvent>(16);
    let scheduler = Arc::new(JobScheduler::new(
        JobQueue::new(),
        workers,
        pool.clone(),
        bcast,
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        "auto".to_string(),
        artifact_store.clone(),
    ));

    let state = App::new(
        "0.1.0",
        None,
        None,
        None,
        broadcaster.clone(),
        None,
        Some(scheduler),
        artifact_store,
        anvilml_core::ServerConfig::default(),
    );

    // Simulate a failed preflight.
    let failed_report = EnvReport {
        python_path: "/nonexistent/venv/bin/python3".to_string(),
        python_version: String::new(),
        torch_version: String::new(),
        preflight_ok: false,
        reason: "python_missing".to_string(),
    };
    state.set_env_report(failed_report);

    // Set mock mode at the process level before the request.
    std::env::set_var("ANVILML_WORKER_MOCK", "1");
    let orig_mock = std::env::var("ANVILML_WORKER_MOCK").ok();

    let app = build_router(state);

    // Submit a bad graph — should proceed past preflight gate
    // and reach graph validation.
    let req_body = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "n0", "type": "NopeNode", "inputs": {}}
            ],
            "edges": []
        },
        "settings": {}
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/jobs")
                .header("content-type", "application/json")
                .body(Full::<Bytes>::from(serde_json::to_vec(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // In mock mode, preflight gate is bypassed, so we should
    // get past the preflight check. The bad graph will be
    // rejected by graph validation (422), not by preflight (503).
    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "expected 422 (graph validation), not 503 (preflight gate)"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();
    assert_eq!(parsed["error"], "invalid_graph");

    // Restore original env state.
    if let Some(orig) = orig_mock {
        std::env::set_var("ANVILML_WORKER_MOCK", orig);
    } else {
        std::env::remove_var("ANVILML_WORKER_MOCK");
    }
}
