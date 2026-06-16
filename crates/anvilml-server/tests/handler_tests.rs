//! Integration tests for the WebSocket events handler.
//!
//! These tests verify that the `/v1/events` route exists, accepts
//! WebSocket upgrades, and delivers broadcast events to connected clients.
//!
//! Tests use a real TCP listener because axum's `WebSocketUpgrade`
//! extractor requires the `hyper::upgrade::OnUpgrade` extension which
//! is only set up when the server processes a real HTTP connection.
//! `Router::oneshot` does not set up this extension, so we use
//! `axum::serve` with a `TcpListener` for these tests.

use anvilml_core::types::WsEvent;
use anvilml_server::{build_router, AppState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Verify that the `/v1/events` route exists and returns HTTP 101
/// on a WebSocket upgrade request.
///
/// Starts a real HTTP server on a random port using `axum::serve`,
/// connects with a raw HTTP request containing WebSocket upgrade
/// headers, and asserts that the server responds with 101 Switching
/// Protocols.
///
/// No preconditions — the server binds to a random OS-assigned port.
#[tokio::test]
async fn test_events_route_returns_101() {
    let state = AppState::new("test-version").await;
    let router = build_router(state);

    // Convert the Router into a make-service with ConnectInfo support.
    // This is required because the ws_events handler uses `ConnectInfo`
    // to extract the client's socket address.
    let make_service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();

    // Bind to a random port and start the server.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn the server in a background task.
    let server = tokio::spawn(async move {
        axum::serve(listener, make_service).await.unwrap();
    });

    // Send a raw HTTP request with WebSocket upgrade headers.
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

    let request = format!(
        "GET /v1/events HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n",
        port = addr.port()
    );

    socket.write_all(request.as_bytes()).await.unwrap();
    socket.flush().await.unwrap();

    // Read the response.
    let mut buf = vec![0u8; 4096];
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut buf))
        .await
        .expect("server should respond within 5 seconds")
        .unwrap();

    let response = String::from_utf8_lossy(&buf[..n]);
    // Parse the status line: "HTTP/1.1 101 Switching Protocols\r\n"
    let status_line = response.lines().next().unwrap_or("");
    assert!(
        status_line.contains("101"),
        "expected HTTP 101 Switching Protocols, got: {status_line}"
    );

    // Drop the server task to shut down the listener.
    server.abort();
    let _ = server.await;
}

/// Verify that a WebSocket client connected to `/v1/events` receives
/// broadcast events as JSON text frames.
///
/// Starts a real HTTP server, connects with a raw HTTP request,
/// verifies the 101 upgrade, then broadcasts a `WsEvent::SystemStats`
/// through the broadcaster and verifies the client receives it as a
/// WebSocket text frame.
///
/// The test confirms the end-to-end path: broadcaster → handler
/// subscription → JSON serialization → WebSocket text frame → client.
///
/// Preconditions: A router built with `AppState::new()` containing an
/// `EventBroadcaster`.
#[tokio::test]
async fn test_events_delivers_broadcast_event() {
    let state = AppState::new("test-version").await;
    let router = build_router(state.clone());

    // Convert the Router into a make-service with ConnectInfo support.
    let make_service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();

    // Bind to a random port and start the server.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn the server in a background task.
    let server = tokio::spawn(async move {
        axum::serve(listener, make_service).await.unwrap();
    });

    // Connect and perform the WebSocket upgrade handshake.
    let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

    let request = format!(
        "GET /v1/events HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n",
        port = addr.port()
    );

    socket.write_all(request.as_bytes()).await.unwrap();
    socket.flush().await.unwrap();

    // Read the 101 response.
    let mut buf = vec![0u8; 4096];
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut buf))
        .await
        .expect("server should respond within 5 seconds")
        .unwrap();

    let response = String::from_utf8_lossy(&buf[..n]);
    let status_line = response.lines().next().unwrap_or("");
    assert!(
        status_line.contains("101"),
        "expected HTTP 101 Switching Protocols, got: {status_line}"
    );

    // Broadcast a SystemStats event through the shared broadcaster.
    // The handler's subscriber will receive this event, serialize it
    // to JSON, and send it as a text frame over the WebSocket.
    state.broadcaster.send(WsEvent::SystemStats {
        cpu_pct: 42.5,
        ram_used_mib: 8192,
        workers: vec![],
    });

    // Read the WebSocket message from the client socket.
    // WebSocket text frames have a specific binary format:
    // - Byte 0: FIN bit + opcode (0x81 = text frame)
    // - Byte 1: MASK bit + payload length
    // - (Bytes 2-5: mask key, if masked — not masked for server→client)
    // - Payload: the JSON string
    //
    // The server does NOT mask frames (masking is required for
    // client→server frames per RFC 6455 §5.1).
    let mut msg_buf = vec![0u8; 4096];
    let msg_n = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut msg_buf))
        .await
        .expect("client should receive the broadcast event within 5 seconds")
        .unwrap();

    // Skip the WebSocket frame header (2 bytes):
    // - Byte 0: FIN + opcode (0x81 = text frame)
    // - Byte 1: MASK + payload length (0x47 = 71 bytes)
    // The payload starts at byte 2 for unmasked server→client frames.
    let payload = &msg_buf[2..msg_n];
    let msg = String::from_utf8_lossy(payload);

    // Parse the received JSON and verify the event type.
    let parsed: serde_json::Value =
        serde_json::from_str(&msg).expect("received frame should be valid JSON");

    assert_eq!(
        parsed["type"], "system_stats",
        "received event should have type \"system_stats\""
    );

    // Verify the payload fields.
    assert_eq!(
        parsed["cpu_pct"], 42.5,
        "cpu_pct should match the broadcast event"
    );
    assert_eq!(
        parsed["ram_used_mib"], 8192,
        "ram_used_mib should match the broadcast event"
    );

    // Drop the server task to shut down the listener.
    server.abort();
    let _ = server.await;
}
