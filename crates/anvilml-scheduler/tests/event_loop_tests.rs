/// Integration tests for `handle_image_ready()` — the event loop handler
/// that persists `WorkerEvent::ImageReady` payloads to the artifact store.
///
/// Tests verify base64 decoding, artifact persistence, metadata field
/// correctness, error handling for malformed base64, and empty payload handling.
use std::sync::Arc;

use anvilml_artifacts::ArtifactStore;
use anvilml_core::AnvilError;
use anvilml_ipc::WorkerEvent;
use anvilml_scheduler::event_loop::handle_image_ready;
use base64::Engine as _;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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
