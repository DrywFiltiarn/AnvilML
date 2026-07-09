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
//! This file covers only `P16-C1`'s scope: the connection skeleton and its
//! exact first frame. It does not test event forwarding after the initial
//! frame or the `Lagged`-disconnect rule — both are `P16-C2`'s scope, added
//! by a later task.

use anvilml_artifacts::ArtifactStore;
use anvilml_core::{NodeTypeRegistry, ServerConfig, WsEvent};
use anvilml_ipc::EventBroadcaster;
use anvilml_registry::JobStore;
use anvilml_scheduler::JobScheduler;
use anvilml_server::{AppState, build_router};
use anvilml_worker::WorkerPool;
use futures_util::StreamExt;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message as ClientMessage;

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

    let scheduler = Arc::new(JobScheduler::new(
        job_store,
        Arc::clone(&node_registry),
        artifact_store.clone(),
    ));
    let workers = Arc::new(
        WorkerPool::new()
            .await
            .expect("WorkerPool::new() must succeed in test"),
    );

    AppState {
        config: Arc::new(ServerConfig::default()),
        node_registry,
        start_time: std::time::Instant::now(),
        scheduler,
        workers,
        db,
        artifact_store,
        broadcaster: Arc::new(EventBroadcaster::new()),
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

/// The handler subscribes then returns after exactly one frame, per this
/// task's explicitly deferred scope — the ongoing forward loop is
/// `P16-C2`'s responsibility, not this one's. The second read on the
/// stream must therefore be a Close frame or the end of the stream, never
/// a second data frame.
#[tokio::test]
async fn test_handler_sends_exactly_one_frame_then_returns() {
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

    // The next item is either a Close frame (axum closes the socket once
    // handle_socket() returns), a transport-level error from the abrupt
    // close, or `None` (stream ended) — never another Text frame, since
    // the forward loop does not exist yet.
    match ws_stream.next().await {
        None => {}
        Some(Ok(ClientMessage::Close(_))) => {}
        Some(Err(_)) => {}
        Some(Ok(other)) => panic!("expected Close or end-of-stream, got {other:?}"),
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
