# Implementation Report: P16-C2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P16-C2                          |
| Phase         | 16 — Live Events                |
| Description   | anvilml-server: ws_handler forward loop + Lagged disconnect |
| Implemented   | 2026-07-09T19:15:00Z            |
| Status        | COMPLETE                          |

## Summary

Extended `handle_socket()` in `crates/anvilml-server/src/ws/handler.rs` from the P16-C1 skeleton (subscribe → send one SystemStats frame → return) into a full WebSocket forward loop that continuously receives `WsEvent`s from the broadcaster's broadcast channel, serialises each as JSON text, and sends it to the connected client. On `RecvError::Lagged` the connection is closed with a Close frame; on `RecvError::Closed` the same. Added `#[tracing::instrument]` and structured logging. Added 4 new integration tests covering event forwarding, lagged disconnect, concurrent clients, and post-disconnect server liveness. Bumped `anvilml-server` version 0.1.13 → 0.1.14.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | tokio     | 1.47.0           | Cargo.toml     |

No new external dependencies. `tokio` was already a dev-dependency; added it as a regular dependency with `sync` feature to access `tokio::sync::broadcast::error::RecvError` at runtime.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/ws/handler.rs` | Implement forward loop in `handle_socket()`, add `#[tracing::instrument]`, add logging, update module doc comment |
| MODIFY | `crates/anvilml-server/tests/handler_tests.rs` | Add 4 new integration tests, update module doc comment, update `test_handler_sends_exactly_one_frame_then_returns` to `test_handler_stays_alive_after_initial_frame` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.13 → 0.1.14, add `tokio` regular dependency |

## Commit Log

```
 .forge/reports/P16-C2_plan.md                | 167 +++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  14 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-server/Cargo.toml             |   3 +-
 crates/anvilml-server/src/ws/handler.rs      |  88 +++++---
 crates/anvilml-server/tests/handler_tests.rs | 303 +++++++++++++++++++++++++--
 7 files changed, 527 insertions(+), 56 deletions(-)
```

## Test Results

```
     Running tests/handler_tests.rs (target/debug/deps/handler_tests-5364fde27efc5d8e)

running 8 tests
test test_connect_receives_initial_system_stats_frame ... ok
test test_forwarded_event_is_json_text ... ok
test test_initial_frame_matches_ws_event_shape ... ok
test test_multiple_clients_each_receive_independent_initial_frame ... ok
test test_lagged_error_closes_connection ... ok
test test_concurrent_clients_independent_copies ... ok
test test_lagged_disconnect_no_panic ... ok
test test_handler_stays_alive_after_initial_frame ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all 300+ tests across all crates passed with 0 failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.52s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.44s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.80s
```

All four platform cross-checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered: this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.

### Gate 3 — Node Parity
Not triggered: this task does not add, remove, or rename node types.

### Gate 4 — Mock/Real Parity Markers
Not triggered: this task does not add or modify a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

```
(no output from grep)
```

No new `pub` items introduced. The only change is to the internal `handle_socket()` function, which is `async fn` (not `pub`). The public API surface of `anvilml-server` is unchanged.

## Deviations from Plan

- **Existing test renamed**: The P16-C1 test `test_handler_sends_exactly_one_frame_then_returns` was updated to `test_handler_stays_alive_after_initial_frame` to reflect the new behavior. The original test expected the handler to return after one frame; the forward loop now keeps it alive. The new test verifies the handler stays connected (no Close frame within 500ms) using a timeout, preventing an indefinite hang.
- **`tokio` added as regular dependency**: The plan listed `tokio` as a dev-dependency only. `RecvError` is used at runtime in `handle_socket()`, so `tokio` was added to `[dependencies]` with the `sync` feature. This is a minimal addition — only the `sync` feature, not `full`.
- **`Message::Close` requires `None` argument**: The plan referenced `Message::Close` but the actual `axum::extract::ws::Message::Close` variant requires a `CloseFrame` argument. Used `Message::Close(None)` as suggested by the compiler.

## Blockers

None.
