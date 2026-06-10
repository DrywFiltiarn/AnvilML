//! Integration tests for `GET /v1/artifacts/:hash`.
//!
//! Verifies the full artifact serve pipeline: save an artifact via
//! `ArtifactStore::save()`, then retrieve it via the HTTP endpoint.

use std::sync::Arc;

use anvilml_registry::open_in_memory;
use anvilml_server::artifact::{ArtifactStore, ArtifactStoreInput};
use anvilml_server::{build_router, App, EventBroadcaster};
use axum::{
    http::{Request, StatusCode},
    Router,
};
use base64::prelude::BASE64_STANDARD;
use base64::Engine as _;
use bytes::Bytes;
use http_body_util::Full;
use serde_json::Value;
use sha2::Digest as _;
use tempfile::TempDir;
use tower::ServiceExt;

/// A minimal valid 1×1 transparent PNG, base64-encoded.
const MINIMAL_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Build a fresh test environment with a temporary artifact directory
/// and an in-memory SQLite database with all migrations applied.
async fn setup_test_env() -> (TempDir, ArtifactStore, String) {
    let tmp = TempDir::new().expect("create temp dir");
    let artifact_dir = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact dir");

    let pool = open_in_memory().await.expect("open in-memory db");

    // Insert a placeholder job row.
    let job_id = "test-job-001".to_string();
    sqlx::query(
        "INSERT INTO jobs (id, status, graph, settings, artifact_count, created_at) \
         VALUES (?, 'Queued', '{}', '{}', 0, strftime('%s','now'))",
    )
    .bind(&job_id)
    .execute(&pool)
    .await
    .expect("insert test job");

    let store = ArtifactStore::new(artifact_dir, pool);

    (tmp, store, job_id)
}

/// Build a test app router with the given artifact store.
async fn build_artifact_app(store: ArtifactStore) -> Router {
    let broadcaster = Arc::new(EventBroadcaster::new(16));
    let state = App::new(
        "0.1.0",
        None,
        None,
        None,
        broadcaster,
        None,
        None,
        store,
        anvilml_core::ServerConfig::default(),
    );
    build_router(state)
}

/// GET /v1/artifacts/{nonexistent} must return 404 with artifact_not_found error.
#[tokio::test]
async fn artifact_serve_404_when_missing() {
    let (_tmp, store, _job_id) = setup_test_env().await;
    let app = build_artifact_app(store).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/artifacts/nonexistent")
                .body(Full::<Bytes>::default())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let parsed: Value = serde_json::from_str(&body_str).unwrap();

    assert_eq!(parsed["error"], "artifact_not_found");
    assert_eq!(parsed["message"], "artifact not found");
}

/// GET /v1/artifacts/{hash} must return 200 with correct headers and bytes.
#[tokio::test]
async fn artifact_serve_200_with_headers() {
    let (_tmp, store, job_id) = setup_test_env().await;

    // Save an artifact first.
    let meta_input = ArtifactStoreInput {
        width: 512,
        height: 512,
        seed: 42,
        steps: 20,
        prompt: "a cat".to_string(),
    };
    let meta = store
        .save(&job_id, MINIMAL_PNG_B64, meta_input)
        .await
        .expect("save should succeed");

    let app = build_artifact_app(store).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/v1/artifacts/{}", meta.hash))
                .body(Full::<Bytes>::default())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check headers.
    let headers = response.headers();
    assert_eq!(
        headers.get("content-type").unwrap(),
        "image/png",
        "Content-Type must be image/png"
    );
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "public, immutable, max-age=31536000",
        "Cache-Control must match"
    );
    let expected_etag = format!("\"{}\"", meta.hash);
    assert_eq!(
        headers.get("etag").unwrap().to_str().unwrap(),
        expected_etag,
        "ETag must match hash in quotes"
    );

    // Check body bytes match the original PNG.
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let expected_bytes = BASE64_STANDARD.decode(MINIMAL_PNG_B64).unwrap();
    assert_eq!(
        body_bytes.to_vec(),
        expected_bytes,
        "response body must match saved PNG bytes"
    );
}

/// GET /v1/artifacts/{hash} must return correct bytes matching the saved artifact.
#[tokio::test]
async fn artifact_serve_returns_correct_bytes() {
    let (_tmp, store, job_id) = setup_test_env().await;

    // Save an artifact.
    let meta_input = ArtifactStoreInput {
        width: 256,
        height: 256,
        seed: 99,
        steps: 10,
        prompt: "a dog".to_string(),
    };
    let meta = store
        .save(&job_id, MINIMAL_PNG_B64, meta_input)
        .await
        .expect("save should succeed");

    let app = build_artifact_app(store).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/v1/artifacts/{}", meta.hash))
                .body(Full::<Bytes>::default())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    // Compute expected hash to verify we're serving the right file.
    let expected_bytes = BASE64_STANDARD.decode(MINIMAL_PNG_B64).unwrap();
    let expected_hash = hex::encode(sha2::Sha256::digest(&expected_bytes));

    assert_eq!(meta.hash, expected_hash, "hash must match SHA-256 of PNG");
    assert_eq!(
        body_bytes.to_vec(),
        expected_bytes,
        "served bytes must match original PNG"
    );
}
