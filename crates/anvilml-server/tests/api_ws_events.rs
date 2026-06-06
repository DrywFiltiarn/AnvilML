//! Integration test for the WebSocket `/v1/events` endpoint.

use std::net::SocketAddr;
use std::sync::Arc;

use anvilml_core::types::events::{SystemStatsEvent, WsEvent};
use axum::Router;
use futures_util::StreamExt;

use anvilml_server::{build_router, AppState, EventBroadcaster};

/// Bind the axum app on a random port (127.0.0.1:0), connect a tungstenite WS
/// client to `/v1/events`, broadcast a `SystemStatsEvent` via the broadcaster,
/// then read from the WS client and assert the received frame is valid JSON
/// text containing `"event":"system.stats"`.
#[tokio::test]
async fn ws_connect_broadcast_receive() {
    let broadcaster = Arc::new(EventBroadcaster::new(16));
    let state = AppState::new("0.1.0", None, None, None, broadcaster.clone(), None);
    let app: Router = build_router(state);

    // Bind the server on a random port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind random port");
    let addr: SocketAddr = listener.local_addr().expect("get local addr");

    // Spawn the axum server in the background.
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve must succeed");
    });

    // Give the server a tick to bind.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Build the WebSocket URI.
    let uri = format!("ws://{addr}/v1/events");

    // Connect a tungstenite client.
    let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&uri)
        .await
        .expect("connect to WS endpoint");

    // Broadcast a test event after a short delay.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let event = WsEvent::SystemStats(SystemStatsEvent {
        event: "system.stats".to_string(),
        timestamp: chrono::Utc::now(),
        gpus: vec![],
        ram_used_mib: 0,
        ram_total_mib: 0,
    });
    broadcaster.send(event);

    // Read from the WS client with a timeout.
    let msg = tokio::time::timeout(std::time::Duration::from_secs(3), ws_stream.next())
        .await
        .expect("read message within timeout");

    let received_text = match msg {
        Some(Ok(msg)) => msg
            .into_text()
            .expect("received message must be valid UTF-8 text"),
        Some(Err(e)) => panic!("WS read error: {e}"),
        None => panic!("WebSocket stream closed unexpectedly"),
    };

    // Validate received text.
    assert!(
        received_text.contains(r#""event":"system.stats""#),
        "received JSON must contain event name system.stats: {received_text}"
    );

    // Parse as JSON to verify it's valid JSON.
    let parsed: serde_json::Value =
        serde_json::from_str(&received_text).expect("received text must be valid JSON");
    // WsEvent serializes as {"SystemStats": {...}} with inner event name.
    assert!(
        parsed.get("SystemStats").is_some(),
        "must have SystemStats variant: {parsed}"
    );
    let inner = parsed["SystemStats"]
        .as_object()
        .expect("inner must be object");
    assert_eq!(inner["event"], "system.stats");

    // Clean up.
    server.abort();
}
