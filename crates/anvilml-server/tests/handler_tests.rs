//! Integration tests for the `GET /v1/events` WebSocket handler.
//!
//! Unlike the other handler test files in this crate, these tests cannot
//! use `tower::ServiceExt::oneshot()` — a WebSocket upgrade needs a real,
//! bidirectional socket, which axum's in-process oneshot testing helper
//! does not provide (it never completes the actual `Upgraded` I/O). Each
//! test here instead binds a real `TcpListener` on an ephemeral localhost
//! port, serves the router via `axum::serve()` in a background task, and
//! connects as a genuine WebSocket client via
//! `tokio_tungstenite::connect_async()`.
//!
//! This file covers the full `GET /v1/events` handler: the connection
//! skeleton, its exact first frame, event forwarding, concurrent clients,
//! and the `Lagged`-disconnect rule.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{
    EnvReport, HardwareInfo, NodeTypeRegistry, ProvisioningState, ServerConfig, WsEvent,
};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use futures_util::StreamExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message as ClientMessage;
use uuid::Uuid;

/// Helper to create an in-memory SQLite pool with migrations applied.
///
/// Duplicated from `health_tests.rs` rather than shared, per this crate's
/// existing test-file convention of self-contained integration test files.
async fn make_test_pool() -> sqlx::SqlitePool {
    let connect_opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("in-memory SQLite pool must connect");

    let migrator = sqlx::migrate!("../../database/migrations");
    migrator
        .run(&pool)
        .await
        .expect("migrations must apply to in-memory pool");

    pool
}

/// Construct a minimal `AppState` suitable for WebSocket handler tests.
///
/// Mirrors `health_tests.rs`'s `make_test_state()` — the `/v1/events`
/// handler only touches `state.broadcaster`, so the other fields are
/// minimal stand-ins sufficient to construct a valid `AppState`.
async fn make_test_state() -> AppState {
    let node_registry = Arc::new(NodeTypeRegistry::new());
    let db = make_test_pool().await;
    let job_store = JobStore::new(db.clone());

    let artifact_store = Arc::new(ArtifactStore::new(
        std::env::temp_dir().join("anvilml-test-artifacts-handler"),
        db.clone(),
    ));

    let workers = Arc::new(
        WorkerPool::new()
            .await
            .expect("WorkerPool::new() must succeed in test"),
    );

    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        artifact_store.clone(),
        Arc::clone(&workers).transport().clone(),
    ));

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db,
        artifact_store,
        broadcaster: Arc::new(EventBroadcaster::new()),
        hardware: Arc::new(RwLock::new(HardwareInfo {
            host: anvilml_core::HostInfo {
                hostname: "test-host".to_string(),
                os: "Linux".to_string(),
            },
            gpus: vec![],
            inference_caps: anvilml_core::InferenceCaps::default(),
        })),
        env_report: Arc::new(RwLock::new(EnvReport {
            python_path: Some("./worker/.venv/bin/python3".to_string()),
            python_version: None,
            torch_version: None,
            provisioning: ProvisioningState::NotStarted,
            preflight_ok: false,
            reason: None,
            node_types: Vec::new(),
        })),
    }
}

/// Bind a real TCP listener on an ephemeral localhost port, serve the
/// router built from `state` via `axum::serve()` in a background task,
/// and return the `ws://.../v1/events` URL clients should connect to.
///
/// A real socket is required — see this file's module doc comment for why
/// `tower::ServiceExt::oneshot()` cannot be used for a WebSocket upgrade.
async fn spawn_test_server(state: AppState) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral port must bind");
    let addr = listener
        .local_addr()
        .expect("bound listener must have a local addr");
    let router = build_router(state);
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("test server must serve without error");
    });
    format!("ws://{addr}/v1/events")
}

/// Connecting to `/v1/events` receives exactly one initial frame, and that
/// frame carries the `"system_stats"` tag — per `ANVILML_DESIGN.md §13.6`'s
/// connect sequence: subscribe, then immediately send the current
/// `SystemStats`.
#[tokio::test]
async fn test_connect_receives_initial_system_stats_frame() {
    let url = spawn_test_server(make_test_state().await).await;

    let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("WebSocket handshake must succeed");

    let msg = ws_stream
        .next()
        .await
        .expect("connection must yield at least one message")
        .expect("first message must not be a transport error");

    let text = match msg {
        ClientMessage::Text(text) => text,
        other => panic!("expected a Text frame, got {other:?}"),
    };

    let parsed: serde_json::Value =
        serde_json::from_str(text.as_str()).expect("initial frame must be valid JSON");
    assert_eq!(parsed["type"], "system_stats");
}

/// The initial frame's JSON shape matches `WsEvent`'s tagged representation
/// exactly — it round-trips through `WsEvent`'s own `Deserialize` impl as
/// the `SystemStats` variant, with the placeholder's zero-valued fields.
#[tokio::test]
async fn test_initial_frame_matches_ws_event_shape() {
    let url = spawn_test_server(make_test_state().await).await;

    let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("WebSocket handshake must succeed");

    let msg = ws_stream
        .next()
        .await
        .expect("connection must yield at least one message")
        .expect("first message must not be a transport error");

    let text = match msg {
        ClientMessage::Text(text) => text,
        other => panic!("expected a Text frame, got {other:?}"),
    };

    let event: WsEvent =
        serde_json::from_str(text.as_str()).expect("initial frame must deserialize as a WsEvent");
    match event {
        WsEvent::SystemStats {
            cpu_pct,
            ram_used_mib,
            workers,
        } => {
            // The placeholder frame is explicitly zero-valued per P16-C1's
            // scope — the real periodic tick is P16-D1's later concern.
            assert_eq!(cpu_pct, 0.0);
            assert_eq!(ram_used_mib, 0);
            assert!(workers.is_empty());
        }
        other => panic!("expected WsEvent::SystemStats, got {other:?}"),
    }
}

/// After sending the initial frame, the handler enters the forward loop
/// (added in `P16-C2`) and stays connected, waiting for events from the
/// broadcast channel. With no events being published, the connection
/// remains open — the client's next read should not produce a Close frame
/// or end-of-stream within a reasonable timeout.
#[tokio::test]
async fn test_handler_stays_alive_after_initial_frame() {
    let url = spawn_test_server(make_test_state().await).await;

    let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("WebSocket handshake must succeed");

    let first = ws_stream
        .next()
        .await
        .expect("connection must yield at least one message")
        .expect("first message must not be a transport error");
    assert!(
        matches!(first, ClientMessage::Text(_)),
        "first message must be the initial SystemStats text frame"
    );

    // The handler now enters the forward loop (P16-C2) and waits for
    // broadcast events. With no events published, the connection stays
    // open. We use a short timeout to verify the stream doesn't
    // immediately close — if it did, the forward loop isn't working.
    // Use a non-blocking peek by polling with a brief timeout.
    let result =
        tokio::time::timeout(tokio::time::Duration::from_millis(500), ws_stream.next()).await;

    // Within 500ms, no message should arrive (no events published).
    // The stream should not be closed — a Close frame would indicate
    // the handler returned prematurely instead of entering the forward loop.
    match result {
        Ok(None) => panic!("connection closed unexpectedly — handler returned before forward loop"),
        Ok(Some(Ok(ClientMessage::Close(_)))) => {
            panic!("Close frame received within 500ms — handler returned prematurely");
        }
        Ok(Some(Ok(other))) => {
            // A data frame arrived — this means an event was published
            // from somewhere (unexpected in a clean test). Accept it
            // as proof the handler is alive and forwarding.
            tracing::info!(frame_type = ?other, "unexpected data frame — handler is alive and forwarding");
        }
        Ok(Some(Err(e))) => {
            panic!("transport error within 500ms: {e:?}");
        }
        Err(_) => {
            // Timeout — no message arrived within 500ms, confirming
            // the handler is in the forward loop waiting for events.
            // This is the expected outcome.
        }
    }
}

/// Two independent clients connecting concurrently each subscribe and each
/// receive their own initial `SystemStats` frame — proving `subscribe()`
/// is called per-connection, not shared across connections.
#[tokio::test]
async fn test_multiple_clients_each_receive_independent_initial_frame() {
    let url = spawn_test_server(make_test_state().await).await;

    let (mut ws_a, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("first client's WebSocket handshake must succeed");
    let (mut ws_b, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("second client's WebSocket handshake must succeed");

    for ws_stream in [&mut ws_a, &mut ws_b] {
        let msg = ws_stream
            .next()
            .await
            .expect("connection must yield at least one message")
            .expect("first message must not be a transport error");
        let text = match msg {
            ClientMessage::Text(text) => text,
            other => panic!("expected a Text frame, got {other:?}"),
        };
        let parsed: serde_json::Value =
            serde_json::from_str(text.as_str()).expect("initial frame must be valid JSON");
        assert_eq!(parsed["type"], "system_stats");
    }
}

/// After connecting and receiving the initial SystemStats frame, publishing
/// a JobQueued event via `broadcaster.publish()` is forwarded to the client
/// as a JSON text frame containing `"type":"job_queued"`.
///
/// This verifies the forward loop added in `P16-C2`: events published to
/// the broadcast channel after the initial frame are serialized and sent
/// as text frames to the connected WebSocket client.
#[tokio::test]
async fn test_forwarded_event_is_json_text() {
    let state = make_test_state().await;
    let state_clone = state.clone();
    let url = spawn_test_server(state).await;

    let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("WebSocket handshake must succeed");

    // Consume the initial SystemStats frame so the stream is ready
    // for the forwarded event.
    let first = ws_stream
        .next()
        .await
        .expect("connection must yield the initial frame")
        .expect("initial frame must not be a transport error");
    assert!(matches!(first, ClientMessage::Text(_)));

    // Publish a JobQueued event directly into the broadcaster.
    // Using a clone of state since the original was moved into spawn_test_server.
    let job_id = Uuid::new_v4();
    state_clone.broadcaster.publish(WsEvent::JobQueued {
        job_id,
        queue_position: 1,
    });

    // The forward loop should deliver the event as a text frame.
    let msg = ws_stream
        .next()
        .await
        .expect("forwarded event must arrive within timeout")
        .expect("forwarded event must not be a transport error");

    let text = match msg {
        ClientMessage::Text(text) => text,
        other => panic!("expected a Text frame for forwarded event, got {other:?}"),
    };

    let parsed: serde_json::Value =
        serde_json::from_str(&text).expect("forwarded event must be valid JSON");
    assert_eq!(parsed["type"], "job_queued");
    assert_eq!(parsed["job_id"], job_id.to_string());
    assert_eq!(parsed["queue_position"], 1);
}

/// Publishing >1024 events rapidly while the client is idle (not calling
/// `recv()`) causes the broadcast buffer to overflow. When the client
/// finally calls `recv()`, it receives a `Lagged` error, and the handler
/// sends a Close frame and exits — rather than panicking or continuing.
///
/// This verifies the `RecvError::Lagged` disconnect path added in `P16-C2`.
#[tokio::test]
async fn test_lagged_error_closes_connection() {
    let state = make_test_state().await;
    let state_clone = state.clone();
    let url = spawn_test_server(state).await;

    let (mut ws_stream, _response) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("WebSocket handshake must succeed");

    // Consume the initial SystemStats frame.
    let first = ws_stream
        .next()
        .await
        .expect("connection must yield the initial frame")
        .expect("initial frame must not be a transport error");
    assert!(matches!(first, ClientMessage::Text(_)));

    // Publish >1024 events rapidly while the client does not read.
    // The broadcast buffer is 1024 events (per ANVILML_DESIGN.md §13.6),
    // so publishing more than that forces the receiver into Lagged state.
    for i in 0..1100u64 {
        state_clone
            .broadcaster
            .publish(WsEvent::ProvisioningProgress {
                message: format!("step {i}"),
                pct: (i % 100) as u8,
            });
    }

    // Give the server's forward loop a brief moment to process.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // The client's next recv() should yield a Close frame (or the
    // stream should end) — never a data frame, since the Lagged path
    // broke the loop.
    match ws_stream.next().await {
        None => {}
        Some(Ok(ClientMessage::Close(_))) => {}
        Some(Err(_)) => {}
        Some(Ok(other)) => panic!("expected Close or end-of-stream after lag, got {other:?}"),
    }
}

/// Two concurrent clients each receive their own independent copy of
/// a forwarded event — proving the forward loop creates independent
/// subscriptions per connection, not a shared stream.
#[tokio::test]
async fn test_concurrent_clients_independent_copies() {
    let state = make_test_state().await;
    let state_clone = state.clone();
    let url = spawn_test_server(state).await;

    let (mut ws_a, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("first client's WebSocket handshake must succeed");
    let (mut ws_b, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("second client's WebSocket handshake must succeed");

    // Both clients consume their initial SystemStats frames.
    let msg_a = ws_a
        .next()
        .await
        .expect("client_a must yield the initial frame")
        .expect("initial frame must not be a transport error");
    let msg_b = ws_b
        .next()
        .await
        .expect("client_b must yield the initial frame")
        .expect("initial frame must not be a transport error");
    assert!(matches!(msg_a, ClientMessage::Text(_)));
    assert!(matches!(msg_b, ClientMessage::Text(_)));

    // Publish one event. Both clients must receive their own copy.
    // Using a clone of state since the original was moved into spawn_test_server.
    let job_id = Uuid::new_v4();
    state_clone.broadcaster.publish(WsEvent::JobQueued {
        job_id,
        queue_position: 1,
    });

    // Give the forward loop a moment to deliver.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Read from client_a.
    let msg_a = ws_a
        .next()
        .await
        .expect("forwarded event must arrive for client_a")
        .expect("forwarded event must not be a transport error");

    let text_a = match msg_a {
        ClientMessage::Text(text) => text,
        other => panic!("expected Text frame for client_a, got {other:?}"),
    };

    let parsed_a: serde_json::Value =
        serde_json::from_str(&text_a).expect("forwarded event must be valid JSON");
    assert_eq!(parsed_a["type"], "job_queued");
    assert_eq!(parsed_a["job_id"], job_id.to_string());

    // Read from client_b — independent copy.
    let msg_b = ws_b
        .next()
        .await
        .expect("forwarded event must arrive for client_b")
        .expect("forwarded event must not be a transport error");

    let text_b = match msg_b {
        ClientMessage::Text(text) => text,
        other => panic!("expected Text frame for client_b, got {other:?}"),
    };

    let parsed_b: serde_json::Value =
        serde_json::from_str(&text_b).expect("forwarded event must be valid JSON");
    assert_eq!(parsed_b["type"], "job_queued");
    assert_eq!(parsed_b["job_id"], job_id.to_string());
}

/// After a Lagged disconnect, the server remains operational — new
/// clients can connect and receive their initial SystemStats frame
/// successfully. This verifies the handler exits cleanly without
/// panicking or taking down the server task.
#[tokio::test]
async fn test_lagged_disconnect_no_panic() {
    let state = make_test_state().await;
    let state_clone = state.clone();
    let url = spawn_test_server(state).await;

    // First client: trigger a Lagged disconnect.
    let (mut ws_lag, _response) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("first client's WebSocket handshake must succeed");

    // Consume the initial SystemStats frame.
    let first = ws_lag
        .next()
        .await
        .expect("connection must yield the initial frame")
        .expect("initial frame must not be a transport error");
    assert!(matches!(first, ClientMessage::Text(_)));

    // Publish >1024 events to force a Lagged error.
    // Using a clone of state since the original was moved into spawn_test_server.
    for i in 0..1100u64 {
        state_clone
            .broadcaster
            .publish(WsEvent::ProvisioningProgress {
                message: format!("step {i}"),
                pct: (i % 100) as u8,
            });
    }

    // Wait for the Lagged disconnect to propagate.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // The lagged client's stream should end or close cleanly.
    match ws_lag.next().await {
        None | Some(Ok(ClientMessage::Close(_))) | Some(Err(_)) => {}
        Some(Ok(other)) => panic!("expected close after lag, got {other:?}"),
    }

    // Second client: connect and verify the server is still running.
    let (mut ws_ok, _response) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("second client must connect after lagged disconnect");

    // The second client should receive its initial SystemStats frame.
    let msg = ws_ok
        .next()
        .await
        .expect("second client must receive initial frame")
        .expect("initial frame must not be a transport error");

    let text = match msg {
        ClientMessage::Text(text) => text,
        other => panic!("expected Text frame for second client, got {other:?}"),
    };

    let parsed: serde_json::Value =
        serde_json::from_str(&text).expect("initial frame must be valid JSON");
    assert_eq!(parsed["type"], "system_stats");
}
