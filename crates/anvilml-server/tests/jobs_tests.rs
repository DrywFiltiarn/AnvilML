//! Integration tests for the `POST /v1/jobs` and `GET /v1/jobs` handlers.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    EnvReport, HardwareInfo, JobSettings, NodeTypeDescriptor, NodeTypeRegistry, ProvisioningState,
    ServerConfig,
};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::{JobStore, ModelStore};
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use axum::http::StatusCode;
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::util::ServiceExt;
use uuid::Uuid;

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
/// fields are not used by the `/health` or `/v1/nodes` handlers, so
/// minimal construction is sufficient. The `node_registry` is shared
/// with the scheduler for consistency.
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
        model_store: Arc::new(ModelStore::new(db)),
    }
}

/// Verify that POST /v1/jobs with a valid graph and populated registry
/// returns 202 Accepted with a job_id (valid UUID) and queue_position 1.
///
/// Constructs an `AppState` with one registered node type, registers a
/// valid graph referencing that type, submits it via `POST /v1/jobs`,
/// and asserts the response status is `StatusCode::ACCEPTED` plus the
/// body contains a valid UUID `job_id` and `queue_position` of 1.
#[tokio::test]
async fn test_submit_job_valid_returns_202() {
    // Register a synthetic node type so the registry is non-empty and the
    // graph validator can resolve the node's type string.
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a valid graph referencing the registered node type.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::ACCEPTED);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // Assert job_id is a valid UUID string (36 characters, contains hyphens).
    let job_id = body["job_id"].as_str().expect("job_id must be a string");
    assert_eq!(job_id.len(), 36, "job_id must be a valid UUID v4 string");

    // Assert queue_position is 1 (first job in an empty queue).
    let queue_position = body["queue_position"]
        .as_u64()
        .expect("queue_position must be a non-negative integer");
    assert_eq!(queue_position, 1);
}

/// Verify that POST /v1/jobs with a malformed JSON body returns 400 Bad Request.
///
/// Sends a request with invalid JSON (`{not valid json}`) to the `/v1/jobs`
/// endpoint and asserts the response status is `StatusCode::BAD_REQUEST`.
/// The axum `Json` extractor returns a `Serde` error for malformed input,
/// which `AnvilError::IntoResponse` maps to HTTP 400.
#[tokio::test]
async fn test_submit_job_malformed_body_returns_400() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Send invalid JSON body.
    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from("{not valid json}"))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
}

/// Verify that POST /v1/jobs with an empty `NodeTypeRegistry` returns 503.
///
/// Constructs an `AppState` with an empty registry (no workers registered),
/// submits a structurally valid graph, and asserts the response status is
/// `StatusCode::SERVICE_UNAVAILABLE`. The scheduler's workers-available
/// guard rejects the submission before validation, returning
/// `AnvilError::WorkersUnavailable` which maps to HTTP 503.
#[tokio::test]
async fn test_submit_job_empty_registry_returns_503() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Submit a structurally valid graph, but the registry is empty so
    // the workers-available guard rejects it.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
}

/// Verify that POST /v1/jobs with an invalid graph (unknown node type)
/// returns 400 Bad Request.
///
/// Constructs an `AppState` with a populated registry, then submits a graph
/// that references a node type not present in the registry. The scheduler's
/// graph validator detects the unknown type and returns
/// `AnvilError::InvalidGraph`, which maps to HTTP 400.
#[tokio::test]
async fn test_submit_job_invalid_graph_returns_400() {
    // Register a node type so the registry is non-empty (avoiding the
    // workers-unavailable path) but the graph references a different,
    // unregistered type.
    let descriptor = NodeTypeDescriptor {
        type_name: "RegisteredNode".to_string(),
        display_name: "Registered Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a graph referencing "UnknownNode" which is NOT in the registry.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "UnknownNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);
}

/// Verify that GET /v1/jobs with no query params returns all submitted jobs.
///
/// Submits a job via POST, then calls GET /v1/jobs with no filters and asserts
/// the response status is `StatusCode::OK` and the returned array is non-empty.
#[tokio::test]
async fn test_list_jobs_no_filter_returns_all() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a job first so the list is non-empty.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    // Now list jobs with no filters.
    let get_req = Request::get("/v1/jobs").body(Body::empty()).unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let jobs = body.as_array().expect("response body must be a JSON array");
    assert!(
        !jobs.is_empty(),
        "list_jobs with no filter should return at least the submitted job"
    );
}

/// Verify that GET /v1/jobs?status=queued returns only jobs matching the filter.
///
/// Submits two jobs, then lists with `status=queued` and asserts only the
/// matching jobs are returned.
#[tokio::test]
async fn test_list_jobs_status_filter() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit two jobs.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    for _ in 0..2 {
        let req = Request::post("/v1/jobs")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);
    }

    // List with status=queued filter — both jobs are queued.
    let get_req = Request::get("/v1/jobs?status=queued")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let jobs = body.as_array().expect("response body must be a JSON array");
    assert_eq!(jobs.len(), 2, "should return exactly 2 queued jobs");
}

/// Verify that GET /v1/jobs?limit=2 returns at most 2 jobs.
///
/// Submits three jobs, then lists with `limit=2` and asserts at most 2
/// jobs are returned.
#[tokio::test]
async fn test_list_jobs_limit() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit three jobs.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    for _ in 0..3 {
        let req = Request::post("/v1/jobs")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::ACCEPTED);
    }

    // List with limit=2.
    let get_req = Request::get("/v1/jobs?limit=2")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let jobs = body.as_array().expect("response body must be a JSON array");
    assert!(
        jobs.len() <= 2,
        "limit=2 should return at most 2 jobs, got {}",
        jobs.len()
    );
}

/// Verify that GET /v1/jobs/:id on an existing job returns 200 with correct data.
///
/// Submits a job, extracts its ID from the POST response, then calls
/// GET /v1/jobs/:id and asserts the response is 200 with the correct job.
#[tokio::test]
async fn test_get_job_existing_returns_200() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a job.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    let body_bytes = to_bytes(post_res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let post_body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let job_id = post_body["job_id"]
        .as_str()
        .expect("job_id must be a string");
    let parsed_id = Uuid::parse_str(job_id).expect("job_id must be a valid UUID");

    // Now GET the job by ID.
    let get_req = Request::get(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    // Assert the returned job has the same ID.
    assert_eq!(
        body["id"].as_str().map(|s| Uuid::parse_str(s).unwrap()),
        Some(parsed_id)
    );
}

/// Verify that GET /v1/jobs/:id on a non-existent UUID returns 404.
///
/// Calls GET /v1/jobs/:id with a UUID that was never submitted and asserts
/// the response status is `StatusCode::NOT_FOUND`.
#[tokio::test]
async fn test_get_job_unknown_returns_404() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Use a random UUID that was never submitted.
    let unknown_id = Uuid::new_v4();
    let get_req = Request::get(&format!("/v1/jobs/{}", unknown_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Verify that GET /v1/jobs accepts the `before` query parameter (forward-compat).
///
/// Submits a job, then lists with `before=<timestamp>` query parameter.
/// The handler accepts the parameter but does not pass it to the persistence
/// layer — this test asserts that the parameter does not cause a 400 error.
#[tokio::test]
async fn test_list_jobs_before_param_accepted() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a job.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    // List with before param — should return 200 (param accepted, ignored by persistence).
    let before_str = "2026-07-08T00:00:00Z";
    let get_req = Request::get(&format!("/v1/jobs?before={}", before_str))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(get_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

/// Verify that POST /v1/jobs/{id} on a Queued job returns 202 Accepted.
///
/// Submits a job (it enters Queued state), then calls POST /v1/jobs/{id}
/// and asserts the response status is `StatusCode::ACCEPTED`. The scheduler's
/// cancel method transitions Queued jobs to Cancelled and returns
/// CancelOutcome::Accepted, which the handler maps to HTTP 202.
#[tokio::test]
async fn test_cancel_queued_job_returns_202() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a job — it enters Queued state.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    let body_bytes = to_bytes(post_res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let post_body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let job_id = post_body["job_id"]
        .as_str()
        .expect("job_id must be a string");

    // Cancel the job — POST /v1/jobs/{id} (same path as GET, different method).
    let cancel_req = Request::post(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(cancel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
}

/// Verify that POST /v1/jobs/{id} on a Completed job returns 409 Conflict.
///
/// Creates a separate AppState with a completed job persisted directly to the
/// database (not via submit, so it is not in the in-memory queue). Calls
/// POST /v1/jobs/{id} and asserts the response status is `StatusCode::CONFLICT`.
/// The scheduler's cancel method returns CancelOutcome::AlreadyTerminal for
/// terminal jobs, which the handler maps to HTTP 409.
#[tokio::test]
async fn test_cancel_completed_job_returns_409() {
    use anvilml_core::Job;
    use chrono::Utc;

    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;

    // Persist a Completed job directly to the database (not via submit, so
    // it is NOT in the in-memory queue). This ensures cancel() skips the
    // queue check and goes straight to the DB, where it sees Completed.
    let job_id = Uuid::new_v4();
    let completed_job = Job {
        id: job_id,
        status: anvilml_core::JobStatus::Completed,
        graph: json!({
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        }),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };

    let job_store = JobStore::new(state.db.clone());
    job_store
        .upsert(&completed_job)
        .await
        .expect("persist must succeed");

    // Build the router with the AppState that now contains the completed job.
    let router = build_router(state);

    // Cancel the job — POST /v1/jobs/{id} should return 409 because it's terminal.
    let cancel_req = Request::post(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(cancel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

/// Verify that POST /v1/jobs/{id} on an unknown UUID returns 404 Not Found.
///
/// Calls cancel with a random UUID that was never submitted and asserts the
/// response status is `StatusCode::NOT_FOUND`. The scheduler's cancel method
/// returns CancelOutcome::NotFound for unknown IDs, which the handler maps
/// to HTTP 404.
#[tokio::test]
async fn test_cancel_unknown_id_returns_404() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Use a random UUID that was never submitted.
    let unknown_id = Uuid::new_v4();
    let cancel_req = Request::post(&format!("/v1/jobs/{}", unknown_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(cancel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

/// Verify that POST /v1/jobs/{id}/cancel on a Running job returns 202 Accepted.
///
/// Submits a job, manually sets its DB status to Running with a worker_id,
/// then calls cancel and asserts the response status is `StatusCode::ACCEPTED`.
/// The scheduler's cancel method sends a CancelJob signal via the transport
/// (best-effort in tests since no real worker is connected) and returns
/// CancelOutcome::Accepted, which the handler maps to HTTP 202.
#[tokio::test]
async fn test_cancel_running_job_returns_202() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state.clone());

    // Submit a job.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    let body_bytes = to_bytes(post_res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let post_body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let job_id = post_body["job_id"]
        .as_str()
        .expect("job_id must be a string");
    let parsed_id = Uuid::parse_str(job_id).expect("job_id must be a valid UUID");

    // Manually update the job status to Running with a worker_id.
    let job_store = JobStore::new(state.db.clone());
    if let Ok(Some(mut job)) = job_store.get(parsed_id).await {
        use anvilml_core::JobStatus;
        job.status = JobStatus::Running;
        job.worker_id = Some("0".to_string());
        let _ = job_store.upsert(&job).await;
    }

    // Cancel the job — POST /v1/jobs/{id} should return 202 because Running is cancellable.
    // The transport send is best-effort (no real worker connected) but the
    // scheduler still returns Accepted because the cancellation was accepted.
    let cancel_req = Request::post(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(cancel_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
}

/// Verify that POST /v1/jobs/{id}/cancel on an already-cancelled job returns 409 Conflict.
///
/// Submits a job, cancels it once (returns 202), then cancels again and asserts
/// the response status is `StatusCode::CONFLICT`. This tests the idempotent-cancel
/// principle: cancelling an already-cancelled job is a no-op that returns 409.
#[tokio::test]
async fn test_cancel_already_cancelled_job_returns_409() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a job.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    let body_bytes = to_bytes(post_res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let post_body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let job_id = post_body["job_id"]
        .as_str()
        .expect("job_id must be a string");

    // First cancel — should return 202 (queued job).
    let cancel_req1 = Request::post(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res1 = router.clone().oneshot(cancel_req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::ACCEPTED);

    // Second cancel — should return 409 (already cancelled).
    let cancel_req2 = Request::post(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res2 = router.oneshot(cancel_req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::CONFLICT);
}

/// Verify that DELETE /v1/jobs/:id on a Completed job returns 204 and removes
/// the job from the database.
///
/// Submits a job, manually sets its status to Completed via direct DB access
/// (not via submit, so it is not in the in-memory queue), then calls DELETE
/// and asserts the response status is `StatusCode::NO_CONTENT`. After deletion,
/// verifies the job is no longer retrievable via `JobStore::get`.
#[tokio::test]
async fn test_delete_terminal_job_returns_204() {
    use anvilml_core::Job;
    use chrono::Utc;

    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;

    // Persist a Completed job directly to the database (not via submit, so
    // it is NOT in the in-memory queue). This ensures the delete handler
    // goes straight to the DB where it sees Completed.
    let job_id = Uuid::new_v4();
    let completed_job = Job {
        id: job_id,
        status: anvilml_core::JobStatus::Completed,
        graph: json!({
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        }),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };

    let job_store = JobStore::new(state.db.clone());
    job_store
        .upsert(&completed_job)
        .await
        .expect("persist must succeed");

    // Clone state before passing to build_router so we can still access
    // state.db after the router consumes the original AppState.
    let router = build_router(state.clone());

    // DELETE the terminal job — should return 204.
    let delete_req = Request::delete(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(delete_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Verify the job is no longer in the database.
    let job_store_after = JobStore::new(state.db.clone());
    let remaining = job_store_after
        .get(job_id)
        .await
        .expect("query must succeed");
    assert!(
        remaining.is_none(),
        "job should be deleted from the database"
    );
}

/// Verify that DELETE /v1/jobs/:id on a Completed job with associated artifacts
/// also deletes the artifact file and DB row.
///
/// Submits a job, manually sets its status to Completed, persists a fake artifact
/// row and file via `artifact_store.save()`, then calls DELETE and asserts 204.
/// After deletion, verifies the artifact is no longer in the list for that job.
#[tokio::test]
async fn test_delete_terminal_job_removes_artifacts() {
    use anvilml_core::{ArtifactMeta, Job};
    use chrono::Utc;

    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let artifact_store = state.artifact_store.clone();

    // Persist a Completed job directly to the database.
    let job_id = Uuid::new_v4();
    let completed_job = Job {
        id: job_id,
        status: anvilml_core::JobStatus::Completed,
        graph: json!({
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        }),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };

    let job_store = JobStore::new(state.db.clone());
    job_store
        .upsert(&completed_job)
        .await
        .expect("persist must succeed");

    // Create a fake PNG file and persist it via artifact_store.save().
    // A minimal valid PNG is 67 bytes: 8-byte signature + IHDR chunk + IDAT chunk + IEND chunk.
    // We use a known 4x4 red pixel PNG (the smallest valid PNG).
    let fake_png: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR length + "IHDR"
        0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x04, // 4x4
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB
        0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, // IDAT
        0x54, 0x08, 0xd7, 0x63, 0xf8, 0xff, 0xff, 0x3f, // compressed data
        0x00, 0x05, 0xfe, 0x02, 0xfe, 0xdc, 0xcc, 0x59, // more data
        0xe7, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, // IEND
        0x44, 0xae, 0x42, 0x60, 0x82, // IEND chunk
    ];

    let meta = ArtifactMeta {
        hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        job_id,
        width: 4,
        height: 4,
        seed: 42,
        steps: 1,
        created_at: Utc::now(),
        file_path: std::path::PathBuf::from("/tmp/fake.png"),
    };

    // Save the artifact — this writes the PNG file and persists the DB row.
    artifact_store
        .save(fake_png, &meta)
        .await
        .expect("artifact save must succeed");

    // Verify the artifact was persisted.
    let before_list = artifact_store
        .list(Some(job_id))
        .await
        .expect("list must succeed");
    assert_eq!(
        before_list.len(),
        1,
        "should have exactly 1 artifact before deletion"
    );

    // Clone state before passing to build_router so we can still access
    // state.artifact_store after the router consumes the original AppState.
    let router = build_router(state.clone());

    // DELETE the job — should return 204 and remove both file and DB row.
    let delete_req = Request::delete(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(delete_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // Verify the artifact is gone.
    let after_list = artifact_store
        .list(Some(job_id))
        .await
        .expect("list must succeed");
    assert!(
        after_list.is_empty(),
        "artifact should be deleted from the database"
    );

    // Also verify the file was removed from disk.
    // The artifact file path is {artifact_dir}/{hash}.png.
    // We use the same artifact_dir that was configured in make_test_state.
    let artifact_dir = state.artifact_store.artifact_dir();
    let file_path = artifact_dir.join(format!("{}.png", meta.hash));
    assert!(
        !file_path.exists(),
        "artifact file should be removed from disk: {}",
        file_path.display()
    );
}

/// Verify that DELETE /v1/jobs/:id on a Queued job returns 409 Conflict.
///
/// Submits a job (it enters Queued state), then calls DELETE and asserts
/// the response status is `StatusCode::CONFLICT`. The handler rejects
/// deletion of non-terminal jobs to prevent accidental data loss.
#[tokio::test]
async fn test_delete_non_terminal_queued_returns_409() {
    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;
    let router = build_router(state);

    // Submit a job — it enters Queued state.
    let body = json!({
        "graph": {
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        },
        "settings": { "device_preference": null }
    });

    let post_req = Request::post("/v1/jobs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::ACCEPTED);

    let body_bytes = to_bytes(post_res.into_body(), usize::MAX)
        .await
        .expect("body collection must succeed");
    let post_body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("response body must be valid JSON");

    let job_id = post_body["job_id"]
        .as_str()
        .expect("job_id must be a string");

    // DELETE the queued job — should return 409.
    let delete_req = Request::delete(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(delete_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

/// Verify that DELETE /v1/jobs/:id on a Running job returns 409 Conflict.
///
/// Submits a job, manually sets its status to Running via direct DB access,
/// then calls DELETE and asserts the response status is `StatusCode::CONFLICT`.
#[tokio::test]
async fn test_delete_non_terminal_running_returns_409() {
    use anvilml_core::Job;
    use chrono::Utc;

    let descriptor = NodeTypeDescriptor {
        type_name: "TestNode".to_string(),
        display_name: "Test Node".to_string(),
        category: "test".to_string(),
        description: "A synthetic test node.".to_string(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };

    let node_registry = Arc::new(NodeTypeRegistry::new());
    node_registry.register_all(vec![descriptor]);

    let state = make_test_state(node_registry).await;

    // Persist a Running job directly to the database (not via submit).
    let job_id = Uuid::new_v4();
    let running_job = Job {
        id: job_id,
        status: anvilml_core::JobStatus::Running,
        graph: json!({
            "nodes": [
                { "id": "node1", "type": "TestNode", "inputs": {}, "outputs": {} }
            ]
        }),
        settings: JobSettings {
            device_preference: None,
        },
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };

    let job_store = JobStore::new(state.db.clone());
    job_store
        .upsert(&running_job)
        .await
        .expect("persist must succeed");

    let router = build_router(state);

    // DELETE the running job — should return 409.
    let delete_req = Request::delete(&format!("/v1/jobs/{}", job_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(delete_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

/// Verify that DELETE /v1/jobs/:id on an unknown UUID returns 404 Not Found.
///
/// Calls DELETE with a random UUID that was never submitted and asserts the
/// response status is `StatusCode::NOT_FOUND`. The handler returns
/// `AnvilError::JobNotFound` which maps to HTTP 404.
#[tokio::test]
async fn test_delete_unknown_id_returns_404() {
    let state = make_test_state(Arc::new(NodeTypeRegistry::new())).await;
    let router = build_router(state);

    // Use a random UUID that was never submitted.
    let unknown_id = Uuid::new_v4();
    let delete_req = Request::delete(&format!("/v1/jobs/{}", unknown_id))
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(delete_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
