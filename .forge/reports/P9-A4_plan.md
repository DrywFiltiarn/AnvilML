# Plan Report: P9-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A4                                       |
| Phase       | 009 — Worker Spawn & Handshake              |
| Description | anvilml-worker: keepalive.rs Ping/Pong heartbeat and pong timeout watchdog |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T17:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `crates/anvilml-worker/src/keepalive.rs` — a background tokio task that periodically sends `Ping{seq}` messages to a worker and waits for matching `Pong{seq}` responses. If a pong is not received within `pong_timeout`, the task invokes an `on_timeout` callback. This provides liveness detection for the worker subprocess, complementing the bridge reader/writer tasks (P9-A3) that handle general IPC message routing.

When complete, the `start()` function is available as `anvilml_worker::start_keepalive` (or `keepalive::start`) and can be spawned from `ManagedWorker::spawn()` to monitor worker liveness. The observable state is a join handle that runs the heartbeat loop and calls `on_timeout` when the worker becomes unresponsive.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/keepalive.rs` with:
  - `pub fn start(worker_id: String, tx: mpsc::Sender<WorkerMessage>, event_rx: broadcast::Receiver<(String, WorkerEvent)>, ping_interval: Duration, pong_timeout: Duration, on_timeout: impl Fn() + Send + 'static) -> JoinHandle<()>`
  - Internal `HeartbeatHandle` struct for clean shutdown
  - Ping/pong matching logic using the sequence number
  - Pong timeout watchdog using `tokio::time::sleep`
- Create `crates/anvilml-worker/tests/keepalive_tests.rs` with:
  - Test: timeout fires within `pong_timeout + 100ms` when no pong is sent
  - Test: pong resets the deadline (no timeout fires)
  - Test: sequence number increments across multiple pings
- Export `keepalive` module and `start` function from `crates/anvilml-worker/src/lib.rs`
- Bump `anvilml-worker` patch version from `0.1.3` to `0.1.4`

### Out of Scope
- Integration with `ManagedWorker` state machine (handled in P9-A5)
- Worker-side pong response (handled in P9-B1, Python worker)
- Adjusting ping interval or pong timeout from config (hardcoded defaults are fine for this task)
- Logging at INFO level for keepalive events (the subsystem is not listed in ENVIRONMENT.md §9 mandatory INFO log points; DEBUG logging is included)

## Existing Codebase Assessment

The `anvilml-worker` crate currently has three source modules (`bridge.rs`, `env.rs`, `spawn.rs`) and three test files (`bridge_tests.rs`, `env_tests.rs`, `spawn_tests.rs`). The `lib.rs` declares `pub mod bridge`, `pub mod env`, `pub mod spawn`, and re-exports `start`, `build_worker_env`, `build_command`.

The `keepalive.rs` module does not yet exist — it is being created as part of this task. The `lib.rs` will be modified to add `pub mod keepalive` and `pub use keepalive::start;`.

The existing bridge tests (`bridge_tests.rs`) demonstrate the established test patterns: they create a real `RouterTransport::bind()`, connect a `DealerSocket`, discover the DEALER's auto-generated identity via a probe, and use `tokio::time::timeout` for timing assertions. The tests use `#[tokio::test]` async functions and import types from `anvilml_ipc` and `anvilml_worker` directly.

The `WorkerMessage::Ping { seq: u64 }` and `WorkerEvent::Pong { seq: u64 }` types are already defined in `anvilml-ipc/src/messages.rs` with the `#[serde(tag = "_type")]` discriminator. The `mpsc::Sender<WorkerMessage>` type is used by the bridge writer task. The `broadcast::Receiver<(String, WorkerEvent)>` type is used by the bridge reader task.

No gap or discrepancy exists between the design doc and current source for this task. The types and channel patterns are already established and match the task specification.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source   | Feature flags confirmed |
|--------|------------|-----------------|--------------|------------------------|
| crate  | tokio      | 1.52.3          | Cargo.lock   | full (time, sync, macros) |

No new external dependencies are introduced. The task uses only `tokio` types (`JoinHandle`, `mpsc::Sender`, `broadcast::Receiver`, `Duration`, `time::sleep`) and types from existing workspace dependencies (`WorkerMessage`, `WorkerEvent`). The `tracing` crate is already a dependency for logging.

## Approach

1. **Create `crates/anvilml-worker/src/keepalive.rs`** with module-level doc comment describing the heartbeat protocol: periodic Ping messages, expected Pong responses, and timeout callback invocation.

2. **Implement `pub fn start(...)`** — the public entry point. It takes:
   - `worker_id: String` — for logging only
   - `tx: mpsc::Sender<WorkerMessage>` — channel to send Ping messages (same sender used by bridge writer)
   - `event_rx: broadcast::Receiver<(String, WorkerEvent)>` — channel to receive events from the bridge reader
   - `ping_interval: Duration` — how often to send Ping (default 30s per design doc §9.1)
   - `pong_timeout: Duration` — max wait for matching Pong (default 10s per design doc §9.1)
   - `on_timeout: impl Fn() + Send + 'static` — callback invoked when pong timeout fires

   The function spawns a tokio task and returns its `JoinHandle`.

3. **Implement the heartbeat loop** inside the spawned task:
   - Initialize `seq: u64 = 0`
   - Enter an infinite loop:
     a. Increment `seq`
     b. Send `WorkerMessage::Ping { seq }` via `tx` (log at DEBUG with `worker_id`, `seq`)
     c. Set `pong_deadline = Instant::now() + pong_timeout`
     d. Enter a `select!` loop between:
        - `event_rx.recv()` — if event is `Pong { seq: received_seq }` matching `seq`, break out of inner loop (continue to next ping cycle)
        - `tokio::time::sleep_until(pong_deadline)` — if deadline passes, call `on_timeout()` and log at WARN (`worker_id`, `seq`), then continue to next ping cycle
   - Handle broadcast channel disconnect: if `event_rx.recv()` returns `Err(broadcast::error::RecvError::Lagged(n))`, skip the lagged events and continue. If it returns `Err(broadcast::error::RecvError::Closed)`, exit the loop cleanly.

   Rationale: Use `select!` with a deadline sleep rather than `tokio::time::interval` because we need per-ping deadline tracking (the deadline is relative to each ping send, not a fixed interval).

4. **Implement `HeartbeatHandle`** — a struct with an `Arc<tokio::sync::Mutex<bool>>` shutdown flag. The loop checks this flag on each iteration to allow clean shutdown from `ManagedWorker`.

   Rationale: The handle allows `ManagedWorker` to signal the heartbeat to stop when the worker transitions to Dead or when the managed worker itself is dropped.

5. **Add logging**:
   - DEBUG: `tracing::debug!(worker_id = %worker_id, seq = %seq, "sending ping")` before each Ping send
   - DEBUG: `tracing::debug!(worker_id = %worker_id, seq = %seq, "received pong")` when matching Pong arrives
   - WARN: `tracing::warn!(worker_id = %worker_id, seq = %seq, "pong timeout — worker may be unresponsive")` when timeout fires

6. **Export from `lib.rs`**: Add `pub mod keepalive;` and `pub use keepalive::start;` to `crates/anvilml-worker/src/lib.rs`.

7. **Create `crates/anvilml-worker/tests/keepalive_tests.rs`** with three tests:
   - `test_timeout_fires`: No pong sent; assert `on_timeout` is called within `pong_timeout + 100ms`
   - `test_pong_resets_deadline`: Pong sent for each ping; assert `on_timeout` is never called after multiple ping cycles
   - `test_seq_increments`: Track sent ping sequence numbers via a channel; assert they increment

8. **Bump `anvilml-worker` patch version** from `0.1.3` to `0.1.4` in `crates/anvilml-worker/Cargo.toml`.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `start` | fn | `anvilml_worker::keepalive::start` | `pub fn start(worker_id: String, tx: mpsc::Sender<WorkerMessage>, event_rx: broadcast::Receiver<(String, WorkerEvent)>, ping_interval: Duration, pong_timeout: Duration, on_timeout: impl Fn() + Send + 'static) -> JoinHandle<()>` |
| `HeartbeatHandle` | struct | `anvilml_worker::keepalive::HeartbeatHandle` | `pub struct HeartbeatHandle { /* private */ }` (not pub) |

Only `start` is a new public item. `HeartbeatHandle` is private to the module. The `lib.rs` re-export `pub use keepalive::start;` makes it available at `anvilml_worker::start_keepalive` — but note: `lib.rs` already has `pub use bridge::start;` which would conflict. The correct approach is to re-export as `pub use keepalive::start as start_keepalive;` in `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/keepalive.rs` | Heartbeat loop: Ping/Pong matching, timeout watchdog |
| CREATE | `crates/anvilml-worker/tests/keepalive_tests.rs` | Integration tests for keepalive timeout and pong reset |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `pub mod keepalive;` and `pub use keepalive::start as start_keepalive;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/keepalive_tests.rs` | `test_timeout_fires` | Timeout callback fires within `pong_timeout + 100ms` when no pong is sent | `event_rx` has no incoming Pong events | `ping_interval=500ms`, `pong_timeout=1s`, `on_timeout` flag | `on_timeout` called within timeout window | `cargo test -p anvilml-worker --features mock-hardware -- keepalive` exits 0 |
| `crates/anvilml-worker/tests/keepalive_tests.rs` | `test_pong_resets_deadline` | Pong matching resets deadline; no timeout fires across multiple ping cycles | Bridge reader delivers Pong for each Ping | `ping_interval=500ms`, `pong_timeout=2s`, 3+ ping cycles | `on_timeout` never called | `cargo test -p anvilml-worker --features mock-hardware -- keepalive` exits 0 |
| `crates/anvilml-worker/tests/keepalive_tests.rs` | `test_seq_increments` | Sequence number increments monotonically across ping sends | Channel captures sent Ping messages | `ping_interval=50ms`, `pong_timeout=1s` | Seq values: 0, 1, 2, ... | `cargo test -p anvilml-worker --features mock-hardware -- keepalive` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/keepalive_tests.rs` is picked up automatically by `cargo test --workspace --features mock-hardware` which runs the full Rust test suite on all CI runners. The new module does not change any CI job configuration.

## Platform Considerations

None identified. The keepalive module uses only `tokio` async primitives (`JoinHandle`, `mpsc`, `broadcast`, `time::sleep`, `Instant`) which are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `broadcast::Receiver::recv()` returns `Err(Lagged(n))` when the heartbeat task falls behind on events; the lagged events must be drained before the next `recv()` succeeds. | Medium | Medium | Use `loop { match event_rx.recv() { Ok(event) => break, Err(RecvError::Lagged(n)) => { tracing::debug!(worker_id = %worker_id, dropped = %n, "heartbeat dropped events"); continue }, Err(RecvError::Closed) => return } }` to drain lagged events before processing. |
| The `tx` channel sender is shared with the bridge writer task; sending a Ping while the bridge writer is also sending may cause the message order to be non-deterministic. Since Ping messages are matched by sequence number (not by arrival order), this is acceptable but worth noting. | Low | Low | The Ping/pong matching uses sequence numbers, not arrival order. Out-of-order delivery from the shared sender is harmless as long as every Ping eventually reaches the worker and the Pong returns with the correct seq. |
| `on_timeout` callback may panic; a panic in the callback would unwind the heartbeat task but could propagate to the tokio runtime. | Low | Medium | Wrap `on_timeout()` call in `std::panic::catch_unwind` with `AssertUnwindSafe`. Log at ERROR if the callback panics and continue the heartbeat loop rather than aborting. |
| `lib.rs` already has `pub use bridge::start;` — adding `pub use keepalive::start;` would create a duplicate name conflict. | High | High | Re-export as `pub use keepalive::start as start_keepalive;` to avoid the name collision. Update the task description and tests to reference `start_keepalive`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- keepalive` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `grep '^pub use keepalive::start as start_keepalive;' crates/anvilml-worker/src/lib.rs` returns 1 line (module is exported)
- [ ] `head -1 crates/anvilml-worker/src/keepalive.rs` begins with `//! ` (doc comment present)
- [ ] `grep '^version' crates/anvilml-worker/Cargo.toml` shows `version = "0.1.4"` (patch bumped)
