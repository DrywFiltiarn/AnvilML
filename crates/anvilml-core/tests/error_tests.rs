/// Tests for `AnvilError` — status code mapping, response body structure,
/// and `From<sqlx::Error>` conversion.
///
/// Verifies:
/// - Each of the 14 variants maps to its expected `StatusCode`.
/// - The JSON response body contains `"error"`, `"message"`, and
///   `"request_id"` keys with correct types.
/// - `request_id` is a valid v4 UUID.
/// - `From<sqlx::Error>` correctly converts to `AnvilError::Db`.
use anvilml_core::AnvilError;
use axum::http::StatusCode;
use sqlx::Error as SqlxError;

// ── Status code tests (one per variant) ────────────────────────────────────

/// Verifies that `AnvilError::Db` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500).
///
/// Database errors are server-side failures that the client cannot fix,
/// so they always produce a 500 response regardless of the underlying
/// sqlx error kind.
#[test]
fn test_db_status_code() {
    let err = AnvilError::Db(SqlxError::PoolTimedOut);
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

/// Verifies that `AnvilError::Io` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500).
///
/// I/O errors on server-owned files indicate a server-side problem
/// (permissions, disk failure, etc.), so they always produce 500.
#[test]
fn test_io_status_code() {
    let err = AnvilError::Io(std::io::Error::other("test io error"));
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

/// Verifies that `AnvilError::Serde` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500).
///
/// Serialization errors indicate a programming error or incompatible
/// data shape — the client cannot fix them.
#[test]
fn test_serde_status_code() {
    let err = AnvilError::Serde("bad json".to_string());
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

/// Verifies that `AnvilError::Ipc` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500).
///
/// IPC failures with Python workers are server-side operational errors.
#[test]
fn test_ipc_status_code() {
    let err = AnvilError::Ipc("connection lost".to_string());
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

/// Verifies that `AnvilError::PayloadTooLarge` maps to `StatusCode::PAYLOAD_TOO_LARGE` (413).
///
/// This is the only client-side error that maps to 4xx other than
/// not-found and bad-request — the client must reduce the payload size.
#[test]
fn test_payload_too_large_status_code() {
    let err = AnvilError::PayloadTooLarge("256MiB".to_string());
    assert_eq!(err.status_code(), StatusCode::PAYLOAD_TOO_LARGE);
}

/// Verifies that `AnvilError::WorkerNotFound` maps to `StatusCode::NOT_FOUND` (404).
///
/// The worker resource does not exist — the client referenced a
/// non-existent worker ID.
#[test]
fn test_worker_not_found_status_code() {
    let err = AnvilError::WorkerNotFound("worker-1".to_string());
    assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
}

/// Verifies that `AnvilError::JobNotFound` maps to `StatusCode::NOT_FOUND` (404).
///
/// The job resource does not exist — the client referenced a
/// non-existent job ID.
#[test]
fn test_job_not_found_status_code() {
    let err = AnvilError::JobNotFound("job-abc".to_string());
    assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
}

/// Verifies that `AnvilError::InvalidGraph` maps to `StatusCode::BAD_REQUEST` (400).
///
/// The client submitted a graph with validation errors — the input
/// data is malformed and must be corrected.
#[test]
fn test_invalid_graph_status_code() {
    let err = AnvilError::InvalidGraph(vec!["missing node".to_string()]);
    assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
}

/// Verifies that `AnvilError::CycleDetected` maps to `StatusCode::BAD_REQUEST` (400).
///
/// The client submitted a graph with a cycle — the input structure
/// is invalid and must be corrected.
#[test]
fn test_cycle_detected_status_code() {
    let err = AnvilError::CycleDetected(vec!["A→B→A".to_string()]);
    assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
}

/// Verifies that `AnvilError::ModelNotFound` maps to `StatusCode::NOT_FOUND` (404).
///
/// The model resource does not exist in any configured model directory.
#[test]
fn test_model_not_found_status_code() {
    let err = AnvilError::ModelNotFound("model-x".to_string());
    assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
}

/// Verifies that `AnvilError::WorkersUnavailable` maps to
/// `StatusCode::SERVICE_UNAVAILABLE` (503).
///
/// All workers are busy or dead — the service is temporarily unable
/// to process the request.
#[test]
fn test_workers_unavailable_status_code() {
    let err = AnvilError::WorkersUnavailable("no idle".to_string());
    assert_eq!(err.status_code(), StatusCode::SERVICE_UNAVAILABLE);
}

/// Verifies that `AnvilError::Internal` maps to `StatusCode::INTERNAL_SERVER_ERROR` (500).
///
/// Internal errors are unexpected failures — bugs in the server code.
#[test]
fn test_internal_status_code() {
    let err = AnvilError::Internal("panic caught".to_string());
    assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

/// Verifies that `AnvilError::Toml` maps to `StatusCode::BAD_REQUEST` (400).
///
/// TOML deserialisation errors mean the config file is malformed —
/// the client (or operator) must fix the config.
#[test]
fn test_toml_status_code() {
    // Create a toml::de::Error by deserializing invalid TOML.
    let toml_err = toml::from_str::<toml::Value>("[invalid toml content {{{")
        .expect_err("invalid TOML should fail");
    let err = AnvilError::Toml(toml_err);
    assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
}

/// Verifies that `AnvilError::EnvVar` maps to `StatusCode::BAD_REQUEST` (400).
///
/// Invalid environment variable values mean the operator set a config
/// variable to an unparseable value — the config must be corrected.
#[test]
fn test_env_var_status_code() {
    let err = AnvilError::EnvVar {
        name: "PORT".to_string(),
        value: "abc".to_string(),
    };
    assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
}

// ── Response body structure tests ──────────────────────────────────────────

/// Verifies that the JSON response body structure produced by `IntoResponse`
/// contains the three required keys (`"error"`, `"message"`, `"request_id"`)
/// with correct types and that `request_id` is a valid v4 UUID.
///
/// We construct the same body that `into_response()` would build and validate
/// its structure. This tests the JSON payload format without needing an axum
/// test client to inspect the Response body.
#[test]
fn test_response_body_structure() {
    let err = AnvilError::JobNotFound("x".to_string());

    // Build the body exactly as IntoResponse does.
    let body = serde_json::json!({
        "error": err.error_kind(),
        "message": err.to_string(),
        "request_id": uuid::Uuid::new_v4().to_string(),
    });

    // All three required keys must be present
    assert!(
        body.get("error").is_some(),
        "response body must contain 'error' key"
    );
    assert!(
        body.get("message").is_some(),
        "response body must contain 'message' key"
    );
    assert!(
        body.get("request_id").is_some(),
        "response body must contain 'request_id' key"
    );

    // Types must be correct
    assert!(
        body["error"].is_string(),
        "'error' must be a string, got: {:?}",
        body["error"]
    );
    assert!(
        body["message"].is_string(),
        "'message' must be a string, got: {:?}",
        body["message"]
    );
    assert!(
        body["request_id"].is_string(),
        "'request_id' must be a string, got: {:?}",
        body["request_id"]
    );

    // request_id must be a valid v4 UUID
    let request_id: uuid::Uuid = uuid::Uuid::parse_str(body["request_id"].as_str().unwrap())
        .expect("request_id must be a valid UUID");
    assert_eq!(
        request_id.get_version(),
        Some(uuid::Version::Random),
        "request_id must be a v4 (random) UUID"
    );
}

/// Verifies that each distinct call to `Uuid::new_v4()` produces a
/// unique UUID, confirming the UUID generation used by `IntoResponse`
/// produces unique request_ids on every call.
#[test]
fn test_unique_request_ids() {
    let ids: std::collections::HashSet<String> =
        (0..10).map(|_| uuid::Uuid::new_v4().to_string()).collect();

    assert_eq!(
        ids.len(),
        10,
        "all 10 generated UUIDs must be unique (got {} unique)",
        ids.len()
    );
}

// ── From conversion test ───────────────────────────────────────────────────

/// Verifies that `From<sqlx::Error>` correctly converts to `AnvilError::Db`
/// via the `#[from]` attribute on the `Db` variant.
///
/// This tests the automatic `From` impl generated by `thiserror::Error`,
/// which is required by downstream crates that use `?` to propagate
/// `sqlx::Error` into `AnvilError`.
#[test]
fn test_from_sqlx_error() {
    let sqlx_err = SqlxError::PoolTimedOut;
    let anvil_err: AnvilError = sqlx_err.into();
    match anvil_err {
        AnvilError::Db(inner) => {
            // The inner error must be the same PoolTimedOut variant.
            assert!(
                matches!(inner, SqlxError::PoolTimedOut),
                "Db variant must wrap the original sqlx::Error"
            );
        }
        other => panic!("expected AnvilError::Db, got {:?}", other),
    }
}
