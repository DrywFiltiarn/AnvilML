# Plan Report: P7-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-B2                                       |
| Phase       | 7 — IPC Foundations                         |
| Description | anvilml-ipc: RouterTransport send()/recv() split-lock methods |
| Depends on  | P7-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T22:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `pub async fn send()` and `pub async fn recv()` on `RouterTransport` in `crates/anvilml-ipc/src/transport.rs`, each locking only its own half (`sender` or `receiver`), to complete the split-lock design that structurally prevents the v3 shutdown deadlock. The `send()` method serializes a `WorkerMessage` via `rmp_serde::to_vec_named`, sends it as a three-frame multipart ROUTER message (worker_id identity + empty delimiter + msgpack payload). The `recv()` method receives a three-frame multipart message, extracts the worker identity and decodes the payload via `rmp_serde::from_slice` into a `WorkerEvent`. Both methods are tested with >=4 new tests including a roundtrip and a load-bearing concurrency regression test.

## Scope

### In Scope
- Implement `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), IpcError>` in `transport.rs`: lock `self.sender`, serialize via `rmp_serde::to_vec_named`, build a 3-frame `ZmqMessage` (identity + delimiter + payload), call `send()`, return on success or `IpcError::SendFailed`.
- Implement `pub async fn recv(&self) -> Result<(String, WorkerEvent), IpcError>` in `transport.rs`: lock `self.receiver`, call `recv()` to get a `ZmqMessage`, validate it has exactly 3 frames, extract identity (frame 0) and payload (frame 2), deserialize payload via `rmp_serde::from_slice` into `WorkerEvent`, return `(identity, event)` or `IpcError::RecvFailed`.
- Add >=4 new integration tests in `tests/roundtrip_tests.rs`: roundtrip send+recv with a real message/event pair, concurrent send+recv regression test (recv blocked does not block send), send with invalid worker_id, recv with malformed frames.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope. No deferrals.

## Existing Codebase Assessment

**What exists:** `RouterTransport` struct is already defined in `transport.rs` (P7-B1) with three fields: `sender: Arc<Mutex<RouterSendHalf>>`, `receiver: Arc<Mutex<RouterRecvHalf>>`, and `pub port: u16`. The `bind()` method creates a ROUTER socket, binds to `tcp://127.0.0.1:0`, extracts the port, splits the socket into independent send/recv halves, and wraps each in `Arc<Mutex<>>`. The struct is annotated `#[allow(dead_code)]` with comments explaining that `sender`/`receiver` are consumed by the deferred send/recv methods.

**Established patterns:**
- Error handling: all IPC operations return `Result<_, IpcError>` which maps to `AnvilError::Ipc(String)` via `From<IpcError>`. The `IpcError::SendFailed(String)` and `IpcError::RecvFailed(String)` variants already exist and are ready for use.
- Serialization: `rmp_serde::to_vec_named` / `rmp_serde::from_slice` for msgpack flat-dict serialization, declared in `[dev-dependencies]` of `Cargo.toml`.
- Test style: integration tests go in `crates/anvilml-ipc/tests/roundtrip_tests.rs`, using `#[tokio::test]` for async tests and `#[test]` for sync tests. Tests construct their own `RouterTransport::bind()` instances.
- Documentation: `///` doc comments on all `pub` items, explaining what the method does, its preconditions, and error variants.

**Gap between design doc and source:** The design doc (§8.3) specifies the return type as `Result<(), AnvilError>` for `send()` and `Result<(String, WorkerEvent), AnvilError>` for `recv()`, but the actual codebase uses `IpcError` (which converts to `AnvilError` via `From`). This is intentional — the methods return `IpcError` and callers convert at their discretion. The plan follows the actual codebase type.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source     | Feature flags confirmed |
|--------|----------|-----------------|----------------|------------------------|
| crate  | zeromq   | 0.6.0           | rust-docs MCP  | tokio-runtime, all-transport |
| crate  | rmp-serde| 1.3.1           | Cargo.toml     | n/a (dev-dependency)   |

**zeromq 0.6.0 API shape confirmed via MCP:** `RouterSendHalf` and `RouterRecvHalf` are produced by `RouterSocket::split()`. Both implement `SocketSend`/`SocketRecv` traits respectively, which provide `send(&mut self, message: ZmqMessage) -> ZmqResult<()>` and `recv(&mut self) -> ZmqResult<ZmqMessage>`. `ZmqMessage` has `push_back(frame: Bytes)` to append frames, `push_front(frame: Bytes)` to prepend frames, `into_vec() -> Vec<Bytes>` to extract frames, and `get(index)` to read individual frames. The `all-transport` feature includes `tcp-transport`, which is needed for the loopback bind.

**rmp-serde 1.3.1:** Already declared in `[dev-dependencies]`. `to_vec_named` and `from_slice` are the standard serialization functions used throughout the crate's existing roundtrip tests.

## Approach

### Step 1: Implement `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), IpcError>`

In `transport.rs`, add the `send` method to the `impl RouterTransport` block:

1. Lock `self.sender` via `self.sender.lock().await` — this is the ONLY lock acquired in this method.
2. Serialize `msg` via `rmp_serde::to_vec_named(msg)` — this produces a `Vec<u8>` of msgpack bytes. On serialization failure, return `IpcError::SerializationFailed(e.to_string())`.
3. Build a `zeromq::ZmqMessage`:
   - Create a new `ZmqMessage`
   - Push the msgpack payload bytes as the last frame (frame 2) using `push_back`
   - Push the empty delimiter as frame 1 using `push_back` (ZeroMQ ROUTER protocol requires an empty frame between identity and payload)
   - Push the `worker_id` as bytes as frame 0 (the identity frame) using `push_front`
   - This produces a 3-frame multipart message: `[worker_id, "", payload]`
4. Call `send_half.send(message).await` on the locked send half. On socket error, return `IpcError::SendFailed(e.to_string())`.
5. Return `Ok(())` on success.

**Rationale:** The 3-frame layout matches the ZeroMQ ROUTER socket protocol: identity frame tells ROUTER which DEALER to route to, the empty delimiter is a ROUTER-specific framing element, and the payload carries the actual message. Using `push_front` for the identity ensures it ends up as frame 0 (first frame sent on the wire).

### Step 2: Implement `pub async fn recv(&self) -> Result<(String, WorkerEvent), IpcError>`

In `transport.rs`, add the `recv` method to the `impl RouterTransport` block:

1. Lock `self.receiver` via `self.receiver.lock().await` — this is the ONLY lock acquired in this method. Never touches `self.sender`.
2. Call `recv_half.recv().await` to get a `ZmqMessage`. On socket error, return `IpcError::RecvFailed(e.to_string())`.
3. Convert the message to frames via `message.into_vec()` to get `Vec<Bytes>`.
4. Validate the frame count is exactly 3 (identity + delimiter + payload). If not, return `IpcError::RecvFailed(format!("expected 3 frames, got {}", frames.len()))`.
5. Extract the identity: `String::from_utf8(frames[0].to_vec()).map_err(|e| IpcError::RecvFailed(format!("invalid UTF-8 identity: {e}")))`
6. Extract the payload: `frames[2]` — skip frame 1 (empty delimiter) entirely.
7. Deserialize the payload: `rmp_serde::from_slice(&payload).map_err(|e| IpcError::RecvFailed(format!("deserialization failed: {e}")))` to get `WorkerEvent`.
8. Return `Ok((identity, event))`.

**Rationale:** The ROUTER socket always returns the identity frame first, followed by the delimiter, then the payload. We skip the delimiter since it carries no semantic information — it's purely a ROUTER protocol marker.

### Step 3: Add `use zeromq::ZmqMessage` import at the top of `transport.rs`

The existing imports include `use zeromq::prelude::*;` and `use zeromq::{Endpoint, RouterRecvHalf, RouterSendHalf, RouterSocket};`. Add `zeromq::ZmqMessage` to the zeromq imports or reference it as `zeromq::ZmqMessage` in the method bodies.

### Step 4: Remove `#[allow(dead_code)]` from `RouterTransport` and its fields

Since `sender` and `receiver` are now consumed by the send/recv methods, the `dead_code` suppressions are no longer needed. Remove the `#[allow(dead_code)]` on the struct and the `#[allow(dead_code)]` on the `receiver` field.

### Step 5: Add `#[tracing::instrument]` to both methods

Per `ANVILML_DESIGN.md §16.2` logging conventions and `FORGE_AGENT_RULES.md §11.6`, apply `#[tracing::instrument]` to both async methods as meaningful units of work.

### Step 6: Write integration tests in `tests/roundtrip_tests.rs`

Add the following tests (all use `#[tokio::test]` for async support):

**Test 1: `test_send_recv_roundtrip`** — Creates two `RouterTransport` instances (simulating a ROUTER server and a DEALER client via a separate socket). Sends a `WorkerMessage::Ping { seq: 42 }` from one transport to the other, then receives it and verifies the event matches. This is the fundamental send-then-recv roundtrip over a real loopback socket.

**Test 2: `test_concurrent_send_recv_does_not_block`** — Spawns `recv()` in a background task (which will block waiting for a message), then immediately calls `send()` from the main task. The send must complete without waiting for the recv to unblock. This is the load-bearing regression test for the v3 shutdown deadlock. Uses a `tokio::time::timeout` to bound the test.

**Test 3: `test_send_ping_then_recv_pong`** — Sends a `WorkerMessage::Ping { seq: 1 }` and receives the corresponding `WorkerEvent::Pong { seq: 1 }` over the bound socket. Verifies the seq field is preserved across the roundtrip.

**Test 4: `test_send_execute_message_roundtrip`** — Sends a `WorkerMessage::Execute` with a realistic graph and settings, receives it, and verifies all fields (job_id, graph, settings, device_index) are preserved. This exercises the most complex message variant.

**Test 5: `test_recv_malformed_frames_returns_error`** — Constructs a `ZmqMessage` with only 2 frames (missing the delimiter) and verifies that `recv()` returns `IpcError::RecvFailed` with an appropriate error message. This tests the frame-count validation path.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `RouterTransport::send` | `anvilml_ipc::transport::RouterTransport::send` | `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), IpcError>` |
| `RouterTransport::recv` | `anvilml_ipc::transport::RouterTransport::recv` | `pub async fn recv(&self) -> Result<(String, WorkerEvent), IpcError>` |

No new pub items beyond these two methods. The existing `RouterTransport` struct fields, `IpcError` variants, and message enums are all pre-existing.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/src/transport.rs` | Add `send()` and `recv()` methods; remove `#[allow(dead_code)]` suppressions; add `ZmqMessage` import |
| Modify | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Add >=5 new integration tests for send/recv |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_send_recv_roundtrip` | A `WorkerMessage::Ping` sent via `send()` is received as the matching `WorkerEvent::Pong` via `recv()` over a real loopback socket | `cargo test -p anvilml-ipc --test roundtrip_tests test_send_recv_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_concurrent_send_recv_does_not_block` | A blocked `recv()` does not prevent a concurrent `send()` from completing — the load-bearing deadlock regression test | `cargo test -p anvilml-ipc --test roundtrip_tests test_concurrent_send_recv_does_not_block` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_send_ping_then_recv_pong` | `Ping { seq: 1 }` roundtrips to `Pong { seq: 1 }` with seq preserved | `cargo test -p anvilml-ipc --test roundtrip_tests test_send_ping_then_recv_pong` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_send_execute_message_roundtrip` | Complex `WorkerMessage::Execute` with all fields roundtrips correctly | `cargo test -p anvilml-ipc --test roundtrip_tests test_send_execute_message_roundtrip` exits 0 |
| `crates/anvilml-ipc/tests/roundtrip_tests.rs` | `test_recv_malformed_frames_returns_error` | `recv()` with only 2 frames returns `IpcError::RecvFailed` | `cargo test -p anvilml-ipc --test roundtrip_tests test_recv_malformed_frames_returns_error` exits 0 |

## CI Impact

No CI changes required. The new tests are integration tests in the existing `tests/roundtrip_tests.rs` file, which is already picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The ROUTER socket on `tcp://127.0.0.1:0` is platform-neutral. The msgpack serialization via `rmp-serde` is also platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `zeromq::ZmqMessage` frame ordering may not match ROUTER expectations — `push_front`/`push_back` order must produce `[identity, delimiter, payload]` on the wire. | Medium | High | Read the zeromq 0.6 ROUTER send example before writing send(); write a single-frame test first to confirm frame layout, then the 3-frame send. |
| `recv()` blocks indefinitely if no DEALER connects — the test must use a timeout or spawn a concurrent send to unblock it. | Medium | High | The concurrency regression test uses `tokio::time::timeout` with a 5-second bound. The roundtrip test spawns a DEALER-side socket that sends and then receives. |
| `rmp_serde::to_vec_named` on `WorkerMessage` may produce a different serialization format than the existing roundtrip tests expect, causing `from_slice` to fail on the recv side. | Low | Medium | The existing roundtrip tests already use `to_vec_named`/`from_slice` for all message variants — the same functions are used here. The serialization is deterministic and consistent within a crate version. |
| The `dead_code` suppression removal causes clippy warnings if `send()`/`recv()` are not `pub` enough to be considered "used". | Low | Low | Both methods are `pub async fn` on a `pub struct`, so they are used by external code. Clippy will not warn. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --test roundtrip_tests` exits 0 (all existing + new tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings from new code)
- [ ] `cargo fmt --all -- --check` exits 0 (code is formatted)
- [ ] `wc -l crates/anvilml-ipc/src/transport.rs` prints a number ≤ 400 (file size guideline)
- [ ] `grep -c "test_send\|test_concurrent\|test_recv_malformed" crates/anvilml-ipc/tests/roundtrip_tests.rs` prints >= 5 (new test count)
