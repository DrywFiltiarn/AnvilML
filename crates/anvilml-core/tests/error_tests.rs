//! Tests for `AnvilError::IntoResponse` — every variant maps to the correct HTTP
//! status code and structured JSON body.
//!
//! Each test constructs a single `AnvilError` variant, calls `.into_response()`,
//! asserts the `StatusCode`, and parses the JSON body to verify the `error` and
//! `message` fields.

use anvilml_core::AnvilError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

/// Helper: extract the JSON body from a Response as a `serde_json::Value`.
///
/// Consumes the response, reads the body as bytes, converts to a string,
/// then parses as JSON.
async fn body_as_value(resp: Response) -> serde_json::Value {
    let (_parts, body) = resp.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("failed to read response body");
    let text = String::from_utf8(bytes.to_vec()).expect("response body is UTF-8");
    serde_json::from_str(&text).expect("response body is valid JSON")
}

// ---------------------------------------------------------------------------
// Status-code tests — one per variant
// ---------------------------------------------------------------------------

/// `AnvilError::Db` maps to HTTP 500 (Internal Server Error).
#[tokio::test]
async fn test_db_returns_500() {
    let err = AnvilError::Db(sqlx::Error::PoolClosed);
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "database_error");
    assert!(body["message"].is_string());
    assert!(body["request_id"].is_string());
}

/// `AnvilError::Io` maps to HTTP 500 (Internal Server Error).
#[tokio::test]
async fn test_io_returns_500() {
    let err = AnvilError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "nope"));
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "io_error");
}

/// `AnvilError::Serde` maps to HTTP 400 (Bad Request).
#[tokio::test]
async fn test_serde_returns_400() {
    let err = AnvilError::Serde("bad json".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "serde_error");
}

/// `AnvilError::Ipc` maps to HTTP 400 (Bad Request).
#[tokio::test]
async fn test_ipc_returns_400() {
    let err = AnvilError::Ipc("timeout".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "ipc_error");
}

/// `AnvilError::PayloadTooLarge` maps to HTTP 413 (Payload Too Large).
#[tokio::test]
async fn test_payload_too_large_returns_413() {
    let err = AnvilError::PayloadTooLarge("1GB".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "payload_too_large");
}

/// `AnvilError::WorkerNotFound` maps to HTTP 404 (Not Found).
#[tokio::test]
async fn test_worker_not_found_returns_404() {
    let err = AnvilError::WorkerNotFound("gpu:0".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "worker_not_found");
}

/// `AnvilError::JobNotFound` maps to HTTP 404 (Not Found).
#[tokio::test]
async fn test_job_not_found_returns_404() {
    let err = AnvilError::JobNotFound("job-xyz".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "job_not_found");
}

/// `AnvilError::InvalidGraph` maps to HTTP 400 (Bad Request).
#[tokio::test]
async fn test_invalid_graph_returns_400() {
    let err = AnvilError::InvalidGraph(vec!["missing input".to_string()]);
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "invalid_graph");
}

/// `AnvilError::CycleDetected` maps to HTTP 400 (Bad Request).
#[tokio::test]
async fn test_cycle_detected_returns_400() {
    let err = AnvilError::CycleDetected(vec!["A->B->A".to_string()]);
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "cycle_detected");
}

/// `AnvilError::ModelNotFound` maps to HTTP 404 (Not Found).
#[tokio::test]
async fn test_model_not_found_returns_404() {
    let err = AnvilError::ModelNotFound("flux2klein4b".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "model_not_found");
}

/// `AnvilError::ArtifactNotFound` maps to HTTP 404 (Not Found).
#[tokio::test]
async fn test_artifact_not_found_returns_404() {
    let err = AnvilError::ArtifactNotFound("abc123".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "artifact_not_found");
}

/// `AnvilError::WorkersUnavailable` maps to HTTP 503 (Service Unavailable).
#[tokio::test]
async fn test_workers_unavailable_returns_503() {
    let err = AnvilError::WorkersUnavailable("no gpu".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "workers_unavailable");
}

/// `AnvilError::Internal` maps to HTTP 500 (Internal Server Error).
#[tokio::test]
async fn test_internal_returns_500() {
    let err = AnvilError::Internal("panic".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = body_as_value(resp).await;
    assert_eq!(body["error"], "internal_error");
}

// ---------------------------------------------------------------------------
// Structural tests
// ---------------------------------------------------------------------------

/// Every response body contains a valid UUID v4 in the `request_id` field.
#[tokio::test]
async fn test_error_body_has_request_id() {
    let err = AnvilError::Serde("test".to_string());
    let resp = err.into_response();

    let body = body_as_value(resp).await;
    let id_str = body["request_id"].as_str().expect("request_id is a string");
    // Verify it parses as a valid UUID.
    let _uuid: Uuid = Uuid::parse_str(id_str).expect("request_id is a valid UUID");
}

/// The `message` field contains the variant's error description.
#[tokio::test]
async fn test_error_body_message_contains_variant_info() {
    let err = AnvilError::WorkerNotFound("gpu:0".to_string());
    let resp = err.into_response();

    let body = body_as_value(resp).await;
    let msg = body["message"].as_str().expect("message is a string");
    assert!(
        msg.contains("gpu:0"),
        "message should contain the worker ID: {msg}"
    );
}

/// The `error` field is always lowercase snake_case.
#[tokio::test]
async fn test_error_field_is_snake_case() {
    let variants: Vec<AnvilError> = vec![
        AnvilError::Db(sqlx::Error::PoolClosed),
        AnvilError::Io(std::io::Error::new(std::io::ErrorKind::Other, "x")),
        AnvilError::Serde("x".into()),
        AnvilError::Ipc("x".into()),
        AnvilError::PayloadTooLarge("x".into()),
        AnvilError::WorkerNotFound("x".into()),
        AnvilError::JobNotFound("x".into()),
        AnvilError::InvalidGraph(vec!["x".into()]),
        AnvilError::CycleDetected(vec!["x".into()]),
        AnvilError::ModelNotFound("x".into()),
        AnvilError::ArtifactNotFound("x".into()),
        AnvilError::WorkersUnavailable("x".into()),
        AnvilError::Internal("x".into()),
    ];

    for err in variants {
        let resp = err.into_response();
        let body = body_as_value(resp).await;
        let error_kind = body["error"].as_str().expect("error is a string");
        // Snake-case check: only lowercase letters and underscores, not empty.
        assert!(
            !error_kind.is_empty() && error_kind.chars().all(|c| c.is_lowercase() || c == '_'),
            "error field must be lowercase snake_case, got: {error_kind}",
        );
    }
}
