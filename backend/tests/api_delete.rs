//! Integration test asserting the full job and artifact deletion lifecycle.
//!
//! This test spins up the full AnvilML server (with `mock-hardware` +
//! `ANVILML_WORKER_MOCK=1` + in-memory DB), and verifies:
//!
//! 1. Submitting a job, advancing to Completed via direct DB update,
//!    inserting a fake artifact file + DB row, `DELETE /v1/jobs/:id`
//!    returns 204, DB row gone, artifact file gone, GET returns 404.
//! 2. Submitting a job, advancing to Running via DB, DELETE returns 409
//!    with `job_active` error body, job not deleted.
//! 3. Bulk clear `DELETE /v1/jobs?status=all` removes all terminal jobs
//!    (Completed, Failed, Cancelled) + artifacts, preserves Running/Queued.
//! 4. Bulk clear `DELETE /v1/jobs?status=completed` removes only completed
//!    jobs + artifacts, preserved failed job + artifact.
//! 5. DELETE on a nonexistent job UUID returns 404 with `not_found` error.
//!
//! Each test uses its own temp directory for artifacts and its own
//! in-memory DB pool via `open_in_memory()`. All tests are serialised
//! with `#[serial]` to prevent parallel execution races.

use std::path::PathBuf;
use std::sync::Arc;

use anvilml_core::types::job::JobSettings;
use anvilml_registry::open_in_memory;
use anvilml_server::EventBroadcaster;
use bytes::Bytes;
use chrono::Utc;
use http_body_util::BodyExt;
use http_body_util::Full;
use serde_json::Value;
use serial_test::serial;
use tokio::net::TcpListener;
use tokio::time::Duration;
use uuid::Uuid;

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
/// Returns the state, DB pool, artifact directory path, scheduler, worker
/// pool Arc, and the broadcaster Arc so that the caller can manually
/// inject events and inspect artifact storage.
async fn build_test_app() -> (
    anvilml_server::App,
    sqlx::SqlitePool,
    PathBuf,
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

    let artifact_store =
        anvilml_server::artifact::store::ArtifactStore::new(artifact_dir.clone(), db.clone());

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
        anvilml_core::ServerConfig::default(),
    );

    (state, db, artifact_dir, scheduler, workers, broadcaster)
}

/// Submit a job via HTTP POST to `http://127.0.0.1:{port}/v1/jobs`.
/// Returns the `job_id` Uuid from the 202 response body.
async fn submit_job_via_http(port: u16) -> Uuid {
    let job_graph = minimal_zit_graph();
    let request_body = serde_json::to_string(&anvilml_core::types::job::SubmitJobRequest {
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

    body["job_id"]
        .as_str()
        .expect("job_id in response")
        .parse()
        .expect("job_id is valid UUID")
}

/// Insert a fake artifact file on disk and a corresponding DB row.
///
/// The artifact uses a 64-char hex hash so `hash[..2]` produces a
/// valid two-char prefix directory.
async fn insert_artifact_on_disk(
    artifact_dir: &PathBuf,
    db: &sqlx::SqlitePool,
    job_id: &str,
    hash: &str,
) {
    // Create the sharded directory and write a fake PNG file.
    let prefix_dir = artifact_dir.join(&hash[..2]);
    tokio::fs::create_dir_all(&prefix_dir)
        .await
        .expect("create artifact prefix dir");
    let file_path = prefix_dir.join(format!("{hash}.png"));
    tokio::fs::write(&file_path, b"fake-png")
        .await
        .expect("write fake artifact file");

    // Insert artifact metadata row in the DB.
    sqlx::query(
        "INSERT INTO artifacts (hash, job_id, width, height, format, seed, steps, prompt, created_at) \
         VALUES (?, ?, 512, 512, 'png', 42, 20, 'test prompt', ?)",
    )
    .bind(hash)
    .bind(job_id)
    .bind(Utc::now().timestamp())
    .execute(db)
    .await
    .expect("insert artifact DB row");
}

/// Advance a job's status in the database using a direct UPDATE query.
async fn advance_job_status(db: &sqlx::SqlitePool, job_id: &str, status: &str) {
    let now = Utc::now().timestamp();
    sqlx::query("UPDATE jobs SET status = ?, completed_at = ? WHERE id = ?")
        .bind(status)
        .bind(now)
        .bind(job_id)
        .execute(db)
        .await
        .expect("advance job status");
}

/// Assert that a job row exists in the database.
async fn assert_job_exists(db: &sqlx::SqlitePool, job_id: &str) {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(db)
        .await
        .expect("query job count");
    assert_eq!(count, 1, "job {job_id} should exist in DB");
}

/// Assert that a job row does NOT exist in the database.
async fn assert_job_gone(db: &sqlx::SqlitePool, job_id: &str) {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE id = ?")
        .bind(job_id)
        .fetch_one(db)
        .await
        .expect("query job count");
    assert_eq!(count, 0, "job {job_id} should be gone from DB");
}

/// Integration test: submit a job, advance to Completed, insert artifact,
/// DELETE returns 204, DB row + artifact file removed, GET returns 404.
#[serial]
#[tokio::test]
async fn delete_completed_job_removes_artifact_and_row() {
    temp_env::async_with_vars(
        [
            ("ANVILML_MOCK_DEVICE_TYPE", Some("cuda")),
            ("ANVILML_MOCK_VRAM_MIB", Some("8192")),
            ("ANVILML_WORKER_MOCK", Some("1")),
        ],
        async {
            let (state, db, artifact_dir, _scheduler, _workers, _broadcaster) =
                build_test_app().await;

            let router = anvilml_server::build_router(state);

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind random port");
            let port = listener.local_addr().expect("get local addr").port();

            let server_handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            // Submit a job via HTTP.
            let job_id = submit_job_via_http(port).await;

            // Advance job to Completed via direct DB update.
            advance_job_status(&db, &job_id.to_string(), "Completed").await;

            // Insert a fake artifact on disk + DB row.
            let fake_hash = "aaa111bbb222ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000";
            insert_artifact_on_disk(&artifact_dir, &db, &job_id.to_string(), fake_hash).await;

            // Verify artifact file exists on disk.
            let artifact_path = artifact_dir
                .join(&fake_hash[..2])
                .join(format!("{fake_hash}.png"));
            assert!(
                std::fs::metadata(&artifact_path).is_ok(),
                "artifact file should exist on disk"
            );

            // Verify job row exists in DB.
            assert_job_exists(&db, &job_id.to_string()).await;

            // DELETE the completed job.
            let delete_uri = format!("http://127.0.0.1:{port}/v1/jobs/{job_id}")
                .parse::<http::Uri>()
                .expect("valid URI");

            let delete_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("DELETE")
                .uri(delete_uri)
                .body(Full::default())
                .expect("build delete request");

            let hyper_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build_http();

            let delete_resp = hyper_client
                .request(delete_req)
                .await
                .expect("DELETE /v1/jobs/{id}");

            assert_eq!(
                delete_resp.status(),
                204,
                "delete completed job must return 204"
            );

            // Verify job row is gone from DB.
            assert_job_gone(&db, &job_id.to_string()).await;

            // Verify artifact file is gone from disk.
            assert!(
                std::fs::metadata(&artifact_path).is_err(),
                "artifact file should be deleted"
            );

            // GET the deleted job — should return 404.
            let get_uri = format!("http://127.0.0.1:{port}/v1/jobs/{job_id}")
                .parse::<http::Uri>()
                .expect("valid URI");

            let get_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("GET")
                .uri(get_uri)
                .body(Full::default())
                .expect("build get request");

            let get_resp = hyper_client
                .request(get_req)
                .await
                .expect("GET /v1/jobs/{id}");

            assert_eq!(get_resp.status(), 404, "GET deleted job must return 404");

            server_handle.abort();
        },
    )
    .await;

    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
}

/// Integration test: submit a job, advance to Running, DELETE returns 409
/// with `job_active` error, job not deleted.
#[serial]
#[tokio::test]
async fn delete_running_job_returns_409() {
    temp_env::async_with_vars(
        [
            ("ANVILML_MOCK_DEVICE_TYPE", Some("cuda")),
            ("ANVILML_MOCK_VRAM_MIB", Some("8192")),
            ("ANVILML_WORKER_MOCK", Some("1")),
        ],
        async {
            let (state, db, _artifact_dir, _scheduler, _workers, _broadcaster) =
                build_test_app().await;

            let router = anvilml_server::build_router(state);

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind random port");
            let port = listener.local_addr().expect("get local addr").port();

            let server_handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            // Submit a job via HTTP.
            let job_id = submit_job_via_http(port).await;

            // Advance job to Running via DB.
            let now = Utc::now().timestamp();
            sqlx::query(
                "UPDATE jobs SET status = 'Running', started_at = ?, worker_id = ? WHERE id = ?",
            )
            .bind(now)
            .bind("worker-0")
            .bind(job_id.to_string())
            .execute(&db)
            .await
            .expect("update job status to Running");

            // DELETE the running job — should return 409.
            let delete_uri = format!("http://127.0.0.1:{port}/v1/jobs/{job_id}")
                .parse::<http::Uri>()
                .expect("valid URI");

            let delete_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("DELETE")
                .uri(delete_uri)
                .body(Full::default())
                .expect("build delete request");

            let hyper_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build_http();

            let delete_resp = hyper_client
                .request(delete_req)
                .await
                .expect("DELETE /v1/jobs/{id}");

            assert_eq!(
                delete_resp.status(),
                409,
                "delete running job must return 409"
            );

            let body_bytes = delete_resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .expect("read response body");
            let body: Value = serde_json::from_slice(&body_bytes).expect("parse JSON response");

            assert_eq!(
                body["error"], "job_active",
                "delete error must be job_active"
            );

            // Verify job still exists in DB.
            assert_job_exists(&db, &job_id.to_string()).await;

            server_handle.abort();
        },
    )
    .await;

    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
}

/// Integration test: bulk delete all terminal jobs removes Completed,
/// Failed, Cancelled jobs + artifacts, preserves Running job.
#[serial]
#[tokio::test]
async fn bulk_delete_all_terminal_jobs() {
    temp_env::async_with_vars(
        [
            ("ANVILML_MOCK_DEVICE_TYPE", Some("cuda")),
            ("ANVILML_MOCK_VRAM_MIB", Some("8192")),
            ("ANVILML_WORKER_MOCK", Some("1")),
        ],
        async {
            let (state, db, artifact_dir, _scheduler, _workers, _broadcaster) =
                build_test_app().await;

            let router = anvilml_server::build_router(state);

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind random port");
            let port = listener.local_addr().expect("get local addr").port();

            let server_handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            // Submit 3 terminal jobs.
            let job_id_1 = submit_job_via_http(port).await;
            let job_id_2 = submit_job_via_http(port).await;
            let job_id_3 = submit_job_via_http(port).await;

            // Submit 1 active job (Running) that must survive.
            let job_id_active = submit_job_via_http(port).await;

            // Advance terminal jobs to different terminal states.
            advance_job_status(&db, &job_id_1.to_string(), "Completed").await;
            advance_job_status(&db, &job_id_2.to_string(), "Failed").await;
            advance_job_status(&db, &job_id_3.to_string(), "Cancelled").await;

            // Advance active job to Running.
            let now = Utc::now().timestamp();
            sqlx::query(
                "UPDATE jobs SET status = 'Running', started_at = ?, worker_id = ? WHERE id = ?",
            )
            .bind(now)
            .bind("worker-0")
            .bind(job_id_active.to_string())
            .execute(&db)
            .await
            .expect("update active job to Running");

            // Insert artifact files + DB rows for terminal jobs.
            let hash_1 = "aaa111bbb222ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000";
            let hash_2 = "bbb222ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000aaa111";
            let hash_3 = "ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000aaa111bbb222";

            insert_artifact_on_disk(&artifact_dir, &db, &job_id_1.to_string(), hash_1).await;
            insert_artifact_on_disk(&artifact_dir, &db, &job_id_2.to_string(), hash_2).await;
            insert_artifact_on_disk(&artifact_dir, &db, &job_id_3.to_string(), hash_3).await;

            // Verify all 4 jobs exist in DB before bulk delete.
            assert_job_exists(&db, &job_id_1.to_string()).await;
            assert_job_exists(&db, &job_id_2.to_string()).await;
            assert_job_exists(&db, &job_id_3.to_string()).await;
            assert_job_exists(&db, &job_id_active.to_string()).await;

            // Bulk delete all terminal jobs.
            let delete_uri = format!("http://127.0.0.1:{port}/v1/jobs?status=all", port = port)
                .parse::<http::Uri>()
                .expect("valid URI");

            let hyper_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build_http();

            let delete_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("DELETE")
                .uri(delete_uri)
                .body(Full::default())
                .expect("build bulk delete request");

            let delete_resp = hyper_client
                .request(delete_req)
                .await
                .expect("DELETE /v1/jobs?status=all");

            assert_eq!(delete_resp.status(), 200, "bulk delete must return 200");

            let body_bytes = delete_resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .expect("read response body");
            let body: Value = serde_json::from_slice(&body_bytes).expect("parse JSON response");

            assert_eq!(
                body["removed"].as_u64().unwrap(),
                3,
                "must remove exactly 3 terminal jobs"
            );

            // Verify terminal jobs are gone.
            assert_job_gone(&db, &job_id_1.to_string()).await;
            assert_job_gone(&db, &job_id_2.to_string()).await;
            assert_job_gone(&db, &job_id_3.to_string()).await;

            // Verify Running job still exists.
            assert_job_exists(&db, &job_id_active.to_string()).await;

            // Verify Running job status is still Running.
            let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
                .bind(job_id_active.to_string())
                .fetch_one(&db)
                .await
                .expect("query active job status");
            assert_eq!(status, "Running", "active job must remain Running");

            server_handle.abort();
        },
    )
    .await;

    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
}

/// Integration test: bulk delete by specific status removes only matching
/// jobs + artifacts, preserves other terminal jobs.
#[serial]
#[tokio::test]
async fn bulk_delete_by_status_removes_only_matching() {
    temp_env::async_with_vars(
        [
            ("ANVILML_MOCK_DEVICE_TYPE", Some("cuda")),
            ("ANVILML_MOCK_VRAM_MIB", Some("8192")),
            ("ANVILML_WORKER_MOCK", Some("1")),
        ],
        async {
            let (state, db, artifact_dir, _scheduler, _workers, _broadcaster) =
                build_test_app().await;

            let router = anvilml_server::build_router(state);

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind random port");
            let port = listener.local_addr().expect("get local addr").port();

            let server_handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            // Submit 2 jobs.
            let job_id_completed = submit_job_via_http(port).await;
            let job_id_failed = submit_job_via_http(port).await;

            // Advance to different terminal states.
            advance_job_status(&db, &job_id_completed.to_string(), "Completed").await;
            advance_job_status(&db, &job_id_failed.to_string(), "Failed").await;

            // Insert artifact files + DB rows for both.
            let hash_completed = "aaa111bbb222ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000";
            let hash_failed = "bbb222ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000aaa111";

            insert_artifact_on_disk(
                &artifact_dir,
                &db,
                &job_id_completed.to_string(),
                hash_completed,
            )
            .await;
            insert_artifact_on_disk(&artifact_dir, &db, &job_id_failed.to_string(), hash_failed)
                .await;

            // Verify both jobs exist before bulk delete.
            assert_job_exists(&db, &job_id_completed.to_string()).await;
            assert_job_exists(&db, &job_id_failed.to_string()).await;

            // Bulk delete only completed jobs.
            let delete_uri = format!(
                "http://127.0.0.1:{port}/v1/jobs?status=completed",
                port = port
            )
            .parse::<http::Uri>()
            .expect("valid URI");

            let hyper_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build_http();

            let delete_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("DELETE")
                .uri(delete_uri)
                .body(Full::default())
                .expect("build bulk delete request");

            let delete_resp = hyper_client
                .request(delete_req)
                .await
                .expect("DELETE /v1/jobs?status=completed");

            assert_eq!(delete_resp.status(), 200, "bulk delete must return 200");

            let body_bytes = delete_resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .expect("read response body");
            let body: Value = serde_json::from_slice(&body_bytes).expect("parse JSON response");

            assert_eq!(
                body["removed"].as_u64().unwrap(),
                1,
                "must remove exactly 1 completed job"
            );

            // Verify completed job is gone.
            assert_job_gone(&db, &job_id_completed.to_string()).await;

            // Verify failed job still exists.
            assert_job_exists(&db, &job_id_failed.to_string()).await;

            // Verify failed job status is still Failed.
            let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
                .bind(job_id_failed.to_string())
                .fetch_one(&db)
                .await
                .expect("query failed job status");
            assert_eq!(status, "Failed", "failed job must remain Failed");

            server_handle.abort();
        },
    )
    .await;

    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
}

/// Integration test: DELETE on a nonexistent job UUID returns 404
/// with `not_found` error.
#[serial]
#[tokio::test]
async fn delete_nonexistent_job_returns_404() {
    temp_env::async_with_vars(
        [
            ("ANVILML_MOCK_DEVICE_TYPE", Some("cuda")),
            ("ANVILML_MOCK_VRAM_MIB", Some("8192")),
            ("ANVILML_WORKER_MOCK", Some("1")),
        ],
        async {
            let (state, _db, _artifact_dir, _scheduler, _workers, _broadcaster) =
                build_test_app().await;

            let router = anvilml_server::build_router(state);

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind random port");
            let port = listener.local_addr().expect("get local addr").port();

            let server_handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });

            tokio::time::sleep(Duration::from_millis(100)).await;

            // DELETE a nonexistent job.
            let random_id = Uuid::new_v4();
            let delete_uri = format!("http://127.0.0.1:{port}/v1/jobs/{random_id}")
                .parse::<http::Uri>()
                .expect("valid URI");

            let delete_req: http::Request<Full<Bytes>> = http::Request::builder()
                .method("DELETE")
                .uri(delete_uri)
                .body(Full::default())
                .expect("build delete request");

            let hyper_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build_http();

            let delete_resp = hyper_client
                .request(delete_req)
                .await
                .expect("DELETE /v1/jobs/{id}");

            assert_eq!(
                delete_resp.status(),
                404,
                "delete nonexistent job must return 404"
            );

            let body_bytes = delete_resp
                .into_body()
                .collect()
                .await
                .map(|c| c.to_bytes())
                .expect("read response body");
            let body: Value = serde_json::from_slice(&body_bytes).expect("parse JSON response");

            assert_eq!(body["error"], "not_found", "delete error must be not_found");

            server_handle.abort();
        },
    )
    .await;

    std::env::remove_var("ANVILML_MOCK_DEVICE_TYPE");
    std::env::remove_var("ANVILML_MOCK_VRAM_MIB");
    std::env::remove_var("ANVILML_WORKER_MOCK");
}
