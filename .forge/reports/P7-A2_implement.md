# Implementation Report: P7-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A2                                         |
| Phase       | 007 — WebSocket Event Stream                  |
| Description | anvilml-server: WebSocket /v1/events handler    |
| Implemented | 2026-06-04T14:30:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Implemented the WebSocket `/v1/events` endpoint in `anvilml-server`. Added `broadcaster: Arc<EventBroadcaster>` to `AppState`, created a `ws_events` handler that upgrades HTTP connections to WebSocket, subscribes to the shared broadcaster, and forwards each event as JSON text frames. Wired the route into `build_router`, added `tokio-tungstenite` as a dev-dependency, and wrote an integration test that connects via WS, broadcasts a test event, and asserts the received frame is valid JSON text containing the event name.

## Resolved Dependencies

| Type   | Name              | Version resolved       | Source        |
|--------|-------------------|-----------------------|---------------|
| crate  | tokio-tungstenite | 0.24.0                | cargo search  |
| crate  | futures-util      | 0.3.32                | existing dep  |

Note: The plan specified `features = ["native-tls"]` but changed to `features = ["rustls-tls-native-roots"]` because the test environment lacks OpenSSL development libraries and `native-tls` requires them at build time. The `rustls-tls-native-roots` feature provides the same TLS support using pure-Rust rustls, which is appropriate for a dev-dependency used only in tests with plain `ws://` URIs.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `broadcaster: Arc<EventBroadcaster>` field; update `new()`, `new_with_hardware()` signatures and `Clone` impl |
| Create | `crates/anvilml-server/src/ws/handler.rs` | WebSocket upgrade handler with broadcast forwarding |
| Modify | `crates/anvilml-server/src/ws/mod.rs` | Add `pub mod handler;` re-export |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `GET /v1/events` route into `build_router`; update test calls |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `axum` ws feature, `futures-util` dep, `tokio-tungstenite` dev-dep |
| Create | `crates/anvilml-server/tests/api_ws_events.rs` | Integration test: connect, broadcast, assert JSON text |
| Modify | `backend/src/main.rs` | Create `EventBroadcaster` and pass to `AppState::new_with_hardware()` |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Fix pre-existing `unused_mut` warning on Windows-gnu cross-compile |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Update `build_test_app_state` to pass broadcaster |

## Commit Log

```
 Cargo.lock                                   | 328 +++++++++++++++++++++++++--
 backend/src/main.rs                          |   4 +-
 crates/anvilml-hardware/src/lib.rs           |   9 +-
 crates/anvilml-server/Cargo.toml             |   4 +-
 crates/anvilml-server/src/lib.rs             |  22 +-
 crates/anvilml-server/src/state.rs           |   9 +
 crates/anvilml-server/src/ws/handler.rs      |  62 +++++
 crates/anvilml-server/src/ws/mod.rs          |   1 +
 crates/anvilml-server/tests/api_models.rs    |   5 +-
 crates/anvilml-server/tests/api_ws_events.rs |  91 ++++++++
 10 files changed, 501 insertions(+), 34 deletions(-)
```

## Test Results

```
running 74 tests (anvilml_core) - all passed
running 59 tests (anvilml_hardware) - all passed
running 0 tests (anvilml_ipc) - all passed
running 0 tests (anvilml_openapi) - all passed
running 10 tests (anvilml_registry) - all passed
running 1 test (anvilml_registry_db integration) - passed
running 2 tests (rescan integration) - all passed
running 1 test (scanner integration) - passed
running 2 tests (store_get integration) - all passed
running 3 tests (store_list integration) - all passed
running 0 tests (anvilml_scheduler) - all passed
running 7 tests (anvilml_server unit) - all passed
running 3 tests (api_models integration) - all passed
running 1 test (api_ws_events integration: ws_connect_broadcast_receive) - passed
running 0 tests (anvilml_worker) - all passed
running 8 tests (backend cli unit) - all passed
running 1 test (config_reference integration) - passed
```

Full output: all 172 tests passed, 0 failed.

## Platform Cross-Check

### Check 1: `cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.59s
```
Result: PASS (exit 0, zero warnings)

### Check 2: `cargo check --bin anvilml`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.38s
```
Result: PASS (exit 0, zero warnings)

### Check 3: `cargo check --bin anvilml --target x86_64-pc-windows-gnu`
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.30s
```
Result: PASS (exit 0, zero warnings — pre-existing `unused_mut` warning fixed)

## Project Gates

### Config Surface Sync Gate
```
cargo test -p backend --features mock-hardware -- config_reference
test_toml_key_set_matches_default ... ok
```
Result: PASS (exit 0)

## Deviations from Plan

1. **axum ws feature added**: Added `ws` feature to axum dependency (production dep, not dev-only). The plan did not explicitly list this but it is required for `axum::extract::ws::*` types.
2. **futures-util dependency added**: Added `futures-util = "0.3"` as a production dependency for `SinkExt`/`StreamExt` traits used in the WebSocket handler.
3. **tokio-tungstenite TLS feature changed**: Plan specified `features = ["native-tls"]` but changed to `features = ["rustls-tls-native-roots"]` because the test environment lacks OpenSSL dev libraries (`libssl-dev`, `pkg-config`). The `rustls-tls-native-roots` feature provides equivalent TLS support using pure-Rust rustls.
4. **State type in handler**: Changed `State<AppState>` to `State<Arc<AppState>>` to match the router's `.with_state(Arc<AppState>)` pattern used by all other routes.
5. **Pre-existing warning fix**: Fixed `unused_mut` warning in `crates/anvilml-hardware/src/lib.rs` (Windows-gnu cross-compile path) by restructuring the `enumerate_gpus()` function to use `mut` only on Unix code paths where `devices` is actually mutated.
6. **Existing test updates**: Updated all existing `AppState::new()` and `AppState::new_with_hardware()` calls in unit tests (`lib.rs`) and integration tests (`api_models.rs`) to include the new `broadcaster` argument.

## Blockers

None.
