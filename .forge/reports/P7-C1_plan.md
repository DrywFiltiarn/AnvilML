# Plan Report: P7-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-C1                                       |
| Phase       | 007 — IPC Foundations                       |
| Description | anvilml-ipc: EventBroadcaster tokio::sync::broadcast wrapper |
| Depends on  | P3-A9                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T07:59:04Z                        |
| Attempt     | 1                                           |

## Objective

Create the `EventBroadcaster` struct in `crates/anvilml-ipc/src/ws/broadcaster.rs` — a thin wrapper around `tokio::sync::broadcast::Sender<WsEvent>` with a 1024-event buffer. This provides `publish()` and `subscribe()` methods that the future `GET /v1/events` WebSocket handler (Phase 8) will use to fan out `WsEvent` messages to all connected clients. It is placed in `anvilml-ipc` specifically to avoid a crate dependency cycle between `anvilml-worker` and `anvilml-server` per the dependency graph in `ANVILML_DESIGN.md §3.2`.

## Scope

### In Scope
- Create `crates/anvilml-ipc/src/ws/mod.rs` — module declaration.
- Create `crates/anvilml-ipc/src/ws/broadcaster.rs` — `EventBroadcaster` struct with `new()`, `publish()`, and `subscribe()` methods.
- Update `crates/anvilml-ipc/Cargo.toml` — add `tokio` dependency with `sync` feature.
- Update `crates/anvilml-ipc/src/lib.rs` — add `mod ws; pub use ws::broadcaster::EventBroadcaster;`.
- Create `crates/anvilml-ipc/tests/roundtrip_tests.rs` — ≥4 tests covering publish/subscribe behaviour.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferred scope. The WebSocket handler that consumes `EventBroadcaster` is a separate task. No dual-mode parity markers apply — `EventBroadcaster` is a Rust struct, not a Python node `execute()` or arch module `load()`/`sample()`/`decode()` function covered by the `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` convention (`ANVILML_DESIGN.md §10.6`).

## Existing Codebase Assessment

`anvilml-ipc` is currently an empty stub crate created in Phase 1's P1-B4. Its `lib.rs` contains only a one-line `//!` crate-level doc comment. No source modules, no test directory, and no `tokio` dependency exist yet.

The `WsEvent` type that this task consumes is already fully defined in `crates/anvilml-core/src/types/events.rs` (Phase 3, P3-A8/P3-A9). It is a `#[serde(tag = "type", rename_all = "snake_case")]` enum with ten variants (`JobQueued`, `JobStarted`, `JobProgress`, `JobImageReady`, `JobCompleted`, `JobFailed`, `JobCancelled`, `WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`). It derives `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`, and `ToSchema`. It is re-exported via `anvilml-core::types::events::*` → `anvilml_core::WsEvent`.

The established crate pattern in this project is: each crate has a `lib.rs` with ≤ 80 lines containing only `pub mod` / `pub use` directives and a `//!` crate-level doc comment; implementation lives in sibling `.rs` files or subdirectories; tests live in `crates/{name}/tests/` as separate test crates; and every `pub` item carries a `///` doc comment.

There is no gap between the design doc and current source that affects this task — the design specifies `EventBroadcaster` precisely, and the `WsEvent` type it wraps already exists and matches the spec.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.47.0          | rust-docs MCP  | sync                   |

The workspace already uses `tokio = { version = "1.47.0", features = ["full"] }` as a dev-dependency in `anvilml-core` and `anvilml-server`. This task adds `tokio` with only the `sync` feature (not `full`) as a direct dependency of `anvilml-ipc`, since `EventBroadcaster` only needs `tokio::sync::broadcast` — no runtime, no net, no io, no time.

API shape confirmed via `rust-docs MCP` on tokio v1.47.0:
- `tokio::sync::broadcast::Sender<T>::new(capacity: usize) -> Self`
- `tokio::sync::broadcast::Sender<T>::send(&self, value: T) -> Result<usize, SendError<T>>`
- `tokio::sync::broadcast::Sender<T>::subscribe(&self) -> Receiver<T>`

## Approach

1. **Update `Cargo.toml`**: Add `tokio = { version = "1.47.0", features = ["sync"] }` to `[dependencies]`. This is the only dependency change needed — `anvilml-core` is already listed and provides `WsEvent`.

2. **Create `ws/mod.rs`**: A single-line module declaration (`pub mod broadcaster;`). No additional content.

3. **Create `ws/broadcaster.rs`**: Implement `EventBroadcaster` as a newtype wrapping `tokio::sync::broadcast::Sender<WsEvent>`:
   - `pub fn new() -> Self` — calls `Sender::new(1024)` to create the broadcast channel with the 1024-event buffer capacity specified in `ANVILML_DESIGN.md §13.6`. Add a `///` doc comment describing the struct and the buffer capacity.
   - `pub fn publish(&self, event: WsEvent)` — calls `self.0.send(event)` and ignores the `Result` (both `Ok(count)` and `Err(SendError)` when there are zero subscribers are normal). Add a `///` doc comment explaining the `SendError`-ignoring behaviour.
   - `pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WsEvent>` — calls `self.0.subscribe()` and returns the receiver directly. Add a `///` doc comment.

   The struct uses a tuple struct (`pub struct EventBroadcaster(Sender<WsEvent>)`) to keep the implementation minimal and to avoid accidental direct field access. All three methods are synchronous (`fn`, not `async fn`) because `tokio::sync::broadcast::Sender::send()` and `subscribe()` are synchronous methods — no `.await` needed.

4. **Update `lib.rs`**: Add `pub mod ws;` and `pub use ws::broadcaster::EventBroadcaster;` below the existing `//!` doc comment. The file will remain well under the 80-line hard cap (approximately 3–5 lines total).

5. **Create `tests/roundtrip_tests.rs`**: Write ≥4 integration tests:
   - `test_publish_zero_subscribers` — creates an `EventBroadcaster`, publishes a `WsEvent` without any subscriber, asserts no panic (the `publish()` call ignores the `SendError`).
   - `test_publish_one_subscriber_delivers` — creates an `EventBroadcaster`, subscribes to get a `Receiver`, publishes a `WsEvent`, then calls `receiver.recv().await` and asserts the received event equals the published event.
   - `test_publish_multiple_subscribers_independent_copies` — creates an `EventBroadcaster`, subscribes twice to get two independent `Receiver`s, publishes one `WsEvent`, then calls `recv().await` on both receivers and asserts each received an equal copy.
   - `test_subscribe_returns_receiver` — creates an `EventBroadcaster`, calls `subscribe()`, and asserts the returned receiver is valid (e.g., `recv().await` does not return `RecvError::Closed` immediately).

   All tests use `#[tokio::test]` for async test support. The `tokio` dependency with `full` features is available in dev-dependencies via the `anvilml-core` transitive path, but since `tokio` is now a direct dependency of `anvilml-ipc` with only `sync`, the tests need `tokio` as a dev-dependency too — add `tokio = { version = "1.47.0", features = ["sync", "macros", "rt-multi-thread"] }` to `[dev-dependencies]` in `Cargo.toml` for test async support.

## Public API Surface

```rust
// crates/anvilml-ipc/src/ws/broadcaster.rs

/// A WebSocket event broadcaster wrapping tokio::sync::broadcast::Sender<WsEvent>.
///
/// Buffer capacity is 1024 events per ANVILML_DESIGN.md §13.6.
/// `publish()` ignores `SendError` — publishing with zero subscribers
/// is expected, normal behaviour and not an error condition.
#[derive(Debug)]
pub struct EventBroadcaster(tokio::sync::broadcast::Sender<WsEvent>);

impl EventBroadcaster {
    /// Create a new `EventBroadcaster` with a 1024-event broadcast buffer.
    pub fn new() -> Self;

    /// Publish an event to all current subscribers.
    ///
    /// If there are zero subscribers, `send()` returns `Err(SendError)` which
    /// is ignored — this is expected and not an error condition.
    pub fn publish(&self, event: WsEvent);

    /// Subscribe to receive future events.
    ///
    /// Returns a new `Receiver<WsEvent>` that can be awaited independently.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<WsEvent>;
}
```

Re-exported in `lib.rs` as `pub use ws::broadcaster::EventBroadcaster;`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/ws/mod.rs` | Module declaration for `broadcaster` |
| CREATE | `crates/anvilml-ipc/src/ws/broadcaster.rs` | `EventBroadcaster` struct and impl |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `mod ws; pub use ws::broadcaster::EventBroadcaster;` |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Add `tokio` dependency and dev-dependency |
| CREATE | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | ≥4 integration tests for publish/subscribe |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_publish_zero_subscribers` | `publish()` with zero subscribers does not panic or propagate error | `EventBroadcaster::new()` created, no subscribers | `WsEvent::JobQueued { job_id: Uuid::new_v4(), queue_position: 1 }` | `publish()` returns without panic (SendError silently ignored) | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_publish_zero_subscribers` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_publish_one_subscriber_delivers` | `publish()` with one subscriber delivers the event | `EventBroadcaster::new()` + one `subscribe()` call | One `WsEvent::JobStarted { job_id, worker_id: "gpu:0" }` | `receiver.recv().await` returns `Ok(event)` equal to published event | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_publish_one_subscriber_delivers` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_publish_multiple_subscribers_independent_copies` | Multiple subscribers each get their own copy of the event | `EventBroadcaster::new()` + two `subscribe()` calls | One `WsEvent::JobCompleted { job_id, elapsed_ms: 42 }` | Both `recv().await` calls return `Ok(event)` equal to published event | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_publish_multiple_subscribers_independent_copies` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_subscribe_returns_valid_receiver` | `subscribe()` returns a receiver that is not immediately closed | `EventBroadcaster::new()` created | None (structural test) | `recv().await` does not return `RecvError::Closed` before any publish | `cargo test -p anvilml-ipc --test roundtrip_tests -- test_subscribe_returns_valid_receiver` exits 0 |

## CI Impact

No CI changes required. The new `tests/roundtrip_tests.rs` file is automatically picked up by `cargo test --workspace --features mock-hardware` (the standard CI test command per `ENVIRONMENT.md §6 Step 6`). No new file types, gates, or configuration are introduced.

## Platform Considerations

None identified. The `tokio::sync::broadcast` channel is a pure in-memory, cross-platform primitive with no platform-specific behaviour. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::sync::broadcast::Sender::send()` signature may differ from what is documented — the MCP confirmed the v1.47.0 API, but if the workspace lockfile pins a different minor version, the API could diverge. | Low | Medium | The plan resolves the version via MCP (1.47.0) and the workspace already uses 1.47.0. If `Cargo.lock` shows a different version, the ACT agent should verify against the lockfile and adjust. |
| The `SendError` type from `tokio::sync::broadcast` may be `SendError<T>` (wrapping the lost value) or `SendError` (no value). Ignoring it via `.ok()` is correct either way since `publish()` returns `()`. | Low | Low | Confirmed via MCP: `Sender::send()` returns `Result<usize, SendError<T>>` where `SendError<T>` wraps `T`. Using `.ok()` discards both the count and the error, which is the correct behaviour. |
| `anvilml-ipc` currently has no test infrastructure — no `tests/` directory exists, so creating it for the first time is a structural change. | Low | Low | Straightforward: `mkdir -p crates/anvilml-ipc/tests/` then create the test file. The `cargo test -p anvilml-ipc --test roundtrip_tests` command will compile it as a separate test crate. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0
- [ ] `wc -l crates/anvilml-ipc/src/lib.rs` reports ≤ 80 lines
- [ ] `grep -c "^pub fn\|^pub struct" crates/anvilml-ipc/src/ws/broadcaster.rs` reports ≥ 3 (new, publish, subscribe)
- [ ] `grep -c "^fn test_" crates/anvilml-ipc/tests/roundtrip_tests.rs` reports ≥ 4
