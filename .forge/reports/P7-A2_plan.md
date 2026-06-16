# Plan Report: P7-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-A2                                             |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml-server: GET /v1/events WebSocket upgrade handler |
| Depends on  | P7-A1 (EventBroadcaster)                          |
| Project     | anvilml                                           |
| Planned at  | 2026-06-16T07:58:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement the `ws_events` handler function in `crates/anvilml-server/src/ws/handler.rs` that accepts a WebSocket upgrade request at `GET /v1/events`, subscribes to the shared `EventBroadcaster`, and forwards each `WsEvent` as a JSON text frame to the connected client. Mount this route in `build_router()` so that a client such as `websocat ws://127.0.0.1:8488/v1/events` connects without error and receives broadcast events. When complete, the handler is the sole integration point for real-time event delivery to WebSocket clients.

## Scope

### In Scope
- Replace the stub in `crates/anvilml-server/src/ws/handler.rs` with the `pub async fn ws_events` implementation.
- Mount `GET /v1/events` route in `build_router()` in `crates/anvilml-server/src/lib.rs`.
- Bump `anvilml-server` crate version from `0.1.10` to `0.1.11`.
- Write integration tests in `crates/anvilml-server/tests/handler_tests.rs` covering the route exists (HTTP 101 upgrade response) and event delivery via WebSocket.

### Out of Scope
- The `stats_tick` background task (handled in P7-A3).
- Any authentication or rate-limiting on the WebSocket endpoint.
- OpenAPI documentation for the WebSocket endpoint.
- Subscribing to the initial `SystemStats` on connect (handled in P7-A3 when the tick exists).

## Existing Codebase Assessment

The `anvilml-server` crate already has the WebSocket infrastructure in place: `EventBroadcaster` (P7-A1) wraps a `tokio::sync::broadcast::Sender<WsEvent>` with capacity 1024, providing `send()` and `subscribe()` methods. The `AppState` struct includes a `broadcaster: Arc<EventBroadcaster>` field constructed in both `AppState::new()` and `AppState::new_with_hardware()`.

The `ws` module (`src/ws/mod.rs`) declares three submodules (`broadcaster`, `handler`, `stats_tick`). The `handler.rs` file currently contains a stub placeholder (`pub fn _stub() {}`). The `stats_tick.rs` file also contains a stub.

The existing `build_router()` in `lib.rs` mounts health, system, and model routes but does not yet include the events WebSocket route. Handler functions follow a consistent pattern: `pub async fn name(State(state): State<AppState>) -> Response`, using `axum::extract::State` for dependency injection. Tests live in `crates/anvilml-server/tests/` as separate test crate files, using `build_router` + `Router::oneshot` for non-WebSocket tests.

The `WsEvent` type in `anvilml-core/src/types/events.rs` is a tagged enum with `#[serde(tag = "type", rename_all = "snake_case")]`, making it directly serialisable to JSON via `serde_json::to_string()`.

No discrepancies were found between the design doc and the current source — the `EventBroadcaster`, `AppState`, and `WsEvent` types all exist as specified.

## Resolved Dependencies

| Type   | Name   | Version verified | MCP source     | Feature flags confirmed |
|--------|--------|-----------------|----------------|------------------------|
| crate  | axum   | 0.8.9           | Cargo.lock     | ws, json, http1, tokio |

**Notes:** No new external dependencies are introduced by this task. The `axum` crate is already declared in the workspace with the `ws` feature enabled (workspace `Cargo.toml` line 23). The `axum` version `0.8.9` was confirmed from `Cargo.lock`. The `ws` feature is confirmed present in the workspace dependency declaration.

## Approach

1. **Implement `ws_events` in `crates/anvilml-server/src/ws/handler.rs`.** Replace the stub with a `pub async fn ws_events` that:
   - Takes `ws: WebSocketUpgrade` and `State(state): State<AppState>` as arguments.
   - Returns `impl IntoResponse` (the axum 0.8 pattern for WebSocket handlers).
   - Calls `ws.on_upgrade(|socket| async move { ... })` to handle the connection lifecycle.
   - Inside the closure: accept the socket via `socket.accept().await`, subscribe to `state.broadcaster.subscribe()`, log `tracing::info!(remote_addr = ?remote_addr, "ws client connected")`, then loop calling `rx.recv().await` to get events.
   - For each event: serialize with `serde_json::to_string(&event)` (unwrap on error — a serialization failure on a `WsEvent` is a programming bug, not a runtime condition), send as `Message::Text(json.into())`, and break on send error (client disconnected).
   - Log `tracing::info!("ws client disconnected")` after the loop exits.
   - Add `///` doc comment on the function describing its purpose, arguments, and behavior.
   - **Rationale:** Use `on_upgrade` closure pattern rather than `WebSocketUpgrade::map` because the closure gives us direct access to the `WebSocket` object and `AppState` (captured via closure), enabling the subscribe+loop pattern cleanly.

2. **Mount the route in `build_router()` in `crates/anvilml-server/src/lib.rs`.** Add `.route("/v1/events", get(ws_events))` before `.with_state(state)`, importing `ws_events` from `crate::ws::handler`. **Rationale:** The route must be added before `.with_state(state)` because `with_state` finalises the router's state type — adding routes after would require a new router instance.

3. **Bump `anvilml-server` version from `0.1.10` to `0.1.11`.** Edit `crates/anvilml-server/Cargo.toml` `[package] version` line. **Rationale:** FORGE_AGENT_RULES §14 mandates patch version bump for any task modifying source files in a crate.

4. **Write integration tests in `crates/anvilml-server/tests/handler_tests.rs`.** Two tests:
   - `test_events_route_returns_101`: Build the router with `build_router`, dispatch a GET request to `/v1/events` using axum's `Request::builder` with `Upgrade: websocket` header, assert HTTP 101 status code. This verifies the route exists and accepts WebSocket upgrades without needing a live TCP listener.
   - `test_events_delivers_broadcast_event`: Use `axum::ws::TestClient` (or `tower::util::ServiceExt` + manual WebSocket upgrade) to connect to the events endpoint, broadcast a `WsEvent::SystemStats` through the broadcaster, and verify the connected client receives the JSON text frame. **Rationale:** This test proves the end-to-end event delivery path (broadcaster → handler → WebSocket frame).

5. **Update `docs/TESTS.md`** with entries for the new handler tests. **Rationale:** FORGE_AGENT_RULES §5.10 requires test catalogue sync — any task adding tests must update `docs/TESTS.md`.

## Public API Surface

| Item | Type | Module Path | Signature / Description |
|------|------|-------------|------------------------|
| `ws_events` | `pub async fn` | `anvilml_server::ws::handler` | `pub async fn ws_events(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse` — Accepts WebSocket upgrade requests at `GET /v1/events`, subscribes to the shared `EventBroadcaster`, and forwards `WsEvent` values as JSON text frames. |

No new `pub struct`, `pub enum`, or `pub trait` items are introduced. The route mounting in `lib.rs` does not change any existing public API surface.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/ws/handler.rs` | Replace stub with `pub async fn ws_events` implementation |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Add `ws_events` import and `.route("/v1/events", get(ws_events))` to `build_router()` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.10` → `0.1.11` |
| CREATE | `crates/anvilml-server/tests/handler_tests.rs` | Integration tests for WebSocket handler |
| MODIFY | `docs/TESTS.md` | Add entries for new handler tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/handler_tests.rs` | `test_events_route_returns_101` | The `/v1/events` route exists and returns HTTP 101 on WebSocket upgrade request | Router built with `build_router(AppState::new(...))` | GET `/v1/events` with `Upgrade: websocket` header | HTTP 101 status code | `cargo test -p anvilml-server -- handler_tests::test_events_route_returns_101` exits 0 |
| `crates/anvilml-server/tests/handler_tests.rs` | `test_events_delivers_broadcast_event` | A WebSocket client connected to `/v1/events` receives broadcast events as JSON text frames | Server running with `AppState` containing `EventBroadcaster`; event pre-broadcast | Connect WebSocket, broadcast `WsEvent::SystemStats` | Client receives `{"type":"system_stats",...}` JSON text frame | `cargo test -p anvilml-server -- handler_tests::test_events_delivers_broadcast_event` exits 0 |
| — | — | — | — | — | — | `cargo test -p anvilml-server -- broadcaster` exits 0 (pre-existing, must remain green) |

## CI Impact

No CI changes required. The new test file follows the existing convention of placing tests in `crates/anvilml-server/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware`. The route addition does not change any CI job's behavior — no new file types, gates, or test modules are introduced beyond the standard Rust test discovery.

## Platform Considerations

None identified. The WebSocket handler uses only tokio async I/O and serde JSON serialization, both of which are platform-neutral. The `tracing` crate is cross-platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum::extract::ws::WebSocketUpgrade::on_upgrade` API shape differs from expected — the closure signature or `WebSocket` methods may differ between 0.8.x patch versions. | Low | High | Verify the exact API at PLAN time using the axum 0.8.9 docs (confirmed via Cargo.lock). The handler implementation uses the standard `on_upgrade(|socket| async move { ... })` pattern which is stable across 0.8.x. Write a minimal compile check before proceeding to tests. |
| The broadcast receiver's `recv()` returns `RecvError::Lagged(n)` when the client falls behind (buffer overflow). If unhandled, the loop would exit silently without logging. | Medium | Medium | Handle `RecvError::Lagged(n)` explicitly: log a WARN with `lagged = n` and continue the loop (the lagged receiver will catch up on subsequent events). This prevents the handler from terminating on transient lag. |
| `serde_json::to_string` panics on a `WsEvent` serialization failure. Since `WsEvent` derives `Serialize`, this should never happen, but a bug in the derive could cause a panic in production. | Low | High | Use `match serde_json::to_string(&event)` with a WARN log and `break` on error rather than `unwrap()`. This converts a panic into a graceful disconnect. |
| Test client (`TestClient`) may not be available in axum 0.8.9 without an additional feature flag. | Medium | Medium | If `TestClient` is unavailable, use the `tower::util::ServiceExt` + `axum::ws::WebSocketConnection` approach to manually upgrade and test. Alternatively, use the integration test pattern from `health_tests.rs` (build router + `oneshot`) with an `Upgrade: websocket` header to verify the route exists, which is sufficient for this task's scope. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-server -- handler_tests::test_events_route_returns_101` exits 0
- [ ] `cargo test -p anvilml-server -- handler_tests::test_events_delivers_broadcast_event` exits 0
- [ ] `cargo test -p anvilml-server -- broadcaster` exits 0 (pre-existing tests remain green)
- [ ] `cargo test -p anvilml-server` exits 0 (all server tests)
