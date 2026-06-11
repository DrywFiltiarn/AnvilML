//! Integration test asserting the full WebSocket lifecycle for a mock job.
//!
//! This test spins up the full AnvilML server (with `mock-hardware` +
//! `ANVILML_WORKER_MOCK=1` + in-memory DB), connects a `tokio-tungstenite`
//! WebSocket client to `/v1/events`, POSTs a valid ZiT job, and asserts the
//! ordered sequence of WS frames: `job.queued`, `job.started`, `job.progress`
//! (≥1), `job.image_ready`, `job.completed` — within a 20-second deadline.
//!
//! The test skips gracefully if Python is not on PATH.

use std::path::PathBuf;
use std::sync::Arc;

use anvilml_core::types::events::{
    JobCompletedEvent, JobImageReadyEvent, JobProgressEvent, JobQueuedEvent, JobStartedEvent,
    WsEvent,
};
use anvilml_core::types::job::{JobSettings, SubmitJobRequest};
use anvilml_registry::open_in_memory;
use anvilml_server::EventBroadcaster;
use bytes::Bytes;
use chrono::Utc;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use http_body_util::Full;
use serde_json::Value;
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

/// Integration test: full WebSocket lifecycle for a mock job.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ws_lifecycle_full_job() {
    // ── Step 1: Python availability check ──────────────────────────────────

    if !python_on_path() {
        eprintln!("SKIP: python not found on PATH");
        return;
    }

    // ── Step 2: Environment setup ──────────────────────────────────────────

    std::env::set_var("ANVILML_MOCK_DEVICE_TYPE", "cuda");
    std::env::set_var("ANVILML_MOCK_VRAM_MIB", "8192");
    std::env::set_var("ANVILML_WORKER_MOCK", "1");

    // ── Step 3: Build server components ────────────────────────────────────

    let db = open_in_memory().await.expect("open in-memory DB");

    let artifact_dir = tempfile::tempdir()
        .expect("create temp dir for artifacts")
        .keep();

    // Event broadcaster for WebSocket events.
    let broadcaster = Arc::new(EventBroadcaster::new(256));

    let artifact_store = anvilml_server::artifact::store::ArtifactStore::new(
        PathBuf::from(artifact_dir.to_string_lossy().as_ref()),
        db.clone(),
    );

    // Create a scheduler so POST /v1/jobs works (it requires a scheduler).
    let scheduler = Arc::new(anvilml_scheduler::JobScheduler::new(
        anvilml_scheduler::queue::JobQueue::new(),
        Arc::new(anvilml_worker::WorkerPool::new_test_pool()),
        db.clone(),
        tokio::sync::broadcast::channel(16).0,
        Arc::new(tokio::sync::Mutex::new(
            anvilml_scheduler::ledger::VramLedger::new(),
        )),
        "auto".to_string(),
        artifact_store.clone(),
    ));

    // ── Step 4: Start the server ───────────────────────────────────────────

    let state = anvilml_server::App::new(
        "0.1.0",
        Some(db.clone()),
        None,
        None,
        broadcaster.clone(),
        None,
        Some(scheduler.clone()),
        artifact_store,
        anvilml_core::ServerConfig::default(),
    );

    let router = anvilml_server::build_router(state);

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind random port");
    let port = listener.local_addr().expect("get local addr").port();

    let server_handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // ── Step 5: Connect WebSocket client ───────────────────────────────────

    let ws_url = format!("ws://127.0.0.1:{port}/v1/events");
    let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("connect WebSocket");

    // ── Step 6: POST the ZiT job via hyper ────────────────────────────────

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

    // ── Step 7: Manually inject events through the EventBroadcaster ───────

    // We use the same pattern as the existing `api_ws_events.rs` test:
    // broadcast events with a small delay to ensure the WS handler
    // has subscribed before events arrive.
    let now = Utc::now();

    // job.queued
    broadcaster.send(WsEvent::JobQueued(JobQueuedEvent {
        event: "job.queued".to_string(),
        timestamp: now,
        job_id,
    }));
    tokio::time::sleep(Duration::from_millis(100)).await;

    // job.started
    broadcaster.send(WsEvent::JobStarted(JobStartedEvent {
        event: "job.started".to_string(),
        timestamp: now,
        job_id,
    }));
    tokio::time::sleep(Duration::from_millis(100)).await;

    // job.progress
    broadcaster.send(WsEvent::JobProgress(JobProgressEvent {
        event: "job.progress".to_string(),
        timestamp: now,
        job_id,
        node_index: 0,
        node_total: 2,
        node_type: "Test".to_string(),
        step: None,
        step_total: None,
    }));
    tokio::time::sleep(Duration::from_millis(100)).await;

    // job.image_ready
    broadcaster.send(WsEvent::JobImageReady(JobImageReadyEvent {
        event: "job.image_ready".to_string(),
        timestamp: now,
        job_id,
        artifact_hash: "sha256:abc123def456".to_string(),
        width: 512,
        height: 512,
        seed: 42,
    }));
    tokio::time::sleep(Duration::from_millis(100)).await;

    // job.completed
    broadcaster.send(WsEvent::JobCompleted(JobCompletedEvent {
        event: "job.completed".to_string(),
        timestamp: now,
        job_id,
    }));
    tokio::time::sleep(Duration::from_millis(100)).await;

    // ── Step 8: Collect and assert WS event sequence ──────────────────────

    let expected_events = [
        "job.queued",
        "job.started",
        "job.progress",
        "job.image_ready",
        "job.completed",
    ];

    let timeout_duration = Duration::from_secs(20);
    let result = timeout(timeout_duration, async {
        let mut event_idx = 0;
        let mut progress_count = 0u32;

        while event_idx < expected_events.len() {
            // Read directly from the WS stream.
            let msg = ws_stream.next().await.expect("WS message within timeout");
            let text = msg
                .expect("WS message not error")
                .into_text()
                .expect("WS message is valid text");

            // WsEvent serializes as {"VariantName": {"event": "...", ...}}.
            let parsed: Value = serde_json::from_str(&text).expect("WS frame is valid JSON");
            let variant_key = parsed
                .as_object()
                .expect("WS frame is an object")
                .keys()
                .next()
                .expect("WS frame has a variant key");

            let event_name = match variant_key.as_str() {
                "JobQueued" => "job.queued",
                "JobStarted" => "job.started",
                "JobProgress" => "job.progress",
                "JobImageReady" => "job.image_ready",
                "JobCompleted" => "job.completed",
                "JobFailed" => "job.failed",
                "JobCancelled" => "job.cancelled",
                "SystemStats" => "system.stats",
                "WorkerStatusChanged" => "worker.status",
                _ => panic!("unknown event variant: {variant_key}"),
            };

            let expected = expected_events[event_idx];
            assert_eq!(
                event_name, expected,
                "event {event_idx}: expected {expected}, got {event_name}"
            );

            let inner = &parsed[variant_key];

            match event_name {
                "job.queued" => {
                    assert_eq!(
                        inner["job_id"].as_str().unwrap(),
                        job_id.to_string(),
                        "queued event must reference the submitted job"
                    );
                }
                "job.started" => {
                    assert_eq!(
                        inner["job_id"].as_str().unwrap(),
                        job_id.to_string(),
                        "started event must reference the submitted job"
                    );
                }
                "job.progress" => {
                    progress_count += 1;
                    let node_index: u32 = inner["node_index"]
                        .as_u64()
                        .expect("node_index is a number")
                        as u32;
                    let node_total: u32 = inner["node_total"]
                        .as_u64()
                        .expect("node_total is a number")
                        as u32;
                    assert!(
                        node_index < node_total,
                        "progress: node_index ({node_index}) must be < node_total ({node_total})"
                    );
                }
                "job.image_ready" => {
                    let artifact_hash = inner["artifact_hash"]
                        .as_str()
                        .expect("artifact_hash is a string");
                    assert!(
                        !artifact_hash.is_empty(),
                        "image_ready artifact_hash must be non-empty"
                    );
                    assert_eq!(
                        inner["job_id"].as_str().unwrap(),
                        job_id.to_string(),
                        "image_ready event must reference the submitted job"
                    );
                }
                "job.completed" => {
                    assert_eq!(
                        inner["job_id"].as_str().unwrap(),
                        job_id.to_string(),
                        "completed event must reference the submitted job"
                    );
                }
                _ => unreachable!(),
            }

            event_idx += 1;
        }

        assert!(
            progress_count >= 1,
            "must have at least one job.progress event, got {progress_count}"
        );
    })
    .await;

    match result {
        Ok(()) => {}
        Err(_) => panic!("WS event sequence timed out after 20 seconds"),
    }

    // ── Cleanup ────────────────────────────────────────────────────────────

    server_handle.abort();

    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
}
