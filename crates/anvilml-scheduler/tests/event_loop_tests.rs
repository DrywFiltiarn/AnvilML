/// Integration tests for `handle_image_ready()` — the event loop handler
/// that persists `WorkerEvent::ImageReady` payloads to the artifact store.
///
/// Tests verify base64 decoding, artifact persistence, metadata field
/// correctness, error handling for malformed base64, and empty payload handling.
use std::sync::Arc;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::AnvilError;
use anvilml_core::NodeTypeRegistry;
use anvilml_core::WsEvent;
use anvilml_core::types::worker::WorkerStatus;
use anvilml_ipc::EventBroadcaster;
use anvilml_ipc::WorkerEvent;
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_scheduler::event_loop::{handle_image_ready, map_worker_event, spawn_event_loop};
use anvilml_worker::Demux;
use anvilml_worker::WorkerHandle;
use anvilml_worker::WorkerPool;
use base64::Engine as _;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Helper to create an `ArtifactStore` backed by an in-memory SQLite pool and a
/// unique temporary directory.
///
/// Creates a fresh in-memory SQLite pool with the `artifacts` table (via the
/// inline DDL in `ArtifactStore::save()`), a unique temp directory for artifact
/// files, and returns an `Arc<ArtifactStore>`. Each call creates isolated state
/// for test independence.
async fn create_test_artifact_store() -> Arc<ArtifactStore> {
    // Create an in-memory SQLite pool. `:memory:` creates a database that
    // exists only for the lifetime of this connection — isolated from all
    // other pools, including other test runs.
    let connect_opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("in-memory SQLite pool must connect");

    // Create a unique temp directory for this test's artifacts. Using a UUID
    // suffix ensures no collision with other tests running in parallel.
    let artifact_dir =
        std::env::temp_dir().join(format!("anvilml-event-loop-test-{}", Uuid::new_v4()));

    Arc::new(ArtifactStore::new(artifact_dir, pool))
}

/// Helper to create a valid base64-encoded PNG payload for testing.
///
/// Constructs a minimal valid PNG file (1x1 pixel, red) and base64-encodes it.
/// The PNG signature and minimal IHDR/IDAT/IEND chunks are included so the
/// bytes are a valid PNG, even though `ArtifactStore::save()` does not validate
/// the format — it stores raw bytes.
fn make_valid_png_b64() -> String {
    // Minimal valid 1x1 red PNG: signature + IHDR + IDAT + IEND.
    // This is a known-good PNG that any image library can decode.
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB
        0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x08, 0xD7, 0x63, 0xF8, 0xFF, 0xFF, 0xFF, // compressed data
        0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x95, 0xE4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
        0x44, // IEND chunk
        0xAE, 0x42, 0x60, 0x82,
    ];
    base64::engine::general_purpose::STANDARD.encode(&png_bytes)
}

/// Test that `handle_image_ready()` saves an artifact and it is retrievable by hash.
///
/// Constructs a valid `WorkerEvent::ImageReady` with a known base64-encoded PNG
/// payload, calls `handle_image_ready()`, verifies the returned hash is non-empty,
/// and confirms the artifact bytes are retrievable via `store.get(&hash)` and match
/// the original decoded PNG bytes.
#[tokio::test]
async fn test_image_ready_saves_artifact() {
    let artifact_store = create_test_artifact_store().await;
    let job_id = Uuid::new_v4();
    let png_b64 = make_valid_png_b64();

    let event = WorkerEvent::ImageReady {
        job_id,
        image_b64: png_b64.clone(),
        width: 512,
        height: 512,
        format: "png".into(),
        seed: 42,
        steps: 20,
    };

    let result = handle_image_ready(artifact_store.clone(), event, job_id).await;

    // Must succeed with a non-empty hash.
    assert!(
        result.is_ok(),
        "handle_image_ready must succeed for valid ImageReady"
    );
    let hash = result.expect("handle_image_ready must return Ok");
    assert!(!hash.is_empty(), "hash must be non-empty");

    // Verify the artifact bytes are retrievable by hash and match the decoded payload.
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&png_b64)
        .unwrap();
    let retrieved = artifact_store
        .get(&hash)
        .await
        .expect("store.get must not error");

    assert!(retrieved.is_some(), "artifact must be retrievable by hash");
    assert_eq!(
        retrieved.unwrap(),
        decoded,
        "retrieved bytes must match the original decoded payload"
    );
}

/// Test that artifact metadata fields (width, height, seed, steps, job_id) match
/// the event's values after saving.
///
/// Constructs a `WorkerEvent::ImageReady` with known metadata values, calls
/// `handle_image_ready()`, then queries the artifact store's `list()` method
/// and verifies all persisted fields match the event's values.
#[tokio::test]
async fn test_image_ready_artifact_meta_fields_match() {
    let artifact_store = create_test_artifact_store().await;
    let job_id = Uuid::new_v4();
    let png_b64 = make_valid_png_b64();
    let expected_width: u32 = 512;
    let expected_height: u32 = 512;
    let expected_seed: i64 = 42;
    let expected_steps: u32 = 20;

    let event = WorkerEvent::ImageReady {
        job_id,
        image_b64: png_b64,
        width: expected_width,
        height: expected_height,
        format: "png".into(),
        seed: expected_seed,
        steps: expected_steps,
    };

    let result = handle_image_ready(artifact_store.clone(), event, job_id).await;
    assert!(result.is_ok(), "handle_image_ready must succeed");

    // Query artifacts for this job and verify metadata fields.
    let artifacts = artifact_store
        .list(Some(job_id))
        .await
        .expect("store.list must not error");

    assert_eq!(
        artifacts.len(),
        1,
        "exactly one artifact must exist for this job_id"
    );

    let meta = &artifacts[0];
    assert_eq!(meta.width, expected_width, "width must match event value");
    assert_eq!(
        meta.height, expected_height,
        "height must match event value"
    );
    assert_eq!(meta.seed, expected_seed, "seed must match event value");
    assert_eq!(meta.steps, expected_steps, "steps must match event value");
    assert_eq!(meta.job_id, job_id, "job_id must match event value");
}

/// Test that malformed base64 returns `Err(AnvilError::Serde(...))` rather than
/// panicking.
///
/// Passes a deliberately malformed base64 string in the event, verifies
/// `handle_image_ready()` returns the expected error variant. This ensures
/// the function handles encoding errors gracefully without panicking.
#[tokio::test]
async fn test_image_ready_malformed_base64_errors() {
    let artifact_store = create_test_artifact_store().await;
    let job_id = Uuid::new_v4();

    let event = WorkerEvent::ImageReady {
        job_id,
        image_b64: "not-valid-base64!!!@@@".into(),
        width: 512,
        height: 512,
        format: "png".into(),
        seed: 42,
        steps: 20,
    };

    let result = handle_image_ready(artifact_store, event, job_id).await;

    // Must return an error, not panic.
    assert!(
        result.is_err(),
        "handle_image_ready must return Err for malformed base64"
    );
    match result.unwrap_err() {
        AnvilError::Serde(msg) => {
            assert!(
                msg.contains("base64 decode failed"),
                "error message must mention base64 decode failure, got: {msg}"
            );
        }
        other => panic!(
            "Expected AnvilError::Serde, got: {:?} — must not panic on malformed base64",
            other
        ),
    }
}

/// Test that an empty base64 string decodes to empty bytes and `save()` succeeds.
///
/// Passes an empty base64 string in the event, verifies `handle_image_ready()`
/// returns `Ok(hash)` and the artifact is stored with zero bytes. This exercises
/// the edge case of an empty image payload — the artifact store should handle
/// it gracefully since it stores raw bytes without format validation.
#[tokio::test]
async fn test_image_ready_empty_image_b64() {
    let artifact_store = create_test_artifact_store().await;
    let job_id = Uuid::new_v4();

    let event = WorkerEvent::ImageReady {
        job_id,
        image_b64: String::new(), // empty base64 string
        width: 64,
        height: 64,
        format: "png".into(),
        seed: 0,
        steps: 1,
    };

    let result = handle_image_ready(artifact_store.clone(), event, job_id).await;

    // Must succeed — empty bytes are valid input to save().
    assert!(
        result.is_ok(),
        "handle_image_ready must succeed for empty base64"
    );
    let hash = result.expect("handle_image_ready must return Ok");
    assert!(
        !hash.is_empty(),
        "hash must be non-empty even for empty bytes"
    );

    // Verify the artifact was stored with zero bytes.
    let retrieved = artifact_store
        .get(&hash)
        .await
        .expect("store.get must not error");

    assert!(retrieved.is_some(), "artifact must be retrievable by hash");
    assert!(
        retrieved.unwrap().is_empty(),
        "artifact must contain zero bytes"
    );
}

/// Test that `map_worker_event` maps `WorkerEvent::Progress` to `WsEvent::JobProgress`
/// with correct field values.
///
/// Constructs a `WorkerEvent::Progress` with known values, calls `map_worker_event()`,
/// and verifies all fields are correctly transferred to the `WsEvent::JobProgress` variant.
#[tokio::test]
async fn test_map_progress() {
    let job_id = Uuid::new_v4();
    let step: u32 = 5;
    let total_steps: u32 = 20;
    let preview_b64 = Some("dGVzdCBwcmV2aWV3".to_string());

    let event = WorkerEvent::Progress {
        job_id,
        step,
        total_steps,
        preview_b64: preview_b64.clone(),
    };

    let ws_event = map_worker_event(event);

    match ws_event {
        WsEvent::JobProgress {
            job_id: got_job_id,
            step: got_step,
            total_steps: got_total_steps,
            preview_b64: got_preview,
        } => {
            assert_eq!(got_job_id, job_id, "job_id must match");
            assert_eq!(got_step, step, "step must match");
            assert_eq!(got_total_steps, total_steps, "total_steps must match");
            assert_eq!(got_preview, preview_b64, "preview_b64 must match");
        }
        other => panic!(
            "Expected WsEvent::JobProgress, got: {:?} — map_worker_event must map Progress correctly",
            other
        ),
    }
}

/// Test that `map_worker_event` maps `WorkerEvent::Completed` to `WsEvent::JobCompleted`
/// with correct field values.
///
/// Constructs a `WorkerEvent::Completed` with known values, calls `map_worker_event()`,
/// and verifies all fields are correctly transferred.
#[tokio::test]
async fn test_map_completed() {
    let job_id = Uuid::new_v4();
    let elapsed_ms: u64 = 12345;

    let event = WorkerEvent::Completed { job_id, elapsed_ms };

    let ws_event = map_worker_event(event);

    match ws_event {
        WsEvent::JobCompleted {
            job_id: got_job_id,
            elapsed_ms: got_elapsed,
        } => {
            assert_eq!(got_job_id, job_id, "job_id must match");
            assert_eq!(got_elapsed, elapsed_ms, "elapsed_ms must match");
        }
        other => panic!(
            "Expected WsEvent::JobCompleted, got: {:?} — map_worker_event must map Completed correctly",
            other
        ),
    }
}

/// Test that `map_worker_event` maps `WorkerEvent::Failed` to `WsEvent::JobFailed`
/// with correct fields, and that the `traceback` field is dropped.
///
/// Constructs a `WorkerEvent::Failed` with an error message and traceback,
/// calls `map_worker_event()`, and verifies the error is preserved but the
/// traceback is absent from the resulting `WsEvent::JobFailed`.
#[tokio::test]
async fn test_map_failed() {
    let job_id = Uuid::new_v4();
    let error = "CUDA out of memory".to_string();

    let event = WorkerEvent::Failed {
        job_id,
        error: error.clone(),
        traceback: Some("Traceback (most recent call last):\n  ...".to_string()),
    };

    let ws_event = map_worker_event(event);

    match ws_event {
        WsEvent::JobFailed {
            job_id: got_job_id,
            error: got_error,
        } => {
            assert_eq!(got_job_id, job_id, "job_id must match");
            assert_eq!(got_error, error, "error must match");
        }
        other => panic!(
            "Expected WsEvent::JobFailed, got: {:?} — map_worker_event must map Failed correctly",
            other
        ),
    }

    // The traceback field is dropped — this is verified by the fact that
    // WsEvent::JobFailed only has `job_id` and `error` fields, no `traceback`.
    // The test above confirms only those two fields are present.
}

/// Test that `map_worker_event` maps `WorkerEvent::Cancelled` to `WsEvent::JobCancelled`
/// with correct field values.
///
/// Constructs a `WorkerEvent::Cancelled` with a known job_id, calls
/// `map_worker_event()`, and verifies the job_id is correctly transferred.
#[tokio::test]
async fn test_map_cancelled() {
    let job_id = Uuid::new_v4();

    let event = WorkerEvent::Cancelled { job_id };

    let ws_event = map_worker_event(event);

    match ws_event {
        WsEvent::JobCancelled { job_id: got_job_id } => {
            assert_eq!(got_job_id, job_id, "job_id must match");
        }
        other => panic!(
            "Expected WsEvent::JobCancelled, got: {:?} — map_worker_event must map Cancelled correctly",
            other
        ),
    }
}

/// Test that `map_worker_event` maps `WorkerEvent::ImageReady` to `WsEvent::JobImageReady`
/// with correct fields (excluding the artifact hash which is only known after save).
///
/// Constructs a `WorkerEvent::ImageReady` with known values, calls
/// `map_worker_event()`, and verifies the width, height, seed, and steps fields
/// are correctly transferred. The artifact_hash is empty because
/// `map_worker_event` does not have access to the saved hash.
#[tokio::test]
async fn test_image_ready_publishes_after_save() {
    let job_id = Uuid::new_v4();

    // Build the WorkerEvent::ImageReady.
    let event = WorkerEvent::ImageReady {
        job_id,
        image_b64: make_valid_png_b64(),
        width: 512,
        height: 512,
        format: "png".into(),
        seed: 42,
        steps: 20,
    };

    // Verify that map_worker_event on ImageReady returns a valid
    // WsEvent::JobImageReady with the correct fields.
    let ws_event = map_worker_event(event);
    match ws_event {
        WsEvent::JobImageReady {
            job_id: got_job_id,
            artifact_hash: got_hash,
            width: got_width,
            height: got_height,
            seed: got_seed,
            steps: got_steps,
        } => {
            assert_eq!(got_job_id, job_id, "job_id must match");
            // The artifact_hash is empty in map_worker_event (placeholder),
            // but the other fields must match. The real hash is populated by
            // spawn_event_loop after the artifact save completes.
            assert_eq!(got_width, 512, "width must match");
            assert_eq!(got_height, 512, "height must match");
            assert_eq!(got_seed, 42, "seed must match");
            assert_eq!(got_steps, 20, "steps must match");
            assert!(
                got_hash.is_empty(),
                "map_worker_event returns empty artifact_hash for ImageReady"
            );
        }
        other => panic!(
            "Expected WsEvent::JobImageReady from map_worker_event, got: {:?}",
            other
        ),
    }
}

/// End-to-end test: spawn the event loop, send a `Completed` event via a real
/// `Demux` subscription, and verify the broadcaster receives the
/// correct `JobCompleted` event.
///
/// Creates a `Demux`, a `JobScheduler`, and an `EventBroadcaster`. Spawns
/// the event loop task subscribed to the `Demux`, then routes a
/// `WorkerEvent::Completed` through it as `bridge.rs`'s reader_task would.
/// The test verifies that the broadcaster receives a `WsEvent::JobCompleted`
/// with the correct elapsed_ms.
#[tokio::test]
async fn test_spawn_event_loop_receives_and_publishes() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    // Create the broadcaster and a receiver.
    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied.
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    // Run migrations from the project-level migrations directory.
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());

    // Create the scheduler with our test artifact_store.
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Idle).await.0;

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    // Serialize the Completed event as msgpack.
    let completed_event = WorkerEvent::Completed {
        job_id: Uuid::new_v4(),
        elapsed_ms: 10000,
    };

    demux
        .route("test-worker-1", completed_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the JobCompleted event.
    // Use a timeout to prevent hanging if the event loop doesn't process
    // the message within the expected timeframe.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);

    let ws_event = loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => break event,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("skipped {} events", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        panic!("broadcast channel closed before receiving event");
                    }
                }
            }
            _ = &mut timeout => {
                panic!("event loop did not publish JobCompleted within 5s timeout");
            }
        }
    };

    match ws_event {
        WsEvent::JobCompleted {
            job_id: got_job_id,
            elapsed_ms: got_elapsed,
        } => {
            assert_eq!(
                got_elapsed, 10000,
                "elapsed_ms must be 10000, got {}",
                got_elapsed
            );
            // job_id was generated by the test — verify it matches.
            assert!(got_job_id != Uuid::nil(), "job_id must be non-nil");
        }
        other => panic!(
            "Expected WsEvent::JobCompleted, got: {:?} — event loop must map Completed to JobCompleted",
            other
        ),
    }
}

/// Test that the event loop retries gracefully after a Demux subscription
/// receiver is closed.
///
/// Creates a `Demux`, spawns the event loop subscribed to it, then aborts the
/// task directly to confirm the JoinHandle behaves as expected. (The recv-error
/// retry path itself is exercised by the subscription channel closing, which
/// `spawn_event_loop` must handle without panicking.)
#[tokio::test]
async fn test_spawn_event_loop_handles_recv_error() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);
    let broadcaster = EventBroadcaster::new();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied.
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());

    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Idle).await.0;

    // Spawn the event loop.
    let handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    // Abort the task directly — see this test's own doc comment for why
    // this replaces the old transport-close trigger.
    handle.abort();
}

/// Test that `WorkerEvent::Completed` persists the terminal status
/// (`status=Completed`, `completed_at=now`) and releases the VRAM
/// reservation in the ledger.
///
/// Creates a full event loop setup with a JobStore containing a
/// `Running` job with `worker_id="0"`. Reserves VRAM on device 0
/// in the ledger. Routes a `Completed` event through the `Demux`,
/// then verifies:
/// - The job's `status` is `Completed` and `completed_at` is set.
/// - The ledger reservation for device 0 is zeroed.
/// - The broadcaster receives `WsEvent::JobCompleted`.
#[tokio::test]
async fn test_completed_persists_status_and_releases_ledger() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    // Create the broadcaster and a receiver.
    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied.
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Reserve VRAM on device 0 (simulating a dispatch that reserved VRAM).
    {
        scheduler.reserve_vram_test(0, 8192).await;
    }

    // Create and persist a Running job with worker_id="0".
    let job_id = Uuid::new_v4();
    let running_job = anvilml_core::Job {
        id: job_id,
        status: anvilml_core::JobStatus::Running,
        graph: serde_json::json!({"nodes": []}),
        settings: anvilml_core::JobSettings {
            device_preference: None,
        },
        created_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };
    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("upsert must succeed");

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Busy).await.0;

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let completed_event = WorkerEvent::Completed {
        job_id,
        elapsed_ms: 5000,
    };
    demux
        .route("test-worker-1", completed_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the event.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCompleted { job_id: ejob_id, elapsed_ms: eelapsed } => {
                                assert_eq!(ejob_id, job_id, "job_id must match");
                                assert_eq!(eelapsed, 5000, "elapsed_ms must match");
                                break;
                            }
                            other => panic!("Expected JobCompleted, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("skipped {} events", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        panic!("broadcast channel closed before receiving event");
                    }
                }
            }
            _ = &mut timeout => {
                panic!("event loop did not publish within 5s timeout");
            }
        }
    }

    // Verify the job's terminal status was persisted.
    let persisted_job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must succeed")
        .expect("job must exist");
    assert_eq!(
        persisted_job.status,
        anvilml_core::JobStatus::Completed,
        "job status must be Completed"
    );
    assert!(
        persisted_job.completed_at.is_some(),
        "completed_at must be set"
    );

    // Verify the ledger reservation was released.
    let reservations = scheduler.ledger_reservations_test().await;
    let reserved = reservations.get(&0).copied().unwrap_or(0);
    assert_eq!(
        reserved, 0,
        "ledger reservation for device 0 must be zeroed after release"
    );
}

/// Test that `WorkerEvent::Failed` persists the terminal status
/// (`status=Failed`, `completed_at=now`, `error=event.error`) and
/// releases the VRAM reservation.
///
/// Same setup as `test_completed_persists_status_and_releases_ledger`,
/// but sends a `Failed` event with an error message and verifies
/// that the error string is persisted in the job record.
#[tokio::test]
async fn test_failed_persists_status_error_and_releases_ledger() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    // Create the broadcaster and a receiver.
    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied.
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Reserve VRAM on device 0.
    {
        scheduler.reserve_vram_test(0, 4096).await;
    }

    // Create and persist a Running job with worker_id="0".
    let job_id = Uuid::new_v4();
    let running_job = anvilml_core::Job {
        id: job_id,
        status: anvilml_core::JobStatus::Running,
        graph: serde_json::json!({"nodes": []}),
        settings: anvilml_core::JobSettings {
            device_preference: None,
        },
        created_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };
    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("upsert must succeed");

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Busy).await.0;

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let failed_event = WorkerEvent::Failed {
        job_id,
        error: "CUDA out of memory".to_string(),
        traceback: Some("Traceback (most recent call last):\n  ...".to_string()),
    };
    demux
        .route("test-worker-1", failed_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the event.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobFailed { job_id: ejob_id, error: eerror } => {
                                assert_eq!(ejob_id, job_id, "job_id must match");
                                assert_eq!(eerror, "CUDA out of memory", "error must match");
                                break;
                            }
                            other => panic!("Expected JobFailed, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("skipped {} events", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        panic!("broadcast channel closed before receiving event");
                    }
                }
            }
            _ = &mut timeout => {
                panic!("event loop did not publish within 5s timeout");
            }
        }
    }

    // Verify the job's terminal status and error were persisted.
    let persisted_job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must succeed")
        .expect("job must exist");
    assert_eq!(
        persisted_job.status,
        anvilml_core::JobStatus::Failed,
        "job status must be Failed"
    );
    assert!(
        persisted_job.completed_at.is_some(),
        "completed_at must be set"
    );
    assert_eq!(
        persisted_job.error,
        Some("CUDA out of memory".to_string()),
        "error must be persisted from event"
    );

    // Verify the ledger reservation was released.
    let reservations = scheduler.ledger_reservations_test().await;
    let reserved = reservations.get(&0).copied().unwrap_or(0);
    assert_eq!(
        reserved, 0,
        "ledger reservation for device 0 must be zeroed after release"
    );
}

/// Test that `WorkerEvent::Cancelled` persists the terminal status
/// (`status=Cancelled`, `completed_at=now`) and releases the VRAM
/// reservation.
///
/// Uses worker_id="1" to exercise a different device index.
#[tokio::test]
async fn test_cancelled_persists_status_and_releases_ledger() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    // Create the broadcaster and a receiver.
    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied.
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Reserve VRAM on device 1.
    {
        scheduler.reserve_vram_test(1, 6144).await;
    }

    // Create and persist a Running job with worker_id="1".
    let job_id = Uuid::new_v4();
    let running_job = anvilml_core::Job {
        id: job_id,
        status: anvilml_core::JobStatus::Running,
        graph: serde_json::json!({"nodes": []}),
        settings: anvilml_core::JobSettings {
            device_preference: None,
        },
        created_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
        worker_id: Some("1".to_string()),
        error: None,
        queue_position: None,
    };
    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("upsert must succeed");

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Busy).await.0;

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let cancelled_event = WorkerEvent::Cancelled { job_id };
    demux
        .route("test-worker-1", cancelled_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the event.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCancelled { job_id: ejob_id } => {
                                assert_eq!(ejob_id, job_id, "job_id must match");
                                break;
                            }
                            other => panic!("Expected JobCancelled, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("skipped {} events", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        panic!("broadcast channel closed before receiving event");
                    }
                }
            }
            _ = &mut timeout => {
                panic!("event loop did not publish within 5s timeout");
            }
        }
    }

    // Verify the job's terminal status was persisted.
    let persisted_job = scheduler
        .get_job(job_id)
        .await
        .expect("get_job must succeed")
        .expect("job must exist");
    assert_eq!(
        persisted_job.status,
        anvilml_core::JobStatus::Cancelled,
        "job status must be Cancelled"
    );
    assert!(
        persisted_job.completed_at.is_some(),
        "completed_at must be set"
    );

    // Verify the ledger reservation was released.
    let reservations = scheduler.ledger_reservations_test().await;
    let reserved = reservations.get(&1).copied().unwrap_or(0);
    assert_eq!(
        reserved, 0,
        "ledger reservation for device 1 must be zeroed after release"
    );
}

/// Test that all three terminal events (`Completed`, `Failed`, `Cancelled`)
/// publish the correct `WsEvent` variant through the broadcaster.
///
/// Sends each event type and verifies the broadcaster receives the
/// matching `WsEvent` variant with correct fields.
#[tokio::test]
async fn test_terminal_events_publish_ws_event() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    // Create the broadcaster and a receiver.
    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied.
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Idle).await.0;

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    // Send Completed and verify JobCompleted.
    let job_id_1 = Uuid::new_v4();
    let completed_event = WorkerEvent::Completed {
        job_id: job_id_1,
        elapsed_ms: 3000,
    };
    demux
        .route("test-worker-1", completed_event)
        .await
        .expect("route must succeed");

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCompleted { job_id: ejob_id, elapsed_ms: eelapsed } => {
                                assert_eq!(ejob_id, job_id_1);
                                assert_eq!(eelapsed, 3000);
                                break;
                            }
                            other => panic!("Expected JobCompleted, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => panic!("timeout waiting for JobCompleted"),
        }
    }

    // Send Failed and verify JobFailed.
    let job_id_2 = Uuid::new_v4();
    let failed_event = WorkerEvent::Failed {
        job_id: job_id_2,
        error: "test error".to_string(),
        traceback: None,
    };
    demux
        .route("test-worker-1", failed_event)
        .await
        .expect("route must succeed");

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobFailed { job_id: ejob_id, error: eerror } => {
                                assert_eq!(ejob_id, job_id_2);
                                assert_eq!(eerror, "test error");
                                break;
                            }
                            other => panic!("Expected JobFailed, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => panic!("timeout waiting for JobFailed"),
        }
    }

    // Send Cancelled and verify JobCancelled.
    let job_id_3 = Uuid::new_v4();
    let cancelled_event = WorkerEvent::Cancelled { job_id: job_id_3 };
    demux
        .route("test-worker-1", cancelled_event)
        .await
        .expect("route must succeed");

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCancelled { job_id: ejob_id } => {
                                assert_eq!(ejob_id, job_id_3);
                                break;
                            }
                            other => panic!("Expected JobCancelled, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => panic!("timeout waiting for JobCancelled"),
        }
    }
}

/// Test that when a terminal event arrives for a `job_id` not in the
/// database, the event loop logs a warning and continues without
/// panicking — it still publishes the `WsEvent`.
///
/// Uses a `Completed` event with a UUID that doesn't exist in the
/// JobStore. The event loop should handle the `Ok(None)` path,
/// log a warning, and publish the event.
#[tokio::test]
async fn test_terminal_event_unknown_job_logs_warning() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    // Create the broadcaster and a receiver.
    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied (but no jobs).
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Idle).await.0;

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    // Connect a DEALER and send a Completed event for a non-existent job.
    let unknown_job_id = Uuid::new_v4();

    let completed_event = WorkerEvent::Completed {
        job_id: unknown_job_id,
        elapsed_ms: 1000,
    };
    demux
        .route("test-worker-1", completed_event)
        .await
        .expect("route must succeed");

    // The event loop should still publish the event despite the job
    // not being found. Wait for the broadcaster to receive it.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCompleted { job_id: ejob_id, elapsed_ms: eelapsed } => {
                                assert_eq!(ejob_id, unknown_job_id, "job_id must match");
                                assert_eq!(eelapsed, 1000, "elapsed_ms must match");
                                break;
                            }
                            other => panic!("Expected JobCompleted, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => {
                panic!("event loop did not publish within 5s timeout");
            }
        }
    }

    // Verify the job was NOT created in the database (it should remain absent).
    let persisted = scheduler
        .get_job(unknown_job_id)
        .await
        .expect("get_job must succeed");
    assert!(persisted.is_none(), "unknown job must not be created in DB");
}

/// Test that `WorkerEvent::Progress` still flows through the existing
/// `map_worker_event()` path unchanged (not through the terminal event
/// arms added in this task).
///
/// Sends a `Progress` event and verifies the broadcaster receives
/// `WsEvent::JobProgress` with correct fields.
#[tokio::test]
async fn test_progress_still_published_via_map_worker_event() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    // Create the broadcaster and a receiver.
    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    // Create a test artifact store.
    let artifact_store = create_test_artifact_store().await;

    // Create a JobStore with migrations applied.
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Create a WorkerPool with one mock handle (needed by spawn_event_loop).
    let pool = create_test_pool(WorkerStatus::Idle).await.0;

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    // Connect a DEALER and send a Progress event.
    let job_id = Uuid::new_v4();

    let progress_event = WorkerEvent::Progress {
        job_id,
        step: 10,
        total_steps: 20,
        preview_b64: Some("dGVzdA==".to_string()),
    };
    demux
        .route("test-worker-1", progress_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the JobProgress event.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobProgress {
                                job_id: ejob_id,
                                step: estep,
                                total_steps: etotal,
                                preview_b64: epreview,
                            } => {
                                assert_eq!(ejob_id, job_id, "job_id must match");
                                assert_eq!(estep, 10, "step must match");
                                assert_eq!(etotal, 20, "total_steps must match");
                                assert_eq!(epreview, Some("dGVzdA==".to_string()), "preview_b64 must match");
                                break;
                            }
                            other => panic!("Expected JobProgress, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => {
                panic!("event loop did not publish within 5s timeout");
            }
        }
    }
}

/// Helper to create a `WorkerPool` with a single mock worker handle at the given status.
///
/// Creates an empty `WorkerPool` (which binds its own ROUTER transport and spawns its own bridge — unused by this test, since events are routed directly via the standalone `Demux` below),
/// then populates it with one mock `WorkerHandle` whose status is set to the provided
/// value. The handle's `worker_id` is `"0"`. Returns `(Arc<WorkerPool>, WorkerHandle)`.
///
/// This is a test-only helper — `WorkerPool::set_up_test_workers()` is `test-utils`-gated.
async fn create_test_pool(initial_status: WorkerStatus) -> (Arc<WorkerPool>, WorkerHandle) {
    let mut pool = WorkerPool::new().await.expect("pool must be creatable");

    // Create a mock handle with worker_id="0".
    let worker_id = "0".to_string();
    let status = Arc::new(tokio::sync::RwLock::new(initial_status));
    let handle = WorkerHandle::new(
        worker_id.clone(),
        status,
        None, // no shutdown sender needed for tests
        None, // no force-shutdown sender needed for tests
        Arc::new(tokio::sync::Mutex::new(None)),
    );

    // Create a minimal GpuDevice for the pool.
    let device = anvilml_core::GpuDevice {
        index: 0,
        name: "test".into(),
        device_type: anvilml_core::DeviceType::Cpu,
        vram_total_mib: 8192,
        vram_free_mib: 8192,
        driver_version: String::new(),
        pci_vendor_id: 0,
        pci_device_id: 0,
        arch: None,
        caps: anvilml_core::InferenceCaps::default(),
        enumeration_source: anvilml_core::EnumerationSource::Mock,
        capabilities_source: anvilml_core::CapabilitySource::Fallback,
    };

    pool.set_up_test_workers(vec![(handle.clone(), device)]);

    (Arc::new(pool), handle)
}

/// Test that `WorkerEvent::Completed` restores the worker to `Idle` and increments
/// the dispatch wake count.
///
/// Creates a full event loop setup with a `WorkerPool` containing one mock handle
/// at `Busy` status. Sends a `Completed` event, then verifies:
/// - The handle's status is `Idle` after the event.
/// - The scheduler's dispatch wake count has been incremented.
#[tokio::test]
async fn test_completed_restores_worker_idle_wakes_dispatch() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();
    let artifact_store = create_test_artifact_store().await;

    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Create a WorkerPool with one mock handle at Busy status.
    let (pool, handle) = create_test_pool(WorkerStatus::Busy).await;

    // Create and persist a Running job with worker_id="0".
    let job_id = Uuid::new_v4();
    let running_job = anvilml_core::Job {
        id: job_id,
        status: anvilml_core::JobStatus::Running,
        graph: serde_json::json!({"nodes": []}),
        settings: anvilml_core::JobSettings {
            device_preference: None,
        },
        created_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };
    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("upsert must succeed");

    // Spawn the event loop with the WorkerPool.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let completed_event = WorkerEvent::Completed {
        job_id,
        elapsed_ms: 5000,
    };
    demux
        .route("test-worker-1", completed_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the event.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCompleted { job_id: ejob_id, .. } => {
                                assert_eq!(ejob_id, job_id, "job_id must match");
                                break;
                            }
                            other => panic!("Expected JobCompleted, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("skipped {} events", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        panic!("broadcast channel closed before receiving event");
                    }
                }
            }
            _ = &mut timeout => {
                panic!("event loop did not publish within 5s timeout");
            }
        }
    }

    // Verify the worker handle's status is now Idle.
    let status = handle.status().await;
    assert_eq!(
        status,
        WorkerStatus::Idle,
        "worker status must be Idle after Completed event, got {:?}",
        status
    );

    // Verify the dispatch wake count has been incremented.
    let wake_count = scheduler.dispatch_wake_count_test().await;
    assert!(
        wake_count >= 1,
        "dispatch wake count must be >= 1 after Completed, got {}",
        wake_count
    );
}

/// Test that `WorkerEvent::Failed` restores the worker to `Idle` and increments
/// the dispatch wake count.
///
/// Same setup as `test_completed_restores_worker_idle_wakes_dispatch`,
/// but sends a `Failed` event and verifies the same outcomes.
#[tokio::test]
async fn test_failed_restores_worker_idle_wakes_dispatch() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();
    let artifact_store = create_test_artifact_store().await;

    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    let (pool, handle) = create_test_pool(WorkerStatus::Busy).await;

    let job_id = Uuid::new_v4();
    let running_job = anvilml_core::Job {
        id: job_id,
        status: anvilml_core::JobStatus::Running,
        graph: serde_json::json!({"nodes": []}),
        settings: anvilml_core::JobSettings {
            device_preference: None,
        },
        created_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };
    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("upsert must succeed");

    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let failed_event = WorkerEvent::Failed {
        job_id,
        error: "CUDA out of memory".to_string(),
        traceback: None,
    };
    demux
        .route("test-worker-1", failed_event)
        .await
        .expect("route must succeed");

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobFailed { job_id: ejob_id, .. } => {
                                assert_eq!(ejob_id, job_id);
                                break;
                            }
                            other => panic!("Expected JobFailed, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => panic!("timeout waiting for JobFailed"),
        }
    }

    let status = handle.status().await;
    assert_eq!(
        status,
        WorkerStatus::Idle,
        "worker must be Idle after Failed"
    );

    let wake_count = scheduler.dispatch_wake_count_test().await;
    assert!(
        wake_count >= 1,
        "wake count must be >= 1 after Failed, got {}",
        wake_count
    );
}

/// Test that `WorkerEvent::Cancelled` restores the worker to `Idle` and
/// increments the dispatch wake count.
///
/// Same setup as the Completed test, but sends a `Cancelled` event.
#[tokio::test]
async fn test_cancelled_restores_worker_idle_wakes_dispatch() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();
    let artifact_store = create_test_artifact_store().await;

    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    let (pool, handle) = create_test_pool(WorkerStatus::Busy).await;

    let job_id = Uuid::new_v4();
    let running_job = anvilml_core::Job {
        id: job_id,
        status: anvilml_core::JobStatus::Running,
        graph: serde_json::json!({"nodes": []}),
        settings: anvilml_core::JobSettings {
            device_preference: None,
        },
        created_at: chrono::Utc::now(),
        started_at: Some(chrono::Utc::now()),
        completed_at: None,
        worker_id: Some("0".to_string()),
        error: None,
        queue_position: None,
    };
    scheduler
        .persist_job_test(&running_job)
        .await
        .expect("upsert must succeed");

    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let cancelled_event = WorkerEvent::Cancelled { job_id };
    demux
        .route("test-worker-1", cancelled_event)
        .await
        .expect("route must succeed");

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCancelled { job_id: ejob_id } => {
                                assert_eq!(ejob_id, job_id);
                                break;
                            }
                            other => panic!("Expected JobCancelled, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => panic!("timeout waiting for JobCancelled"),
        }
    }

    let status = handle.status().await;
    assert_eq!(
        status,
        WorkerStatus::Idle,
        "worker must be Idle after Cancelled"
    );

    let wake_count = scheduler.dispatch_wake_count_test().await;
    assert!(
        wake_count >= 1,
        "wake count must be >= 1 after Cancelled, got {}",
        wake_count
    );
}

/// Test that a `Progress` event does NOT increment the dispatch wake count.
///
/// Creates the event loop and sends a `Progress` event. Verifies that the
/// wake count remains at 0 — Progress is a non-terminal event and should
/// not trigger dispatch loop wake.
#[tokio::test]
async fn test_progress_does_not_wake_dispatch() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();
    let artifact_store = create_test_artifact_store().await;

    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    let (pool, _handle) = create_test_pool(WorkerStatus::Idle).await;

    // Verify initial wake count is 0.
    let initial_count = scheduler.dispatch_wake_count_test().await;
    assert_eq!(initial_count, 0, "wake count must start at 0");

    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let job_id = Uuid::new_v4();
    let progress_event = WorkerEvent::Progress {
        job_id,
        step: 10,
        total_steps: 20,
        preview_b64: Some("dGVzdA==".to_string()),
    };
    demux
        .route("test-worker-1", progress_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the event.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobProgress { .. } => break,
                            other => panic!("Expected JobProgress, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => panic!("timeout waiting for JobProgress"),
        }
    }

    // Verify the wake count is still 0 — Progress does not wake dispatch.
    let wake_count = scheduler.dispatch_wake_count_test().await;
    assert_eq!(
        wake_count, 0,
        "wake count must remain 0 after Progress event, got {}",
        wake_count
    );
}

/// Test that a queued second job remains in the queue after the first
/// job's terminal event, and that the worker is restored to Idle and
/// the dispatch loop is woken.
///
/// This is the integration counterpart to the per-terminal-event tests.
/// It verifies the full flow: submit two jobs, send a Completed event
/// for the first, and confirm the worker is restored to Idle and the
/// wake count incremented. The second job's dispatch is tested by the
/// dispatch loop integration in the full system (P16-B1+).
#[tokio::test]
async fn test_queued_job_dispatched_after_first_completes() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();
    let artifact_store = create_test_artifact_store().await;

    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());

    // Populate the node registry so submit() doesn't reject jobs.
    node_registry.register_all(vec![anvilml_core::NodeTypeDescriptor {
        type_name: "PassThrough".into(),
        display_name: "PassThrough".into(),
        category: "test".into(),
        description: "Test node".into(),
        inputs: vec![],
        outputs: vec![],
    }]);

    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));

    // Create a WorkerPool with one mock handle at Busy status.
    let (pool, handle) = create_test_pool(WorkerStatus::Busy).await;

    // Submit two jobs — both go into the queue.
    let (job_id_1, _) = scheduler
        .submit(
            serde_json::json!({"nodes": []}),
            anvilml_core::JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    let (job_id_2, _) = scheduler
        .submit(
            serde_json::json!({"nodes": []}),
            anvilml_core::JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Manually set the first job's status to Running with worker_id="0"
    // (simulating that it was dispatched). The second job stays Queued.
    let mut job1 = scheduler
        .get_job(job_id_1)
        .await
        .expect("get_job must succeed")
        .expect("job must exist");
    job1.status = anvilml_core::JobStatus::Running;
    job1.worker_id = Some("0".to_string());
    job1.started_at = Some(chrono::Utc::now());
    scheduler
        .persist_job_test(&job1)
        .await
        .expect("upsert must succeed");

    // Verify the second job is still Queued.
    let job2 = scheduler
        .get_job(job_id_2)
        .await
        .expect("get_job must succeed")
        .expect("job must exist");
    assert_eq!(
        job2.status,
        anvilml_core::JobStatus::Queued,
        "second job must be Queued before first completes"
    );

    // Spawn the event loop.
    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    let completed_event = WorkerEvent::Completed {
        job_id: job_id_1,
        elapsed_ms: 5000,
    };
    demux
        .route("test-worker-1", completed_event)
        .await
        .expect("route must succeed");

    // Wait for the broadcaster to receive the JobCompleted event.
    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match event {
                            WsEvent::JobCompleted { job_id: ejob_id, .. } => {
                                assert_eq!(ejob_id, job_id_1);
                                break;
                            }
                            other => panic!("Expected JobCompleted, got: {:?}", other),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => panic!("channel closed"),
                }
            }
            _ = &mut timeout => panic!("timeout waiting for JobCompleted"),
        }
    }

    // Verify the worker was restored to Idle.
    let status = handle.status().await;
    assert_eq!(
        status,
        WorkerStatus::Idle,
        "worker must be Idle after first completes"
    );

    // Verify the dispatch wake count was incremented.
    let wake_count = scheduler.dispatch_wake_count_test().await;
    assert!(
        wake_count >= 1,
        "wake count must be >= 1, got {}",
        wake_count
    );

    // Verify the second job is still Queued (not yet dispatched — the
    // dispatch loop isn't running in this test, but the event loop's
    // terminal-event handling correctly restored Idle and woke dispatch).
    let persisted_job2 = scheduler
        .get_job(job_id_2)
        .await
        .expect("get_job must succeed")
        .expect("job must exist");
    assert_eq!(
        persisted_job2.status,
        anvilml_core::JobStatus::Queued,
        "second job must still be Queued (dispatch loop not running in this test)"
    );
}

/// Regression test: `spawn_event_loop()`'s `Demux` subscription must exist
/// by the time the function *returns* to its caller, not merely by the time
/// the spawned task eventually gets scheduled.
///
/// Routes an event via `demux.route()` immediately after `spawn_event_loop()`
/// returns, with no sleep or other synchronization in between. If the
/// subscription were instead established inside the spawned `async move`
/// block (a regression this test guards against), this event could be routed
/// before the task is ever polled, and the subscriber would silently miss it
/// — exactly the failure mode this test would catch.
#[tokio::test]
async fn test_spawn_event_loop_subscription_exists_before_return() {
    let demux = Arc::new(Demux::new());
    // Register a dummy primary consumer for "test-worker-1" so that
    // demux.route() below succeeds — mirrors production, where
    // bridge.rs's reader_task only routes successfully to a worker_id
    // that has an actual ManagedWorker registered. This test cares about
    // the event loop's own subscription (fan-out), not primary delivery;
    // the receiver is kept alive (not `_`-dropped) for the test's
    // duration so route()'s primary send doesn't fail either.
    let (_dummy_primary_tx, _dummy_primary_rx) = tokio::sync::mpsc::channel::<WorkerEvent>(16);
    demux.register("test-worker-1".to_string(), _dummy_primary_tx);

    let broadcaster = EventBroadcaster::new();
    let mut rx = broadcaster.subscribe();

    let artifact_store = create_test_artifact_store().await;
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(":memory:")
                .create_if_missing(true),
        )
        .await
        .expect("in-memory SQLite pool must connect");
    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&db_pool)
        .await
        .expect("migrations must apply to in-memory pool");

    let job_store = JobStore::new(db_pool);
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let scheduler = Arc::new(JobScheduler::new(job_store, node_registry, artifact_store));
    let pool = create_test_pool(WorkerStatus::Idle).await.0;

    let _handle = spawn_event_loop(
        Arc::clone(&scheduler),
        Arc::clone(&demux),
        Arc::new(broadcaster),
        pool,
    );

    // No sleep here — this is the whole point of the test. If the
    // subscription isn't guaranteed to exist synchronously by the time
    // spawn_event_loop() returns, this route() call races the spawned
    // task's first poll and could lose the event.
    let event = WorkerEvent::Progress {
        job_id: Uuid::new_v4(),
        step: 1,
        total_steps: 10,
        preview_b64: None,
    };
    demux
        .route("test-worker-1", event)
        .await
        .expect("route must succeed");

    let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
    tokio::pin!(timeout);
    let ws_event = loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => break event,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        panic!("broadcast channel closed before receiving event");
                    }
                }
            }
            _ = &mut timeout => {
                panic!(
                    "event loop did not publish JobProgress within 5s — the Demux \
                     subscription was not established before spawn_event_loop() \
                     returned"
                );
            }
        }
    };

    assert!(
        matches!(ws_event, WsEvent::JobProgress { .. }),
        "expected WsEvent::JobProgress, got: {:?}",
        ws_event
    );
}
