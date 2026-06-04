# Plan Report: P7-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A3                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-server: WS keepalive ping every 30s  |
| Depends on  | P7-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-04T12:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Add a 30-second WebSocket keepalive ping to the `/v1/events` handler in `ws/handler.rs`, ensuring it runs alongside the existing broadcast-forward and receive tasks via `tokio::select!` so that any task ending (including ping send error) closes the connection cleanly.

## Scope

### In Scope
- Add `time` feature to `tokio` dependency in `crates/anvilml-server/Cargo.toml` (required for `tokio::time::interval`)
- Modify `crates/anvilml-server/src/ws/handler.rs`:
  - Import `std::time::Duration` and `tokio::time::Interval`
  - Create a `ping_task` async block using `tokio::time::interval(Duration::from_secs(30))` that sends `Message::Ping(vec![])` on each tick and breaks on send error
  - Extend the existing two-arm `tokio::select!` to three arms (`forward`, `receive`, `ping_task`) so any termination closes the connection

### Out of Scope
- Pong response handling (tungstenite/axum handles pong auto-response; the handler ignores inbound pongs as it already does for all non-close messages)
- Ping interval configuration or tunability
- New test files — existing `api_ws_events.rs` test is sufficient regression check
- Changes to `broadcaster.rs`, `mod.rs`, `lib.rs`, or any other crate
- P7-A4 (system.stats tick), P7-A5, P7-B1

## Approach

1. **Add tokio `time` feature** — In `crates/anvilml-server/Cargo.toml`, change the tokio dependency from:
   ```toml
   tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync"] }
   ```
   to:
   ```toml
   tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time"] }
   ```
   This is required because `tokio::time::interval` needs the `time` feature. The existing test file already uses `tokio::time::sleep` and `tokio::time::timeout`, which also require this feature — adding it fixes a latent build gap.

2. **Add ping task to handler** — In `crates/anvilml-server/src/ws/handler.rs`:
   - Add `use std::time::Duration;` import (or use fully-qualified path)
   - Create a `ping_task` async block:
     ```rust
     let mut interval = tokio::time::interval(Duration::from_secs(30));
     // Set initial tick to fire immediately on connect.
     interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
     let ping_task = async move {
         loop {
             if interval.tick().await.is_err() {
                 break;
             }
             if ws_tx.send(Message::Ping(vec![])).await.is_err() {
                 break;
             }
         }
     };
     ```
   - Extend the `tokio::select!` from two arms to three:
     ```rust
     tokio::select! {
         _ = forward => {},
         _ = receive => {},
         _ = ping_task => {},
     }
     ```

3. **Verify** — Run `cargo test -p anvilml-server --features mock-hardware -- ws` to confirm the existing WS integration test still passes (the 30s ping interval is far beyond the ~200ms test duration, so it never fires during the test).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Add `time` feature to tokio dependency |
| Modify | `crates/anvilml-server/src/ws/handler.rs` | Add ping task and extend select! |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/tests/api_ws_events.rs` | `ws_connect_broadcast_receive` | Existing WS integration test still passes — connection, broadcast, receive all work with the new ping task running in background. The 30s interval never fires during the ~200ms test window. |
| `crates/anvilml-server/src/lib.rs` (tests module) | `health_returns_200`, `env_returns_200_with_stub_report`, `system_returns_200_with_hardware_info`, `get_model_returns_404_when_missing`, `rescan_returns_202` | No regression in other handler tests (these are all HTTP-only, unaffected by WS changes). |

No new test files are created. The existing `api_ws_events.rs` test serves as the regression gate per the task definition ("cargo test … still exits 0 (no regression)").

## CI Impact

No CI file changes. The `mock-hardware` CI already runs `cargo test --workspace --features mock-hardware`, which includes `anvilml-server`. Adding the `time` feature to tokio does not change any runtime behavior or API surface — it only enables a module that was previously unavailable at compile time. No workflow modifications needed (P7-B1 will handle CI hardening separately).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `tokio::time::MissedTickBehavior` API may not exist in tokio 1.x as used | Use `interval.set_missed_tick_behavior(MissedTickBehavior::Skip)` which is stable since tokio 1.23; if the crate pins an older version, fall back to letting ticks accumulate (the ping fires once then continues). The interval will still work — just may send multiple pings on slow systems. |
| Ping task could interfere with the existing `select!` semantics | The three-arm `select!` is a direct extension of the two-arm pattern already in use; any arm completing terminates all, which is exactly the desired behavior (ping error → close connection). |
| Adding `time` feature to tokio may change compilation time or binary size negligibly | Negligible impact; `time` is a small module and is already transitively pulled by dev-dependencies in tests. |
| Test `api_ws_events.rs` might fail if it requires `tokio::time` features not currently available | The test already uses `tokio::time::sleep` and `tokio::time::timeout`; adding the `time` feature to the tokio dependency resolves this latent gap. The test should pass both before (if time was transitively provided) and after (explicitly declared). |

## Acceptance Criteria

- [ ] `Cargo.toml` has `time` in tokio features list for `anvilml-server`
- [ ] `ws/handler.rs` contains a `ping_task` using `tokio::time::interval(Duration::from_secs(30))` sending `Message::Ping(vec![])`
- [ ] The `ping_task` runs alongside `forward` and `receive` in a `tokio::select!` block
- [ ] On ping send error, the connection closes (the `select!` exits)
- [ ] `cargo test -p anvilml-server --features mock-hardware -- ws` exits 0 with no new failures
- [ ] All existing tests in `anvilml-server` continue to pass
