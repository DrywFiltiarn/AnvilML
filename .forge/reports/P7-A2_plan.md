# Plan Report: P7-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A2                                         |
| Phase       | 007 — WebSocket Event Stream                  |
| Description | anvilml-server: WebSocket /v1/events handler    |
| Depends on  | P7-A1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-04T13:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Implement the WebSocket `/v1/events` endpoint in `anvilml-server`: wire `Arc<EventBroadcaster>` into `AppState`, create a `ws_events` handler that upgrades the connection, subscribes to the broadcaster, and forwards each event as JSON text frames over tungstenite, with lag-disconnect handling (close code 1008). Wire the route into `build_router`. Add `tokio-tungstenite` as a dev-dependency. Provide an integration test that connects via WS, broadcasts a test event, and asserts the received frame is valid JSON text.

## Scope

### In Scope
- Add `broadcaster: Arc<EventBroadcaster>` field to `AppState` (capacity from `cfg.limits.ws_broadcast_capacity`).
- Update `AppState::new()` and `AppState::new_with_hardware()` to accept and store the broadcaster.
- Create `crates/anvilml-server/src/ws/handler.rs`: `ws_events` handler function using `WebSocketUpgrade` + `State<AppState>`.
- On connection: subscribe to broadcaster, spawn a task that forwards each `Arc<WsEvent>` as `Message::Text(serde_json::to_string(...))`.
- Handle `tokio_tungstenite::tungstenite::error::RecvError::Lagged` by closing the connection with close code 1008 (Policy Violation).
- Wire `GET /v1/events` route into `build_router` in `lib.rs`.
- Add `tokio-tungstenite = { version = "0.24", features = ["native-tls"] }` as a dev-dependency in `crates/anvilml-server/Cargo.toml`.
- Update `ws/mod.rs` to re-export the new `handler` module.
- Add integration test in `crates/anvilml-server/tests/api_ws_events.rs`: spin up app, connect WS client, broadcast test event via broadcaster, assert JSON text received.

### Out of Scope
- Keepalive ping (deferred to P7-A3).
- System stats tick task (deferred to P7-A4).
- History replay / backfill of missed events.
- Auth or subscription filtering.
- Modifying any production dependencies (tokio-tungstenite is a dev-dep only).

## Approach

1. **Add broadcaster field to `AppState`** (`state.rs`).
   - Add `broadcaster: Arc<EventBroadcaster>` as a new field.
   - Update `new()` and `new_with_hardware()` signatures to accept `Arc<EventBroadcaster>`.
   - Update the `Clone` impl to clone the `Arc`.

2. **Create WebSocket handler** (`src/ws/handler.rs`).
   - Define `async fn ws_events(upgrade: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse`.
   - Use `upgrade.on_upgrade()` with an async closure that:
     a. Accepts the WebSocket connection via `tokio_tungstenite::accept_ws(stream)`.
     b. Obtains a broadcast receiver via `state.broadcaster.subscribe()`.
     c. Spawns two concurrent tasks inside the connection scope using `tokio::spawn`:
        - **Forward task**: polls the broadcast receiver in a loop, serializes each `Arc<WsEvent>` to JSON text, and sends it as `Message::Text`. On error, breaks the loop.
        - **Receive task**: reads incoming messages from the client; on `RecvError::Lagged`, closes the connection with close frame `(1008, "lagged")`; on other errors or close, breaks.
     d. Uses `tokio::select!` to end when either task finishes.

3. **Wire route into router** (`src/lib.rs`).
   - Import `ws_events` from `crate::ws::handler`.
   - Add `.route("/v1/events", get(ws_events))` in `build_router`.

4. **Update module declarations** (`src/ws/mod.rs`).
   - Add `pub mod handler;`.

5. **Add dev-dependency** (`Cargo.toml`).
   - Add `tokio-tungstenite = { version = "0.24", features = ["native-tls"] }` under `[dev-dependencies]`.

6. **Write integration test** (`tests/api_ws_events.rs`).
   - Create `AppState` with a fresh `EventBroadcaster::new(16)`.
   - Build the router with that state.
   - Use `tower::ServiceExt::oneshot` to create an axum test request to `/v1/events` with appropriate upgrade headers (`Upgrade: websocket`, `Connection: Upgrade`, etc.).
   - Connect a `tokio_tungstenite::client_async` client using `tungstenite::client::IntoWsUri` on the mock server.
   - Alternatively, simpler approach: bind the axum app on a random port (127.0.0.1:0), connect a tungstenite WS client to it, have the test task call `state.broadcaster.send(WsEvent::SystemStats(...))`, then read from the WS client and assert the received text contains `"event":"system.stats"`.
   - Assert: connection opens (HTTP 101), at least one JSON text frame arrives within a short timeout, and the event name matches.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `broadcaster: Arc<EventBroadcaster>` field; update constructors and Clone impl |
| Create | `crates/anvilml-server/src/ws/handler.rs` | WebSocket upgrade handler with broadcast forwarding |
| Modify | `crates/anvilml-server/src/ws/mod.rs` | Add `pub mod handler;` re-export |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `GET /v1/events` route into `build_router` |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `tokio-tungstenite` dev-dependency |
| Create | `crates/anvilml-server/tests/api_ws_events.rs` | Integration test: connect, broadcast, assert JSON text |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/api_ws_events.rs` | `ws_connect_broadcast_receive` | Connect to `/v1/events`, broadcast a `WsEvent::SystemStats`, assert client receives valid JSON text frame with correct event name |

## CI Impact

No CI workflow files are modified. The task only affects the `anvilml-server` crate. The existing CI matrix (rust, python-worker, openapi-diff, rust-windows) runs `cargo test --workspace --features mock-hardware`, which will include the new test. No changes to `.github/workflows/ci.yml`.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `tokio-tungstenite` API version mismatch with axum 0.7 | Use `tokio-tungstenite` 0.24 which is compatible with axum 0.7 and tungstenite 0.23; pin exact version in Cargo.toml |
| Broadcast receiver lag causing memory pressure on slow clients | The broadcast channel capacity (default 256) limits buffering; the `Lagged` error variant closes the connection with code 1008 as specified |
| Test server binding conflicts | Use port 0 (OS-assigned random port) and extract the actual bound address from the listener |
| `tokio::select!` race between forward and receive tasks | Both tasks share the same connection lifetime; either ending closes the scope naturally — no extra cleanup needed |

## Acceptance Criteria

- [ ] `AppState` contains `broadcaster: Arc<EventBroadcaster>` field, populated from `cfg.limits.ws_broadcast_capacity`
- [ ] `ws_events` handler exists in `src/ws/handler.rs`, accepts `WebSocketUpgrade` + `State<AppState>`, subscribes to broadcaster, forwards events as JSON text
- [ ] `RecvError::Lagged` causes close with code 1008
- [ ] `GET /v1/events` route is wired into `build_router` and returns HTTP 101 on WebSocket upgrade
- [ ] `tokio-tungstenite` added as dev-dependency in `crates/anvilml-server/Cargo.toml`
- [ ] Integration test connects to `/v1/events`, broadcasts a test event, asserts received frame is valid JSON text containing the event name
- [ ] `cargo test -p anvilml-server --features mock-hardware -- ws` exits 0
