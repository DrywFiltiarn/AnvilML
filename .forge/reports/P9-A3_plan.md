# Plan Report: P9-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-A3                                       |
| Phase       | 009 — Worker Spawn & Handshake              |
| Description | anvilml-worker: bridge.rs two independent IPC reader/writer tasks |
| Depends on  | P9-A1, P9-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-16T18:20:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-worker/src/bridge.rs` with `pub fn start()` that spawns two independent tokio tasks — a writer task that receives `WorkerMessage` from an `mpsc::Receiver` and sends each via `RouterTransport`, and a reader task that receives `(String, WorkerEvent)` from the transport and broadcasts each via a `broadcast::Sender`. This is the IPC bridge that connects `ManagedWorker`'s message channel and event broadcast to the shared ZeroMQ ROUTER socket. When complete, `cargo test -p anvilml-worker --features mock-hardware -- bridge` exits 0 with ≥ 3 tests.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/bridge.rs` — the `start()` function and two tokio task closures
- Create `crates/anvilml-worker/tests/bridge_tests.rs` — ≥ 3 integration tests
- Update `crates/anvilml-worker/src/lib.rs` to `pub mod bridge;` and `pub use bridge::start;`
- Bump `anvilml-worker` patch version from `0.1.2` to `0.1.3` in `Cargo.toml`

### Out of Scope
- `managed.rs` — integration of bridge handles into ManagedWorker (P9-A5)
- `keepalive.rs` — heartbeat logic (P9-A4)
- `pool.rs` — WorkerPool (P9-A6)
- Python worker changes (Group B)
- Any new external crate dependencies — all types come from existing workspace deps

## Existing Codebase Assessment

The `anvilml-worker` crate currently exports only `env` and `spawn` modules with their public functions (`build_worker_env`, `build_command`). The `lib.rs` has 13 lines with a crate-level doc comment, `pub mod` declarations, and `pub use` re-exports — following the established pattern.

The `anvilml-ipc` crate provides `RouterTransport` (in `transport.rs`) with `bind()`, `send(&self, worker_id: &[u8], msg: &WorkerMessage) -> Result<(), TransportError>`, and `recv(&self) -> Result<(String, WorkerEvent), AnvilError>`. The inner socket is `Arc<Mutex<RouterSocket>>` and the `socket` field is `pub` — this allows tests to access the raw socket for integration testing.

The existing test files (`env_tests.rs`, `spawn_tests.rs`) use `#[test]` functions (not `#[tokio::test]`) with synchronous assertions. For bridge tests, `#[tokio::test]` is required because the bridge function spawns async tasks. The test fixtures use minimal structs constructed with explicit field values.

No gap or discrepancy was found between the design doc (§9.5) and the current source. The design doc's code snippet for the bridge tasks matches the actual `RouterTransport` API shape on disk.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source  | Feature flags confirmed |
|--------|------------|-----------------|-------------|------------------------|
| crate  | tokio      | 1.52.3          | Cargo.lock  | full (sync::mpsc, sync::broadcast, task) |
| crate  | zeromq     | 0.6.0           | Cargo.lock  | tokio-runtime, tcp-transport |
| crate  | anvilml-ipc| 0.1.4           | Cargo.lock  | (workspace path dep)   |

No new external dependencies are introduced. All types (`tokio::sync::mpsc::Receiver`, `tokio::sync::broadcast::Sender`, `tokio::task::JoinHandle`, `std::sync::Arc`) come from `tokio` which is already a workspace dependency with `features = ["full"]`. The `RouterTransport` type is from `anvilml-ipc` (path dependency).

## Approach

1. **Create `crates/anvilml-worker/src/bridge.rs`** with module-level doc comment explaining the bridge's role (two independent tokio tasks for IPC routing). Include imports: `std::sync::Arc`, `tokio::sync::{mpsc, broadcast}`, `tokio::task::JoinHandle`, `tracing`, `anvilml_ipc::{RouterTransport, WorkerMessage, WorkerEvent}`.

2. **Implement `pub fn start(...)`** with the exact signature:
   ```rust
   pub fn start(
       transport: Arc<RouterTransport>,
       worker_id: String,
       msg_rx: mpsc::Receiver<WorkerMessage>,
       event_tx: broadcast::Sender<(String, WorkerEvent)>,
   ) -> (JoinHandle<()>, JoinHandle<()>)
   ```
   Rationale for the signature: `msg_rx` is a `Receiver` (not `Sender`) because the caller owns the sender side and passes the receiver to the bridge. The function returns both `JoinHandle`s so the caller can store them in `ManagedWorker`.

3. **Writer task closure** (spawned first):
   ```rust
   let writer_handle = tokio::spawn(async move {
       while let Some(msg) = msg_rx.recv().await {
           if let Err(e) = transport.send(worker_id.as_bytes(), &msg).await {
               tracing::warn!(worker_id = %worker_id, error = %e, "writer send failed");
           }
           tracing::debug!(worker_id = %worker_id, msg_type = ?msg, "message sent to worker");
       }
       tracing::debug!(worker_id = %worker_id, "writer task ended (channel closed)");
   });
   ```
   Rationale: The writer uses `while let Some(msg) = msg_rx.recv().await` — when the channel sender is dropped (e.g., ManagedWorker shutdown), `recv()` returns `None` and the loop exits cleanly. Errors from `transport.send()` are logged at WARN but do not abort the task, because the transport may temporarily be unavailable during worker respawn.

4. **Reader task closure** (spawned second):
   ```rust
   let reader_handle = tokio::spawn(async move {
       loop {
           match transport.recv().await {
               Ok((id, event)) => {
                   tracing::debug!(worker_id = %worker_id, event_type = ?event, "event received from worker");
                   let _ = event_tx.send((id, event));
               }
               Err(e) => {
                   tracing::warn!(worker_id = %worker_id, error = %e, "reader recv failed, stopping");
                   break;
               }
           }
       }
   });
   ```
   Rationale: The reader uses a `loop { match ... }` pattern (not `while let`) because `transport.recv()` returns `Result`, not `Option`. On error (e.g., socket closed), the reader logs at WARN and breaks out of the loop. This is different from the writer because transport errors are terminal for the reader — if the socket is closed, there is no point retrying.

5. **Return both handles**: `return (writer_handle, reader_handle)`.

6. **Update `crates/anvilml-worker/src/lib.rs`**: Add `pub mod bridge;` and `pub use bridge::start;` after the existing `pub mod spawn;` line. The file will have 15 lines — well within the 80-line limit.

7. **Create `crates/anvilml-worker/tests/bridge_tests.rs`** with ≥ 3 tests:
   - `test_writer_sends_message`: Spawns a real ROUTER socket, connects a DEALER client in a background thread, runs the bridge writer, sends a message through the mpsc channel, then verifies the message appears on the ROUTER socket (by reading it from the DEALER side).
   - `test_reader_broadcasts_event`: Spawns a ROUTER socket and bridge reader, sends an event from a background DEALER thread, verifies the reader receives and broadcasts it, then closes the transport to trigger graceful shutdown.
   - `test_handles_drop_cleanly`: Creates a ROUTER socket, spawns both bridge tasks with dummy channels, drops both handles, and asserts no panic (the tasks will exit when their respective channels/transport close).

   Rationale for integration-test approach: The bridge operates on a real `RouterTransport` backed by a ZeroMQ ROUTER socket. Unit testing would require mocking the socket, which is not feasible with the zeromq 0.6 API. Integration tests using a real ROUTER + DEALER pair exercise the actual code paths.

8. **Bump `anvilml-worker` version** from `0.1.2` to `0.1.3` in `crates/anvilml-worker/Cargo.toml`.

9. **Verify**: Run `cargo test -p anvilml-worker --features mock-hardware -- bridge` and confirm exit 0 with ≥ 3 tests.

### Logging applied (per ENVIRONMENT.md §9 mandatory DEBUG log points)

| File | Location | Log call | Mandatory field |
|------|----------|----------|----------------|
| bridge.rs | writer task body | `tracing::debug!(worker_id = %worker_id, msg_type = ?msg, "message sent to worker")` | IPC — message sent to a worker |
| bridge.rs | reader task body | `tracing::debug!(worker_id = %worker_id, event_type = ?event, "event received from worker")` | IPC — event received from a worker |
| bridge.rs | writer task body | `tracing::warn!(worker_id = %worker_id, error = %e, "writer send failed")` | Non-obvious error path |
| bridge.rs | reader task body | `tracing::warn!(worker_id = %worker_id, error = %e, "reader recv failed, stopping")` | Non-obvious error path |

### Documentation applied

- Module-level `//!` doc comment on `bridge.rs` (crate-level doc)
- `///` doc comment on `pub fn start()` describing parameters, return value, and lifecycle
- Inline `//` comments at non-trivial decision points (why writer uses `while let`, why reader uses `loop { match }`)

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `pub fn start` | `anvilml_worker::start` | `pub fn start(transport: Arc<RouterTransport>, worker_id: String, msg_rx: mpsc::Receiver<WorkerMessage>, event_tx: broadcast::Sender<(String, WorkerEvent)>) -> (JoinHandle<()>, JoinHandle<()>)` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/bridge.rs` | IPC bridge: two independent tokio reader/writer tasks |
| CREATE | `crates/anvilml-worker/tests/bridge_tests.rs` | Integration tests for bridge tasks |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `pub mod bridge;` and `pub use bridge::start;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.2` → `0.1.3` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/bridge_tests.rs` | `test_writer_sends_message` | Writer task receives from mpsc channel and sends via ROUTER socket; terminates when channel closes. | ROUTER socket bound, DEALER client connected. | Send `WorkerMessage::Ping { seq: 1 }` through mpsc channel. | DEALER client receives msgpack-encoded Ping; writer task exits cleanly. | `cargo test -p anvilml-worker --features mock-hardware -- bridge_tests::test_writer_sends_message` exits 0 |
| `crates/anvilml-worker/tests/bridge_tests.rs` | `test_reader_broadcasts_event` | Reader task receives from ROUTER socket, broadcasts via event_tx; terminates when transport socket closes. | ROUTER socket bound, DEALER client connected. | Send `WorkerEvent::Pong { seq: 1 }` from DEALER thread; close transport after receive. | Event is broadcast on channel; reader task exits cleanly. | `cargo test -p anvilml-worker --features mock-hardware -- bridge_tests::test_reader_broadcasts_event` exits 0 |
| `crates/anvilml-worker/tests/bridge_tests.rs` | `test_handles_drop_cleanly` | Dropping both JoinHandles does not panic; tasks terminate. | ROUTER socket bound, dummy mpsc channel. | Create bridge, drop handles immediately. | No panic; both handles resolve. | `cargo test -p anvilml-worker --features mock-hardware -- bridge_tests::test_handles_drop_cleanly` exits 0 |

## CI Impact

No CI changes required. The new test file `bridge_tests.rs` lives in `crates/anvilml-worker/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware` (the CI command). The test uses `#[tokio::test]` which requires the tokio runtime — already available via `tokio = { workspace = true }` with `features = ["full"]`.

## Platform Considerations

None identified. The bridge code uses only `tokio::spawn`, `mpsc`, `broadcast`, and `Arc` — all platform-neutral async primitives. The underlying `RouterTransport::send()` and `recv()` use ZeroMQ which operates identically on Linux and Windows (per ENVIRONMENT.md §2). No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `RouterTransport::send()` method takes `worker_id: &[u8]`, but the task context says `worker_id: String`. The writer must call `.as_bytes()` to convert. If the ACT agent uses `&worker_id` directly (string reference), it will fail to compile. | Medium | High | The plan explicitly documents the conversion: `transport.send(worker_id.as_bytes(), &msg).await`. The approach step 3 shows the exact call. |
| Integration tests need a real ZeroMQ ROUTER + DEALER pair. If the zeromq 0.6 API for `RouterSocket::recv()` or `DealerSocket::connect()` differs from what the codebase inspection reveals, tests will fail to compile. | Low | High | The approach was verified against the actual `transport.rs` source on disk (lines 155, 213). The `socket: InnerSocket` field is `pub`, allowing test code to access the raw socket. |
| `broadcast::Sender::send()` returns `Result<(), RecvError>`. If the broadcast channel has no receivers, `send()` returns `Err(RecvError::Closed)`. The reader task currently ignores this error with `let _ = ...`. This is intentional but could mask bugs. | Low | Low | The reader only broadcasts events it actually receives from the transport, so there will always be at least one receiver (ManagedWorker). The `let _ =` is a deliberate no-op on error. |
| The test `test_reader_broadcasts_event` needs to close the transport to trigger reader shutdown. Closing the underlying `RouterSocket` while the reader holds the mutex lock could cause a race. | Medium | Medium | The test acquires the mutex lock (via `transport.socket.lock().await`), drops the socket, then releases the lock — all before the reader's next iteration. This is safe because the reader can only call `recv()` after dropping its previous lock. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- bridge` exits 0 with ≥ 3 tests
- [ ] `cargo check -p anvilml-worker --features mock-hardware` exits 0 (bridge module compiles)
- [ ] `grep "^pub use bridge::start;" crates/anvilml-worker/src/lib.rs` returns non-empty output (lib.rs exports bridge)
- [ ] `grep "^version = \"0.1.3\"" crates/anvilml-worker/Cargo.toml` returns non-empty output (version bumped)
- [ ] `grep "^## " .forge/reports/P9-A3_plan.md` returns exactly 11 lines (plan report structure verified)
