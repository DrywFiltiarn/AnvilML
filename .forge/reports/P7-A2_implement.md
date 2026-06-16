# Implementation Report: P7-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P7-A2                              |
| Phase         | 007 — WebSocket Event Stream       |
| Description   | anvilml-server: GET /v1/events WebSocket upgrade handler |
| Implemented   | 2026-06-16T07:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented the `ws_events` WebSocket handler in `crates/anvilml-server/src/ws/handler.rs` that accepts WebSocket upgrade requests at `GET /v1/events`, subscribes to the shared `EventBroadcaster` from `AppState`, and forwards each `WsEvent` as a JSON text frame to the connected client. The route is mounted in `build_router()` in `lib.rs`. Version bumped `anvilml-server` from 0.1.10 to 0.1.11. Two integration tests verify the route returns HTTP 101 on upgrade and that broadcast events are delivered as JSON text frames through a real TCP listener. Updated `docs/TESTS.md` with entries for both new tests.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source       |
|--------|-----------|------------------|--------------|
| crate  | axum      | 0.8.9            | Cargo.lock   |

**Notes:** No new dependencies introduced. The `axum` crate is already declared in the workspace with `["json", "http1", "tokio", "ws"]` features. The `axum::serve` function was used for integration tests (requires `tokio` + `http1` features, both present).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/ws/handler.rs` | Replaced stub with `pub async fn ws_events` implementation using `on_upgrade` closure pattern, `ConnectInfo` for remote address, broadcast subscription loop, lag handling, and JSON serialization |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Added `ws_events` import from `crate::ws::handler` and `.route("/v1/events", get(ws_events))` before `.with_state(state)` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.10 → 0.1.11 |
| CREATE | `crates/anvilml-server/tests/handler_tests.rs` | Two integration tests using real TCP listener with `axum::serve` |
| MODIFY | `docs/TESTS.md` | Added entries for `test_events_route_returns_101` and `test_events_delivers_broadcast_event` |

## Commit Log

```
 .forge/state/CURRENT_TASK.md            |   6 +-
 .forge/state/state.json                 |  13 ++--
 Cargo.lock                              |   2 +-
 crates/anvilml-server/Cargo.toml        |   2 +-
 crates/anvilml-server/src/lib.rs        |  11 +++-
 crates/anvilml-server/src/ws/handler.rs | 109 ++++++++++++++++++++++++++++++--
 docs/TESTS.md                           |  18 ++++++
 7 files changed, 142 insertions(+), 19 deletions(-)
```

## Test Results

```
     Running tests/handler_tests.rs (target/debug/deps/handler_tests-f410e8adf5ed8924)

running 2 tests
test test_events_route_returns_101 ... ok
test test_events_delivers_broadcast_event ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/broadcaster_tests.rs (target/debug/deps/broadcaster_tests-9803b0ed4183bf0c)

running 3 tests
test test_broadcaster_send_and_receive ... ok
test test_broadcaster_new ... ok
test test_broadcaster_lagged_receiver ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/health_tests.rs (target/debug/deps/health_tests-0c8b0cdb3ac9147c)

running 1 test
test test_health_returns_200_with_status_key ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/models_tests.rs (target/debug/deps/models_tests-0dfdb5f993cbe85)

running 6 tests
test test_get_model_not_found ... ok
test test_list_models_empty ... ok
test test_rescan_returns_202 ... ok
test test_list_models_with_kind_filter ... ok
test test_rescan_populates_registry ... ok
test test_rescan_infer_kind_and_dtype ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/state_tests.rs (target/debug/deps/state_tests-d0326e0326e0c38d)

running 3 tests
test test_app_state_new ... ok
test test_app_state_clone ... ok
test test_app_state_version_from_env ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/system_tests.rs (target/debug/deps/system_tests-9eb97876f4751c05)

running 2 tests
test test_system_env_returns_200_with_default_report ... ok
test test_system_returns_200_with_hardware_info ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 103 tests passed, 0 failed.

## Format Gate

```
(exit 0 — no output, all files formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Checking anvilml-server v0.1.11 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.78s

# 2. Mock-hardware Windows
Checking anvilml-server v0.1.11 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.39s

# 3. Real-hardware Linux
Checking anvilml-server v0.1.11 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.14s

# 4. Real-hardware Windows
Checking anvilml-server v0.1.11 (/home/dryw/AnvilML/crates/anvilml-server)
Checking anvilml v0.1.9 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.21s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1: Config Surface Sync
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Gate 2: OpenAPI Drift
(cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json) — no diff, exits 0
```

Both gates pass.

## Public API Delta

```
+pub async fn ws_events(
```

One new `pub` item introduced:
- `pub async fn ws_events` — function in `anvilml_server::ws::handler`
  - Signature: `pub async fn ws_events(ws: WebSocketUpgrade, State(state): State<AppState>, ConnectInfo(remote_addr): ConnectInfo<std::net::SocketAddr>) -> impl IntoResponse`
  - Accepts WebSocket upgrade requests at `GET /v1/events`, subscribes to the shared `EventBroadcaster`, and forwards `WsEvent` values as JSON text frames.

## Deviations from Plan

- **Test approach changed:** The plan specified using `axum::ws::TestClient` for the delivery test. In axum 0.8.9, `TestClient` is gated by `#[cfg(test)]` on the axum crate itself, making it unavailable in integration test crates. Replaced with a real TCP listener approach using `axum::serve` + raw TCP I/O, which exercises the full handler path end-to-end.
- **Added `ConnectInfo` extractor:** The plan's handler signature used only `WebSocketUpgrade` and `State<AppState>`. Added `ConnectInfo<std::net::SocketAddr>` to extract the remote address for the required `tracing::info!(remote_addr = ?remote_addr, ...)` log point. The `axum::serve` function does not automatically set up `ConnectInfo`; tests use `into_make_service_with_connect_info::<SocketAddr>()` to enable it.
- **WebSocket frame parsing in tests:** The handler sends raw WebSocket text frames (2-byte header: FIN+opcode + MASK+payload length). The delivery test skips the first 2 bytes to extract the JSON payload, since the test uses raw TCP I/O instead of a WebSocket client library.
- **No new dependencies:** The plan anticipated that `TestClient` might not be available and listed `tungstenite` as a potential alternative. However, the `axum::serve` + raw TCP approach was chosen instead, avoiding any new dependency additions.

## Blockers

None.
