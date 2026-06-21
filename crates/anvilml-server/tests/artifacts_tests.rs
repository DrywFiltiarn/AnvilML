//! Integration tests for the artifact endpoints: `GET /v1/artifacts` and
//! `GET /v1/artifacts/:hash`.
//!
//! These tests verify the artifact listing and serving handlers via a real
//! TCP listener, following the same pattern used in `handler_tests.rs`.
//! Tests use a real `ArtifactStore` so they exercise the full pipeline
//! from database metadata through the HTTP handler to the filesystem.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::NodeTypeRegistry;
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::ModelStore;
use anvilml_scheduler::{ledger::VramLedger, queue::JobQueue, scheduler::JobScheduler};
use anvilml_server::{build_router, AppState};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Build a JobScheduler and ArtifactStore for tests.
///
/// Reuses the same helper pattern from `handler_tests.rs` to construct
/// a fully wired `AppState` with in-memory database and artifact store.
async fn test_state(registry: Arc<NodeTypeRegistry>) -> (Arc<JobScheduler>, Arc<ArtifactStore>) {
    let pool = anvilml_registry::open_in_memory().await.unwrap();
    let artifact_dir = std::env::temp_dir().join("anvilml-test-artifacts");
    let artifact_store = Arc::new(ArtifactStore::new(artifact_dir, pool.clone()).await);
    let model_store = Arc::new(ModelStore::new(pool.clone()).await);
    let scheduler = Arc::new(JobScheduler::new(
        Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
        Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
        registry.clone(),
        pool,
        Arc::new(EventBroadcaster::new()),
        Arc::clone(&artifact_store),
        model_store,
        None, // cancellation requires a real worker pool
    ));
    (scheduler, artifact_store)
}

/// Send a raw HTTP request to the server and return the response body bytes.
///
/// Connects to the server via TCP, sends the raw HTTP request string,
/// reads the response, and parses the HTTP status line and headers
/// to extract the status code and content-type. Returns the raw body
/// bytes and the parsed status code.
async fn send_request(socket: &mut tokio::net::TcpStream, request: &str) -> (u16, String, Vec<u8>) {
    socket.write_all(request.as_bytes()).await.unwrap();
    socket.flush().await.unwrap();

    let mut buf = vec![0u8; 8192];
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut buf))
        .await
        .expect("server should respond within 5 seconds")
        .unwrap();

    let response = String::from_utf8_lossy(&buf[..n]);
    let status_code = parse_status_code(&response);
    let content_type = parse_content_type(&response);
    // For text responses (JSON), extract from the string. For binary
    // responses (PNG), use extract_body_raw on the raw buffer.
    let body = extract_body(&response);

    (status_code, content_type, body)
}

/// Send a raw HTTP request and return the raw response bytes.
///
/// Same as `send_request` but returns the full raw byte buffer
/// so binary data is not corrupted by UTF-8 lossy conversion.
async fn send_request_raw(
    socket: &mut tokio::net::TcpStream,
    request: &str,
) -> (u16, String, Vec<u8>) {
    socket.write_all(request.as_bytes()).await.unwrap();
    socket.flush().await.unwrap();

    let mut buf = vec![0u8; 8192];
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut buf))
        .await
        .expect("server should respond within 5 seconds")
        .unwrap();

    // Parse headers from the string representation for status and content-type.
    let response = String::from_utf8_lossy(&buf[..n]);
    let status_code = parse_status_code(&response);
    let content_type = parse_content_type(&response);

    // Extract body from raw bytes to preserve binary data integrity.
    let body = extract_body_raw(&buf[..n]);

    (status_code, content_type, body)
}

/// Parse the HTTP status code from a raw HTTP response string.
///
/// Extracts the three-digit status code from the status line
/// (e.g. "HTTP/1.1 200 OK\r\n" → 200).
fn parse_status_code(response: &str) -> u16 {
    let status_line = response.lines().next().unwrap_or("");
    status_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse::<u16>()
        .unwrap_or(0)
}

/// Parse the Content-Type header value from a raw HTTP response string.
fn parse_content_type(response: &str) -> String {
    response
        .lines()
        .find(|line| line.to_lowercase().starts_with("content-type:"))
        .map(|line| line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string())
        .unwrap_or_default()
}

/// Extract the response body from a raw HTTP response string.
///
/// The body starts after the first blank line ("\r\n\r\n") that
/// separates headers from the body. Returns the raw bytes.
fn extract_body(response: &str) -> Vec<u8> {
    // The response string was built from the raw bytes via from_utf8_lossy.
    // We find the header/body separator ("\r\n\r\n") and return the
    // raw bytes from that point onward. Since the response was constructed
    // from the original bytes, the byte positions are preserved even if
    // non-UTF8 bytes were replaced by the replacement character.
    //
    // However, for binary data (PNG), from_utf8_lossy has already
    // corrupted the bytes. We need to re-read from the original buffer.
    // The send_request function already has access to the full buf —
    // but since we're extracting from the string here, we need a
    // different approach. We'll use a separate function that takes
    // the raw buffer.
    //
    // For now, we keep this function for text responses (JSON) and
    // use extract_body_raw for binary responses.
    if let Some(body_start) = response.find("\r\n\r\n") {
        response[body_start + 4..].as_bytes().to_vec()
    } else {
        Vec::new()
    }
}

/// Extract the response body from raw HTTP response bytes.
///
/// The body starts after the first "\r\n\r\n" sequence.
fn extract_body_raw(buf: &[u8]) -> Vec<u8> {
    if let Some(body_start) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
        buf[body_start + 4..].to_vec()
    } else {
        Vec::new()
    }
}

/// Verify that listing artifacts on an empty store returns 200 with an empty JSON array.
///
/// Starts a real HTTP server with `AppState` containing an empty `ArtifactStore`,
/// sends `GET /v1/artifacts`, and asserts that the response is HTTP 200 with
/// `Content-Type: application/json` and body `[]`.
///
/// No preconditions — the server binds to a random OS-assigned port.
#[tokio::test]
async fn test_list_artifacts_empty() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;
    let router = build_router(state);

    let make_service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, make_service).await.unwrap();
    });

    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

    let request = format!(
        "GET /v1/artifacts HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Content-Type: application/json\r\n\
         \r\n",
        port = addr.port()
    );

    let (status, content_type, body) = send_request(&mut socket, &request).await;

    assert_eq!(status, 200, "expected HTTP 200, got {status}");
    assert_eq!(
        content_type, "application/json",
        "expected application/json content-type"
    );
    let body_str = String::from_utf8_lossy(&body);
    assert_eq!(
        body_str.trim(),
        "[]",
        "expected empty JSON array, got {body_str}"
    );

    server.abort();
    let _ = server.await;
}

/// Verify that listing artifacts with a job_id filter returns only matching artifacts.
///
/// Saves an artifact via `ArtifactStore`, then calls `GET /v1/artifacts?job_id=<id>`
/// and asserts that the response contains exactly one artifact with the matching
/// job_id. Also verifies that listing without a filter returns more than one
/// artifact when multiple artifacts exist.
///
/// Preconditions: An artifact has been saved via `ArtifactStore::save()`.
#[tokio::test]
async fn test_list_artifacts_filtered() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    // Clone the Arc so we can save artifacts after constructing AppState.
    // AppState::new() takes ownership of the Arc, but we need the store
    // for the save operations below. Cloning an Arc is a cheap reference
    // count increment, not a deep copy.
    let store_for_saving = artifact_store.clone();
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;
    let router = build_router(state);

    let make_service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, make_service).await.unwrap();
    });

    // Save an artifact via the store so we have data to query.
    // We create a minimal PNG-like byte sequence — the store only
    // hashes and persists it, it doesn't validate PNG structure.
    let job_id = uuid::Uuid::new_v4();
    let image_bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01";
    let _meta = store_for_saving.save(job_id, image_bytes).await.unwrap();

    // Save a second artifact with a different job_id.
    let job_id_2 = uuid::Uuid::new_v4();
    let image_bytes_2 = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x02";
    let _meta_2 = store_for_saving
        .save(job_id_2, image_bytes_2)
        .await
        .unwrap();

    // List with filter for the first job_id — should return exactly 1 artifact.
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "GET /v1/artifacts?job_id={job_id} HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         \r\n",
        job_id = job_id,
        port = addr.port()
    );

    let (status, _content_type, body) = send_request(&mut socket, &request).await;
    assert_eq!(status, 200, "expected HTTP 200, got {status}");
    let body_str = String::from_utf8_lossy(&body);
    // The response should be a JSON array with exactly one element.
    let parsed: serde_json::Value =
        serde_json::from_str(&body_str).expect("response should be valid JSON");
    assert_eq!(
        parsed.as_array().unwrap().len(),
        1,
        "expected exactly 1 artifact"
    );

    // List without filter — should return 2 artifacts.
    let mut socket2 = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request2 = format!(
        "GET /v1/artifacts HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         \r\n",
        port = addr.port()
    );

    let (status2, _content_type2, body2) = send_request(&mut socket2, &request2).await;
    assert_eq!(status2, 200, "expected HTTP 200, got {status2}");
    let body_str2 = String::from_utf8_lossy(&body2);
    let parsed2: serde_json::Value =
        serde_json::from_str(&body_str2).expect("response should be valid JSON");
    assert_eq!(
        parsed2.as_array().unwrap().len(),
        2,
        "expected 2 artifacts total"
    );

    server.abort();
    let _ = server.await;
}

/// Verify that serving an artifact returns 200 with `Content-Type: image/png`
/// and the correct body bytes.
///
/// Saves an artifact via `ArtifactStore`, then calls `GET /v1/artifacts/:hash`
/// and asserts that the response is HTTP 200 with `Content-Type: image/png`
/// and the body matches the original bytes.
///
/// Preconditions: An artifact has been saved via `ArtifactStore::save()`.
#[tokio::test]
async fn test_serve_artifact_returns_png() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    // Clone the Arc so we can save an artifact after constructing AppState.
    let store_for_saving = artifact_store.clone();
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;
    let router = build_router(state);

    let make_service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, make_service).await.unwrap();
    });

    // Save an artifact via the store.
    let job_id = uuid::Uuid::new_v4();
    let original_bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x99";
    let meta = store_for_saving.save(job_id, original_bytes).await.unwrap();
    let hash = meta.hash;

    // Serve the artifact by hash.
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "GET /v1/artifacts/{hash} HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         \r\n",
        hash = hash,
        port = addr.port()
    );

    // Use send_request_raw to avoid UTF-8 lossy corruption of binary body.
    let (status, content_type, body) = send_request_raw(&mut socket, &request).await;

    assert_eq!(status, 200, "expected HTTP 200, got {status}");
    assert_eq!(content_type, "image/png", "expected image/png content-type");
    assert!(!body.is_empty(), "body should not be empty");
    assert_eq!(
        body, original_bytes,
        "body should match the original artifact bytes"
    );

    server.abort();
    let _ = server.await;
}

/// Verify that serving a non-existent artifact returns 404 with the correct
/// error kind.
///
/// Calls `GET /v1/artifacts/<invalid_hash>` where the hash does not match
/// any saved artifact, and asserts that the response is HTTP 404 with
/// `error: "artifact_not_found"`.
///
/// No preconditions — the artifact store is empty.
#[tokio::test]
async fn test_serve_artifact_not_found() {
    let registry = Arc::new(NodeTypeRegistry::new().await);
    let (scheduler, artifact_store) = test_state(registry.clone()).await;
    let state = AppState::new("test-version", registry, scheduler, artifact_store).await;
    let router = build_router(state);

    let make_service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, make_service).await.unwrap();
    });

    // Request an artifact with a hash that does not exist.
    let fake_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let request = format!(
        "GET /v1/artifacts/{hash} HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         \r\n",
        hash = fake_hash,
        port = addr.port()
    );

    let (status, _content_type, body) = send_request(&mut socket, &request).await;

    assert_eq!(status, 404, "expected HTTP 404, got {status}");
    let body_str = String::from_utf8_lossy(&body);
    let parsed: serde_json::Value =
        serde_json::from_str(&body_str).expect("error response should be valid JSON");
    assert_eq!(
        parsed["error"], "artifact_not_found",
        "error kind should be artifact_not_found"
    );

    server.abort();
    let _ = server.await;
}
