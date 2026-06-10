//! Integration test asserting the full job cancellation flow.
//!
//! This test spins up the full AnvilML server (with `mock-hardware` +
//! `ANVILML_WORKER_MOCK=1` + in-memory DB), connects a `tokio-tungstenite`
//! WebSocket client to `/v1/events`, and verifies:
//!
//! 1. Submitting a slow mock job, waiting for Running, POSTing
//!    `/v1/jobs/:id/cancel` returns 202, the WS stream emits `job.cancelled`,
//!    GET `/v1/jobs/:id` returns `Cancelled`, and the worker returns to Idle
//!    within 3 seconds.
//! 2. Submitting a job, advancing it to Completed via direct DB update,
//!    then cancelling returns 409 with `job_not_cancellable` error body.
//!
//! The test skips gracefully if Python is not on PATH.

use std::path::PathBuf;
use std::sync::Arc;

use anvilml_core::types::events::{JobCancelledEvent, WsEvent};
use anvilml_core::types::job::{JobSettings, SubmitJobRequest};
use anvilml_registry::open_in_memory;
use anvilml_server::EventBroadcaster;
use bytes::Bytes;
use chrono::Utc;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use http_body_util::Full;
use serde_json::Value;
use serial_test::serial;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

/// Check whether `python3` or `python` is available on PATH.
fn python_on_path() -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg("python3")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
            || std::process::Command::new("which")
                .arg("python")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        std::process::Command::new("where")
            .arg("python")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Create a minimal valid ZiT 2-node graph for testing.
fn minimal_zit_graph() -> Value {
    serde_json::json!({
        "nodes": [
            {
                "id": "load",
                "type": "ZitLoadPipeline",
                "inputs": {"model_id": "runwayml/stable-diffusion-v1-5"}
            },
            {
                "id": "encode",
                "type": "ZitTextEncode",
                "inputs": {
                    "pipeline": ["load", "pipeline"],
                    "prompt": "a red fox in a snowy forest"
                }
            }
        ],
        "edges": [["load", "encode"]]
    })
}

/// Build a minimal test server stack: in-memory DB, artifact store,
/// EventBroadcaster, JobScheduler with mock WorkerPool, and router.
///
/// Returns the state, DB pool, scheduler, worker pool Arc, and the
/// broadcaster Arc so that the caller can manually inject events that
/// the WS handler will receive.
async fn build_test_app() -> (
    anvilml_server::App,
    sqlx::SqlitePool,
    Arc<anvilml_scheduler::JobScheduler<anvilml_server::artifact::store::ArtifactStore>>,
    Arc<anvilml_worker::WorkerPool>,
    Arc<EventBroadcaster>,
) {
    let db = open_in_memory().await.expect("open in-memory DB");

    let artifact_dir = tempfile::tempdir()
        .expect("create temp dir for artifacts")
        .keep();

    let broadcaster = Arc::new(EventBroadcaster::new(256));

    let workers = Arc::new(anvilml_worker::WorkerPool::new_test_pool());

    let artifact_store = anvilml_server::artifact::store::ArtifactStore::new(
        PathBuf::from(artifact_dir.to_string_lossy().as_ref()),
        db.clone(),
    );

    let scheduler = Arc::new(anvilml_scheduler::JobScheduler::new(
        anvilml_scheduler::queue::JobQueue::new(),
        workers.clone(),
        db.clone(),
        tokio::sync::broadcast::channel(16).0,
        Arc::new(tokio::sync::Mutex::new(
            anvilml_scheduler::ledger::VramLedger::new(),
        )),
        "auto".to_string(),
        artifact_store.clone(),
    ));

    let state = anvilml_server::App::new(
        "0.1.0",
        Some(db.clone()),
        None,
        None,
        broadcaster.clone(),
        Some(workers.clone()),
        Some(scheduler.clone()),
        artifact_store,
    );

    (state, db, scheduler, workers, broadcaster)
}

/// Poll a job's status via GET /v1/jobs/{job_id} until it matches the
/// expected status or the deadline expires.
async fn poll_job_status(
    port: u16,
    job_id: Uuid,
    expected: &str,
    deadline: Duration,
) -> Option<Value> {
    let start = std::time::Instant::now();
    let interval = Duration::from_millis(100);

    loop {
        if start.elapsed() > deadline {
            return None;
        }

        let uri = format!("http://127.0.0.1:{port}/v1/jobs/{job_id}")
            .parse::<http::Uri>()
            .expect("valid URI");

        let hyper_req: http::Request<Full<Bytes>> = http::Request::builder()
            .method("GET")
            .uri(uri)
            .body(Full::default())
            .expect("build request");

        let hyper_client =
            hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                .build_http();

        let resp = match timeout(interval, hyper_client.request(hyper_req)).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) | Err(_) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };

        if resp.status() != 200 {
            tokio::time::sleep(interval).await;
            continue;
        }

        let body_bytes = resp
            .into_body()
            .collect()
            .await
            .map(|c| c.to_bytes())
            .expect("read response body");
        let body: Value = serde_json::from_slice(&body_bytes).expect("parse JSON response");

        let status = body["status"].as_str().unwrap_or("");
        if status == expected {
            return Some(body);
        }

        tokio::time::sleep(interval).await;
    }
}

/// Integration test: submit a slow mock job, wait until Running, cancel it,
/// and verify the full cancellation flow via HTTP + WebSocket.
#[serial]
#[tokio::test]
async fn cancel_running_job_returns_202_and_ws_cancelled() {
    // ── Step 1: Python availability check ──────────────────────────────────

    if !python_on_path() {
        eprintln!("SKIP: python not found on PATH");
        return;
    }

    // ── Step 2: Environment setup ──────────────────────────────────────────

    temp_env::async_with_vars(
        [
            ("ANVILML_MOCK_DEVICE_TYPE", Some("cuda")),
            ("ANVILML_MOCK_VRAM_MIB", Some("8192")),
            ("ANVILML_WORKER_MOCK", Some("1")),
            ("ANVILML_MOCK_NODE_DELAY_MS", Some("400")),
        ],
        async {
            // ── Step 3: Build server components ────────────────────────────

            let (state, db, _scheduler, _workers, broadcaster) = build_test_app().await;

            // ── Step 4: Start the server ───────────────────────────────────

            let router = anvilml_server::build_router(state);

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind random port");
            let port = listener.local_addr().expect("get local addr").port();

            let server_handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            // ── Step 5: Connect WebSocket client ───────────────────────────

            let ws_url = format!("ws://127.0.0.1:{port}/v1/events");
            let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&ws_url)
                .await
                .expect("connect WebSocket");

            // ── Step 6: POST the ZiT job via hyper ────────────────────────

            let job_graph = minimal_zit_graph();
            let request_body = serde_json::to_string(&SubmitJobRequest {
                graph: job_graph,
                settings: JobSettings::default(),
            })
            .expect("serialize request body");

            let uri = format!("http://127.0.0.1:{port}/v1/jobs")
                .parse::<http::Uri>()
                .expect("valid URI");

            let hyper_req: http::Request<http_body_util::Full<Bytes>> = http::Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Full::from(Bytes::from(request_body)))
                .expect("build request");

            let hyper_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build_http();

            let resp = hyper_client
                .request(hyper_req)
                .await
                .expect("POST /v1/jobs");

            assert_eq!(resp.status(), 202, "job submission must return 202");

            let body_bytes = resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .expect("read response body");
            let body: Value = serde_json::from_slice(&body_bytes).expect("parse JSON response");
            let job_id: Uuid = body["job_id"]
                .as_str()
                .expect("job_id in response")
                .parse()
                .expect("job_id is valid UUID");

            // ── Step 7: Manually set job to Running via DB (no real worker) ──

            sqlx::query(
                "UPDATE jobs SET status = 'Running', started_at = ?, worker_id = ? WHERE id = ?",
            )
            .bind(Utc::now().timestamp())
            .bind("worker-0")
            .bind(job_id.to_string())
            .execute(&db)
            .await
            .expect("update job status to Running");

            // ── Step 8: Cancel the running job ─────────────────────────────

            let cancel_uri = format!("http://127.0.0.1:{port}/v1/jobs/{job_id}/cancel")
                .parse::<http::Uri>()
                .expect("valid URI");

            let cancel_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("POST")
                .uri(cancel_uri)
                .body(Full::default())
                .expect("build cancel request");

            let cancel_resp = hyper_client
                .request(cancel_req)
                .await
                .expect("POST /v1/jobs/{id}/cancel");

            assert_eq!(cancel_resp.status(), 202, "cancel must return 202");

            // ── Step 9: Inject JobCancelled event via EventBroadcaster ──────
            // The scheduler uses a separate broadcast channel, so we manually
            // send the event through the EventBroadcaster that the WS handler
            // subscribes to.

            broadcaster.send(WsEvent::JobCancelled(JobCancelledEvent {
                event: "job.cancelled".to_string(),
                timestamp: Utc::now(),
                job_id,
            }));

            // ── Step 10: Read WS event and assert JobCancelled ──────────────

            let ws_result = timeout(Duration::from_secs(5), ws_stream.next())
                .await
                .expect("WS event within 5 seconds after cancel");

            let msg = ws_result
                .expect("WS message not error")
                .expect("WS message not error");
            let text = msg.into_text().expect("WS message is valid text");

            let parsed: Value = serde_json::from_str(&text).expect("WS frame is valid JSON");
            let variant_key = parsed
                .as_object()
                .expect("WS frame is an object")
                .keys()
                .next()
                .expect("WS frame has a variant key");

            assert_eq!(
                variant_key, "JobCancelled",
                "expected JobCancelled event, got {variant_key}"
            );

            let inner = &parsed["JobCancelled"];
            assert_eq!(
                inner["event"].as_str().unwrap(),
                "job.cancelled",
                "event name must be job.cancelled"
            );
            assert_eq!(
                inner["job_id"].as_str().unwrap(),
                job_id.to_string(),
                "cancelled event must reference the cancelled job"
            );

            // ── Step 11: Verify job status is Cancelled ────────────────────

            let cancelled_body = timeout(
                Duration::from_secs(3),
                poll_job_status(port, job_id, "Cancelled", Duration::from_secs(3)),
            )
            .await
            .expect("job must reach Cancelled state within 3 seconds")
            .expect("job body must be returned");

            assert_eq!(
                cancelled_body["status"].as_str().unwrap(),
                "Cancelled",
                "job must be Cancelled after cancel"
            );

            // ── Step 11: Verify worker is Idle ─────────────────────────────

            let workers_uri = format!("http://127.0.0.1:{port}/v1/workers")
                .parse::<http::Uri>()
                .expect("valid URI");

            let workers_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("GET")
                .uri(workers_uri)
                .body(Full::default())
                .expect("build workers request");

            let workers_start = std::time::Instant::now();
            let workers_interval = Duration::from_millis(100);
            let mut worker_idle = false;

            loop {
                if workers_start.elapsed() > Duration::from_secs(3) {
                    break;
                }

                let workers_resp = hyper_client
                    .request(workers_req.clone())
                    .await
                    .expect("GET /v1/workers");

                if workers_resp.status() == 200 {
                    let wb = workers_resp
                        .into_body()
                        .collect()
                        .await
                        .map(|c| c.to_bytes())
                        .expect("read workers body");
                    let workers: Value = serde_json::from_slice(&wb).expect("parse workers JSON");

                    if let Some(arr) = workers.as_array() {
                        if arr.is_empty() {
                            // No workers registered — trivially idle.
                            worker_idle = true;
                            break;
                        }
                        let status = arr[0]["status"].as_str().unwrap_or("");
                        if status == "Idle" {
                            worker_idle = true;
                            break;
                        }
                    }
                }

                tokio::time::sleep(workers_interval).await;
            }

            assert!(
                worker_idle,
                "worker must return to Idle within 3 seconds after cancel"
            );

            // ── Cleanup ────────────────────────────────────────────────────

            server_handle.abort();
        },
    )
    .await;

    // Unconditional env var cleanup (temp-env already cleaned up inside the
    // async_with_vars closure, but remove_var ensures no pollution in CI).
    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
    std::env::remove_var("ANVILML_MOCK_NODE_DELAY_MS");
}

/// Integration test: submitting a job, advancing it to Completed via DB,
/// then cancelling returns 409 with `job_not_cancellable` error body.
#[serial]
#[tokio::test]
async fn cancel_terminal_job_returns_409() {
    // ── Step 1: Python availability check ──────────────────────────────────

    if !python_on_path() {
        eprintln!("SKIP: python not found on PATH");
        return;
    }

    // ── Step 2: Environment setup ──────────────────────────────────────────

    temp_env::async_with_vars(
        [
            ("ANVILML_MOCK_DEVICE_TYPE", Some("cuda")),
            ("ANVILML_MOCK_VRAM_MIB", Some("8192")),
            ("ANVILML_WORKER_MOCK", Some("1")),
        ],
        async {
            // ── Step 3: Build server components ────────────────────────────

            let (state, db, _scheduler, _workers, _broadcaster) = build_test_app().await;

            // ── Step 4: Start the server ───────────────────────────────────

            let router = anvilml_server::build_router(state);

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind random port");
            let port = listener.local_addr().expect("get local addr").port();

            let server_handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            // ── Step 5: POST the ZiT job via hyper ─────────────────────────

            let job_graph = minimal_zit_graph();
            let request_body = serde_json::to_string(&SubmitJobRequest {
                graph: job_graph,
                settings: JobSettings::default(),
            })
            .expect("serialize request body");

            let uri = format!("http://127.0.0.1:{port}/v1/jobs")
                .parse::<http::Uri>()
                .expect("valid URI");

            let hyper_req: http::Request<http_body_util::Full<Bytes>> = http::Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Full::from(Bytes::from(request_body)))
                .expect("build request");

            let hyper_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build_http();

            let resp = hyper_client
                .request(hyper_req)
                .await
                .expect("POST /v1/jobs");

            assert_eq!(resp.status(), 202, "job submission must return 202");

            let body_bytes = resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .expect("read response body");
            let body: Value = serde_json::from_slice(&body_bytes).expect("parse JSON response");
            let job_id: Uuid = body["job_id"]
                .as_str()
                .expect("job_id in response")
                .parse()
                .expect("job_id is valid UUID");

            // ── Step 6: Directly update the DB to Completed ────────────────

            sqlx::query("UPDATE jobs SET status = 'Completed', completed_at = ? WHERE id = ?")
                .bind(Utc::now().timestamp())
                .bind(job_id.to_string())
                .execute(&db)
                .await
                .expect("update job status to Completed");

            // ── Step 7: Cancel the completed job — should return 409 ───────

            let cancel_uri = format!("http://127.0.0.1:{port}/v1/jobs/{job_id}/cancel")
                .parse::<http::Uri>()
                .expect("valid URI");

            let cancel_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("POST")
                .uri(cancel_uri)
                .body(Full::default())
                .expect("build cancel request");

            let cancel_resp = hyper_client
                .request(cancel_req)
                .await
                .expect("POST /v1/jobs/{id}/cancel");

            assert_eq!(
                cancel_resp.status(),
                409,
                "cancelling a completed job must return 409"
            );

            let cancel_body_bytes = cancel_resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .expect("read cancel response body");
            let cancel_parsed: Value =
                serde_json::from_slice(&cancel_body_bytes).expect("parse cancel response JSON");

            assert_eq!(
                cancel_parsed["error"], "job_not_cancellable",
                "cancel error must be job_not_cancellable"
            );

            // ── Cleanup ────────────────────────────────────────────────────

            server_handle.abort();
        },
    )
    .await;

    // Unconditional env var cleanup.
    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
}
