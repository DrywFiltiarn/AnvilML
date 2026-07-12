//! Integration tests for the `GET /v1/workers` and
//! `POST /v1/workers/{id}/restart` handlers.
//!
//! Tests use the crate's public API (`build_router()`) to make
//! in-process HTTP requests without opening a real socket.

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::types::hardware::{CapabilitySource, EnumerationSource};
use anvilml_core::types::worker::WorkerStatus;
use anvilml_core::{
    AnvilError, DeviceType, EnvReport, GpuDevice, HardwareInfo, InferenceCaps, NodeTypeRegistry,
    ProvisioningState, ServerConfig,
};
use anvilml_ipc::{EventBroadcaster, RouterTransport, WorkerEvent};
use anvilml_registry::{JobStore, ModelStore};
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::{WorkerHandle, WorkerPool, WorkerSpawner};
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle as TokioJoinHandle;
use tower::util::ServiceExt;
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::{DealerSocket, SocketOptions, ZmqMessage};

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

/// Spawns a real, long-lived, harmless process (`sleep 999` / `cmd timeout
/// 999`) — matching `crates/anvilml-worker/tests/pool_tests.rs`'s own
/// established `MockWorkerSpawner` pattern, needed because
/// `WorkerPool::spawn_all()`'s real path always constructs a
/// `ProcessWorkerSpawner`, which launches a real Python interpreter from a
/// real virtualenv — nothing a test environment has.
/// `spawn_all_with_spawner()` (`test-utils`-gated) is what the restart
/// tests below actually call, since `restart_worker()` needs a pool whose
/// `spawn_config` is populated (only `spawn_all()`/`spawn_all_with_spawner()`
/// do that) — unlike `set_up_test_workers()`, which injects handles
/// directly and leaves `spawn_config` unset.
struct MockWorkerSpawner {
    call_count: AtomicUsize,
}

impl MockWorkerSpawner {
    fn new() -> Self {
        Self {
            call_count: AtomicUsize::new(0),
        }
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl WorkerSpawner for MockWorkerSpawner {
    fn spawn<'a>(
        &'a self,
        _venv_path: &'a Path,
        _env: HashMap<String, String>,
    ) -> Pin<Box<dyn Future<Output = Result<tokio::process::Child, AnvilError>> + Send + 'a>> {
        Box::pin(async move {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            #[cfg(unix)]
            let mut cmd = {
                let mut c = tokio::process::Command::new("sleep");
                c.arg("999");
                c
            };
            #[cfg(windows)]
            let mut cmd = {
                let mut c = tokio::process::Command::new("cmd");
                c.args(["/c", "timeout", "999"]);
                c
            };

            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            cmd.spawn().map_err(AnvilError::Io)
        })
    }
}

/// Connect a DEALER socket to a `RouterTransport`'s bound endpoint —
/// matching `pool_tests.rs`'s own established `connect_dealer` helper.
async fn connect_dealer(transport: &RouterTransport, worker_id: &str) -> DealerSocket {
    let mut opts = SocketOptions::default();
    opts.peer_identity(
        PeerIdentity::try_from(bytes::Bytes::from(worker_id.to_string())).expect("valid identity"),
    );
    let mut dealer = DealerSocket::with_options(opts);
    let endpoint = format!("tcp://127.0.0.1:{}", transport.port);
    dealer
        .connect(&endpoint)
        .await
        .expect("DEALER connect to ROUTER should succeed");
    tokio::time::sleep(Duration::from_millis(50)).await;
    dealer
}

async fn send_event(dealer: &mut DealerSocket, event: &WorkerEvent) {
    let payload = rmp_serde::to_vec_named(event).expect("event should serialize");
    let mut msg = ZmqMessage::from(bytes::Bytes::from(""));
    msg.push_back(bytes::Bytes::from(payload));
    dealer.send(msg).await.expect("DEALER send should succeed");
}

fn ready_event(worker_id: &str) -> WorkerEvent {
    WorkerEvent::Ready {
        worker_id: worker_id.to_string(),
        device_index: 0,
        device_name: "Mock GPU".to_string(),
        device_type: "cpu".to_string(),
        vram_total_mib: 1024,
        vram_free_mib: 900,
        torch_version: "2.5.0".to_string(),
        fp16: true,
        bf16: true,
        fp8: false,
        flash_attention: false,
        capabilities_source: "mock".to_string(),
        node_types: vec![],
    }
}

/// Construct an `AppState` whose `WorkerPool` was populated via the real
/// `spawn_all_with_spawner()` path (not `set_up_test_workers()`), so
/// `spawn_config` is populated and `restart_worker()` can actually spawn a
/// replacement. Returns the `MockWorkerSpawner` alongside so tests can poll
/// `call_count()` to confirm a new generation was spawned.
async fn make_test_state_with_spawned_workers(
    node_registry: Arc<NodeTypeRegistry>,
    devices: &[GpuDevice],
) -> (AppState, Arc<MockWorkerSpawner>) {
    let db = make_test_pool().await;
    let job_store = JobStore::new(db.clone());

    let artifact_store = Arc::new(ArtifactStore::new(
        std::env::temp_dir().join("anvilml-test-artifacts"),
        db.clone(),
    ));

    let mut pool = WorkerPool::new()
        .await
        .expect("WorkerPool::new() must succeed in test");
    let spawner = Arc::new(MockWorkerSpawner::new());
    let cfg = ServerConfig::default();
    pool.spawn_all_with_spawner(
        devices,
        &cfg,
        Arc::clone(&spawner) as Arc<dyn WorkerSpawner>,
        Arc::clone(&node_registry),
    )
    .await
    .expect("spawn_all_with_spawner() should succeed");
    let workers = Arc::new(pool);

    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        artifact_store.clone(),
        Arc::clone(&workers).transport().clone(),
    ));

    let state = AppState {
        config: Arc::new(cfg),
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
    };

    (state, spawner)
}

/// Poll `spawner.call_count()` until it reaches at least `n`, bounded by
/// `timeout` — `spawn_all_with_spawner()`/`spawn_worker()` only schedule
/// the spawn via `tokio::spawn()`, they don't wait for the spawner itself
/// to actually run.
async fn wait_for_spawn_calls(spawner: &MockWorkerSpawner, n: usize, timeout: Duration) {
    tokio::time::timeout(timeout, async {
        loop {
            if spawner.call_count() >= n {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("expected >= {n} spawn() calls within {timeout:?}"));
}

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

/// Verify that `POST /v1/workers/{id}/restart` returns `404 Not Found`
/// when no worker with the given id exists in the pool.
#[tokio::test]
async fn test_restart_unknown_worker_returns_404() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let devices = vec![make_test_device(0, DeviceType::Cpu)];
    let (state, _spawner) = make_test_state_with_spawned_workers(node_registry, &devices).await;

    let router = build_router(state);
    let req = Request::post("/v1/workers/99/restart")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::NOT_FOUND);
}

/// Verify that restarting a known, non-`Dying` worker returns `202
/// Accepted` and actually spawns a brand-new generation — observed via a
/// second `MockWorkerSpawner::spawn()` call, not just the HTTP status.
///
/// This is the acceptance test for the audit finding P18-D3 closes:
/// `request_shutdown()` alone does not respawn a worker, so this confirms
/// the handler does the full shutdown-then-spawn sequence, not just the
/// first half.
#[tokio::test]
async fn test_restart_known_worker_returns_202_and_spawns_new_generation() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let devices = vec![make_test_device(0, DeviceType::Cpu)];
    let (state, spawner) = make_test_state_with_spawned_workers(node_registry, &devices).await;

    // Wait for the pool's initial spawn to actually invoke spawner.spawn()
    // before restarting — otherwise a call_count() of 1 after restart
    // would be ambiguous (initial spawn vs. restart's own spawn).
    wait_for_spawn_calls(&spawner, 1, Duration::from_secs(2)).await;

    let router = build_router(state);
    let req = Request::post("/v1/workers/0/restart")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::ACCEPTED);

    // A second spawn() call proves a genuinely new generation was spawned,
    // not merely that the old one was left running.
    wait_for_spawn_calls(&spawner, 2, Duration::from_secs(2)).await;
}

/// Verify that restarting an already-`Dying` worker returns `409 Conflict`
/// rather than starting a second, overlapping shutdown-then-spawn
/// sequence.
///
/// Forces the worker into `Dying` directly via `set_status()` on a cloned
/// handle — clones share the same underlying status lock as the pool's
/// own handle (see `WorkerHandle::clone()`'s own doc comment), so this
/// reliably simulates "a shutdown is already in flight" (e.g. from
/// `shutdown_all()`) without needing a real in-progress shutdown race.
#[tokio::test]
async fn test_restart_already_dying_returns_409() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let devices = vec![make_test_device(0, DeviceType::Cpu)];
    let (state, spawner) = make_test_state_with_spawned_workers(node_registry, &devices).await;
    wait_for_spawn_calls(&spawner, 1, Duration::from_secs(2)).await;

    let handles = state.workers.handles();
    let handle = handles
        .iter()
        .find(|h| h.worker_id == "0")
        .expect("worker 0 should exist");
    handle.set_status(WorkerStatus::Dying).await;

    let router = build_router(state);
    let req = Request::post("/v1/workers/0/restart")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::CONFLICT);

    // Conflict must not have triggered a spawn — still exactly the one
    // call from initial startup.
    assert_eq!(
        spawner.call_count(),
        1,
        "a 409 conflict must not spawn a replacement"
    );
}

/// Verify that the worker spawned by a restart genuinely reaches `Idle` —
/// not just that a new OS process was launched (the previous test's
/// concern), but that the new generation completes registration the same
/// way a normal startup spawn does.
///
/// Sends a synthetic `WorkerEvent::Ready` over a DEALER socket connected
/// to the pool's shared `RouterTransport`, matching
/// `pool_tests.rs`'s own established pattern for driving a mock-spawned
/// worker to `Idle` in tests. Retries the send within the poll loop
/// (rather than sending once) to absorb the small, otherwise-racy window
/// between the new generation's process launching and its
/// `Demux::register()` call completing.
#[tokio::test]
async fn test_restart_respawned_worker_reaches_idle() {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let devices = vec![make_test_device(0, DeviceType::Cpu)];
    let (state, spawner) = make_test_state_with_spawned_workers(node_registry, &devices).await;
    wait_for_spawn_calls(&spawner, 1, Duration::from_secs(2)).await;

    let transport = Arc::clone(state.workers.transport());
    let router = build_router(state.clone());
    let req = Request::post("/v1/workers/0/restart")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::ACCEPTED);

    wait_for_spawn_calls(&spawner, 2, Duration::from_secs(2)).await;

    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let mut dealer = connect_dealer(&transport, "0").await;
            send_event(&mut dealer, &ready_event("0")).await;
            tokio::time::sleep(Duration::from_millis(50)).await;

            let handles = state.workers.handles();
            if let Some(h) = handles.iter().find(|h| h.worker_id == "0")
                && h.status().await == WorkerStatus::Idle
            {
                return;
            }
        }
    })
    .await
    .expect("respawned worker should reach Idle within 3s");
}
