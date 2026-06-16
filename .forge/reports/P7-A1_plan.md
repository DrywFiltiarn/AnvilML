# Plan Report: P7-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A1                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-server: EventBroadcaster wrapping broadcast channel |
| Depends on  | P6 tasks (WsEvent type exists in anvilml-core) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T00:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `EventBroadcaster` struct in `crates/anvilml-server/src/ws/broadcaster.rs` — a thin wrapper around `tokio::sync::broadcast::Sender<WsEvent>` with capacity 1024. Provide `new()`, `send()`, and `subscribe()` methods. Inject `Arc<EventBroadcaster>` into `AppState` so the WebSocket handler (P7-A2) and stats tick task (P7-A3) can share a single broadcaster instance. The observable outcome is that `cargo test -p anvilml-server -- broadcaster` exits 0 with at least three tests, proving events can be sent and received through the channel, and that lagged subscribers are detected and logged.

## Scope

### In Scope
- **CREATE** `crates/anvilml-server/src/ws/broadcaster.rs` — `EventBroadcaster` struct with `new()`, `send()`, `subscribe()`.
- **CREATE** `crates/anvilml-server/src/ws/mod.rs` — declares `broadcaster`, `handler` (stub), `stats_tick` (stub); re-exports `EventBroadcaster`.
- **MODIFY** `crates/anvilml-server/src/lib.rs` — adds `pub mod ws;` to expose the new module.
- **MODIFY** `crates/anvilml-server/src/state.rs` — adds `broadcaster: Arc<EventBroadcaster>` field to `AppState`; initialises it in both `new()` and `new_with_hardware()` constructors.
- **MODIFY** `crates/anvilml-server/Cargo.toml` — bump patch version `0.1.9 → 0.1.10`.
- **CREATE** `crates/anvilml-server/tests/broadcaster_tests.rs` — ≥ 3 tests exercising `new()`, `send()` + `subscribe()`, and lagged-receiver detection.

### Out of Scope
- WebSocket handler implementation (`handler.rs`) — handled by P7-A2.
- SystemStats tick task implementation (`stats_tick.rs`) — handled by P7-A3.
- Adding `sysinfo` crate dependency — handled by P7-A3.
- Modifying `build_router()` to mount the WebSocket route — handled by P7-A2.
- Any changes to `anvilml-core` — `WsEvent` already exists there.

## Existing Codebase Assessment

The `anvilml-server` crate currently has six submodules under `src/`: `lib.rs`, `state.rs`, `error.rs`, and `handlers/` (with health, models, system, jobs, workers, artifacts, nodes). The `ws/` directory does not yet exist.

`AppState` (`state.rs`) holds shared server state: `start_time`, `version`, `env_report`, `hardware: Arc<RwLock<HardwareInfo>>`, `db: SqlitePool`, `registry: Arc<ModelStore>`, and `model_dirs: Vec<ModelDirConfig>`. It has two constructors: `new()` (async, in-memory pool, for tests) and `new_with_hardware()` (sync, file-backed pool, for production). Both are well-documented with `///` comments.

`WsEvent` already exists in `anvilml-core/src/types/events.rs` as a tagged enum with ten variants, serialised with `#[serde(tag = "type", rename_all = "snake_case")]`. It is re-exported as `anvilml_core::types::WsEvent`.

The test style in `crates/anvilml-server/tests/` uses `#[tokio::test] async fn` with doc comments explaining the test's purpose. Tests use `anvilml_server::AppState::new("version").await` to construct state, then exercise the production `build_router` path or directly test individual types. Integration-style tests use `tower::util::ServiceExt::oneshot`.

The crate dependency graph is linear: `anvilml-server` depends on `anvilml-core`, `anvilml-hardware`, `anvilml-ipc`, `anvilml-registry`, `anvilml-scheduler`, and `anvilml-worker`. Adding `Arc<EventBroadcaster>` to `AppState` does not introduce new crate dependencies — `tokio::sync::broadcast` is available via the existing `tokio` workspace dependency (1.52.3, "full" features).

## Resolved Dependencies

| Type   | Name  | Version verified | MCP source | Feature flags confirmed |
|--------|-------|-----------------|------------|------------------------|
| crate  | tokio | 1.52.3          | Workspace Cargo.toml | sync (included in "full") |

No new external dependencies are introduced by this task. The `tokio` workspace dependency already includes the `sync` module (part of "full" features), which provides `tokio::sync::broadcast::channel`, `Sender`, and `Receiver`.

## Approach

1. **Create `crates/anvilml-server/src/ws/broadcaster.rs`.** Define `pub struct EventBroadcaster { tx: tokio::sync::broadcast::Sender<WsEvent> }`. Implement `new()` that calls `tokio::sync::broadcast::channel(1024)` and wraps the sender. Implement `pub fn send(&self, event: WsEvent)` that calls `self.tx.send(&event)` and logs `tracing::warn!(event_type = ?event, "broadcast receiver lagged, message dropped")` when `send()` returns `Err(broadcast::error::SendError(_))`. Implement `pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WsEvent>` that calls `self.tx.subscribe()`. Add `///` doc comments on the struct and every public method.

2. **Create `crates/anvilml-server/src/ws/mod.rs`.** Declare `pub mod broadcaster;` and `pub mod handler;` (stub — will be implemented in P7-A2) and `pub mod stats_tick;` (stub — will be implemented in P7-A3). Re-export `pub use broadcaster::EventBroadcaster;` so consumers can write `anvilml_server::ws::EventBroadcaster`.

3. **Modify `crates/anvilml-server/src/lib.rs`.** Add `pub mod ws;` after `pub mod state;` to expose the new module at the crate root. This is a one-line addition.

4. **Modify `crates/anvilml-server/src/state.rs`.** Add `broadcaster: Arc<EventBroadcaster>` as a new field to `AppState`. In `new()`, initialise it with `Arc::new(EventBroadcaster::new())`. In `new_with_hardware()`, initialise it the same way. Update the struct-level doc comment to mention the broadcaster. The `EventBroadcaster` type is in scope because `ws` module is now `pub mod` in `lib.rs`, so `crate::ws::EventBroadcaster` is accessible within `state.rs` (same crate).

5. **Bump `crates/anvilml-server/Cargo.toml` version** from `0.1.9` to `0.1.10`.

6. **Create `crates/anvilml-server/tests/broadcaster_tests.rs`.** Write ≥ 3 tests:
   - `test_broadcaster_new`: verify `EventBroadcaster::new()` succeeds and the channel capacity is 1024 (use `tx.receiver_count()` which returns 0 immediately after creation, then subscribe and check `receiver_count() == 1`).
   - `test_broadcaster_send_and_receive`: create a broadcaster, subscribe, send a `WsEvent::SystemStats`, verify the receiver gets the exact event.
   - `test_broadcaster_lagged_receiver`: create a broadcaster, send multiple events without consuming, verify that when the buffer overflows the `send()` returns `Err(SendError)` and a warning is logged. (Note: since we can't easily assert log output in unit tests, we verify the error return path by sending past capacity.)

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `EventBroadcaster` | struct | `anvilml_server::ws::broadcaster` | `pub struct EventBroadcaster { tx: broadcast::Sender<WsEvent> }` |
| `EventBroadcaster::new` | fn | `anvilml_server::ws::broadcaster` | `pub fn new() -> Self` |
| `EventBroadcaster::send` | fn | `anvilml_server::ws::broadcaster` | `pub fn send(&self, event: WsEvent)` |
| `EventBroadcaster::subscribe` | fn | `anvilml_server::ws::broadcaster` | `pub fn subscribe(&self) -> broadcast::Receiver<WsEvent>` |
| `AppState::broadcaster` | field | `anvilml_server::state` | `pub broadcaster: Arc<EventBroadcaster>` (new field on existing struct) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/ws/broadcaster.rs` | EventBroadcaster struct and impl |
| CREATE | `crates/anvilml-server/src/ws/mod.rs` | Module declarations and re-exports |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Add `pub mod ws;` |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `broadcaster` field to AppState; init in both constructors |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.9 → 0.1.10 |
| CREATE | `crates/anvilml-server/tests/broadcaster_tests.rs` | ≥ 3 unit tests for EventBroadcaster |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/broadcaster_tests.rs` | `test_broadcaster_new` | `EventBroadcaster::new()` creates a valid broadcaster with channel capacity 1024; `receiver_count()` is 0 before subscribe, 1 after. | None | None | Constructor succeeds; channel is functional. | `cargo test -p anvilml-server --features mock-hardware -- broadcaster` exits 0 |
| `crates/anvilml-server/tests/broadcaster_tests.rs` | `test_broadcaster_send_and_receive` | `send()` delivers an event to a subscriber; the received event matches the sent event exactly. | A broadcaster with one subscriber. | A `WsEvent::SystemStats` with known fields. | Receiver gets the same event via `recv().await`. | Same command above. |
| `crates/anvilml-server/tests/broadcaster_tests.rs` | `test_broadcaster_lagged_receiver` | When the channel buffer overflows, `send()` returns `Err(SendError)` and the event is dropped (not delivered to any subscriber). | A broadcaster with capacity 1024 and one subscriber that does not consume. | 1025 events sent in rapid succession. | Last `send()` returns `Err`; subscriber only receives 1024 events. | Same command above. |

## CI Impact

No CI changes required. The test module `broadcaster_tests.rs` lives under `crates/anvilml-server/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware`. No new CI job, gate, or platform-specific handling is introduced.

## Platform Considerations

None identified. The `tokio::sync::broadcast` channel is a pure in-memory construct with no platform-specific code. The `tracing::warn!` macro is also platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::sync::broadcast::Sender::send()` takes `&self` and accepts `&T` (reference) in tokio 1.x, not owned `T`. The plan specifies `send(&self, event: WsEvent)` which calls `self.tx.send(&event)` — this is correct for tokio 1.x broadcast API. If the ACT agent mistakenly passes an owned value, the compiler will reject it. | Low | Medium | The approach step explicitly notes `self.tx.send(&event)` with the reference. The compiler will catch any deviation. |
| `tracing::warn!` with `?event` on `WsEvent` requires `WsEvent` to derive `Debug`. `WsEvent` already derives `Debug` in `events.rs`, so this is safe. | Low | Low | Confirmed by reading `anvilml-core/src/types/events.rs` — `#[derive(Debug, Clone, PartialEq, ...)]` is present on the enum. |
| Circular dependency: `state.rs` needs `EventBroadcaster` which is in `ws/broadcaster.rs`. Both are in the same crate (`anvilml-server`), so there is no inter-crate dependency issue. The `pub mod ws;` declaration in `lib.rs` makes the module visible within the crate. | Low | Low | Same-crate modules can reference each other freely. No circular crate dependency is introduced. |
| Test `test_broadcaster_lagged_receiver` requires the channel to be small enough to overflow quickly. The default capacity is 1024, so sending 1025 events is needed. This is a synchronous operation (no await between sends), so it will work. However, `broadcast::Receiver::recv()` is async and must be awaited. The test must ensure the subscriber does not consume between sends. | Low | Medium | The test sends all events in a tight loop without awaiting `recv()`. After the loop, it drains the receiver to count how many events were actually received (should be 1024). |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware -- broadcaster` exits 0 with ≥ 3 tests
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (no compilation errors from new code)
- [ ] `grep "^pub mod ws;" crates/anvilml-server/src/lib.rs` finds the module declaration
- [ ] `grep "broadcaster:" crates/anvilml-server/src/state.rs` finds the new field in AppState
- [ ] `grep 'version = "0.1.10"' crates/anvilml-server/Cargo.toml` confirms version bump
- [ ] `head -1 crates/anvilml-server/src/ws/broadcaster.rs` confirms the file exists
- [ ] `cargo clippy --package anvilml-server --features mock-hardware -- -D warnings` exits 0 (no lint warnings from new code)
