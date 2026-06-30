# Plan Report: P8-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-A1                                       |
| Phase       | 008 — IPC Stress Gate & Worker Pool         |
| Description | anvilml-ipc: 1000-round-trip ROUTER/DEALER stress test (GATE) |
| Depends on  | P7-D1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-01T00:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-ipc/tests/stress_test.rs` — a single integration test file that proves the IPC transport (ZeroMQ ROUTER ↔ DEALER over TCP loopback, with msgpack serialisation) survives 1000 sustained round-trips with zero message loss or reordering. The test binds a `RouterTransport`, spawns a Rust-side simulated DEALER worker in a background task, sends 1000 `WorkerMessage::Ping { seq }` messages with monotonically increasing `seq` values (1–1000), and asserts that all 1000 matching `WorkerEvent::Pong { seq }` responses arrive with the correct sequence numbers in order. This test gates Phase 8 and every subsequent phase per `ANVILML_DESIGN.md §20`'s IPC Baseline roadmap entry.

## Scope

### In Scope
- Create `crates/anvilml-ipc/tests/stress_test.rs` with a single test function `test_1000_roundtrips` that:
  - Binds a `RouterTransport` via `RouterTransport::bind()`.
  - Spawns a simulated DEALER worker task that loops: receive a `Pong` event, echo it back with matching `seq`.
  - Sends 1000 `WorkerMessage::Ping { seq: 1..=1000 }` messages sequentially.
  - Receives 1000 `WorkerEvent::Pong { seq }` responses and validates each `seq` matches the expected value.
  - Uses explicit `tokio::time::timeout` on every blocking socket operation (per `ENVIRONMENT.md §11.5`).
  - Asserts zero message loss (1000 sent = 1000 received) and zero reordering (seq values in ascending order).

### Out of Scope
- No real Python subprocess — the simulated DEALER is Rust-only, within the same test process.
- No worker pool, spawn, or supervision logic — that is `anvilml-worker`'s scope (P8-B1+).
- No dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) — these apply only to node `execute()` and arch-module `load()`/`sample()`/`decode()`/`compute_latent_shape()` per `ANVILML_DESIGN.md §10.6`; this is an IPC transport test, not a node function.
- No changes to existing files (no `lib.rs`, `transport.rs`, `messages.rs`, or `Cargo.toml` modifications).
- defers_to (from JSON): `[]` — this task must implement its full scope; no deferrals permitted.

## Existing Codebase Assessment

**What already exists:** The `anvilml-ipc` crate is fully implemented through Phase 7. `RouterTransport` (in `transport.rs`) provides `bind()`, `send()`, and `recv()` with split send/recv mutex halves. `WorkerMessage` and `WorkerEvent` enums (in `messages.rs`) include `Ping { seq }` and `Pong { seq }` variants. The `roundtrip_tests.rs` file already demonstrates the exact pattern needed: a `make_dealer()` helper that creates a `DealerSocket` with a peer identity, spawns it in a background task, and sends/receives through the router.

**Established patterns:** Tests use `#[tokio::test]` for async, `Arc<RouterTransport>` for sharing across tasks, `tokio::time::timeout` for bounded waits, `tokio::spawn` for background DEALER tasks, and `bytes::Bytes` with `zeromq::ZmqMessage` for building multipart messages. The ROUTER socket expects 3-frame messages (`[identity, "", payload]`) on send; the DEALER sends 2-frame messages (`["", payload]`) since the ROUTER prepends identity automatically.

**Gap between design and source:** None identified. The existing `make_dealer` helper in `roundtrip_tests.rs` is directly reusable. The `RouterTransport::send()` and `recv()` signatures match exactly what the stress test needs. No API changes or new types are required.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | zeromq     | 0.6.0           | rust-docs MCP  | tokio-runtime, all-transport |
| crate  | tokio      | 1.47.0          | rust-docs MCP  | sync, macros, rt-multi-thread |
| crate  | rmp-serde  | 1.3.1           | rust-docs MCP  | (none)                  |

All versions are confirmed via MCP against the `anvilml-ipc` Cargo.toml. The zeromq 0.6 API used (`RouterSocket`, `DealerSocket`, `RouterSendHalf`, `RouterRecvHalf`, `ZmqMessage`, `SocketOptions`, `PeerIdentity`, `Endpoint`) is confirmed present in the resolved version.

## Approach

1. **Create `crates/anvilml-ipc/tests/stress_test.rs`.** This file is a standalone test crate that uses `anvilml-ipc`'s public API (`RouterTransport`, `WorkerMessage`, `WorkerEvent`, `IpcError`) through the `use anvilml_ipc::...` imports.

2. **Implement `test_1000_roundtrips` function.** This is the single test function that exercises the full 1000-round-trip scenario:
   - Bind a `RouterTransport` via `RouterTransport::bind().await`.
   - Clone `Arc<RouterTransport>` for the receiver side (the main task will call `recv()`).
   - Spawn a simulated DEALER worker task via `tokio::spawn`:
     - Create a `DealerSocket` with peer identity `"stress-worker"` using `SocketOptions` + `PeerIdentity` (pattern from `roundtrip_tests.rs`'s `make_dealer()`).
     - Connect to `tcp://127.0.0.1:{port}`.
     - Loop 1000 times: for each `seq` value (1..=1000), serialize `WorkerEvent::Pong { seq }` via `rmp_serde::to_vec_named`, build a 2-frame `ZmqMessage` (`["", payload]`), and send it. Each send is wrapped in `tokio::time::timeout(Duration::from_secs(5), ...)` to bound the wait.
   - In the main task, loop 1000 times: call `recv()` with `tokio::time::timeout(Duration::from_secs(5), ...)` to bound the wait. On timeout, surface the captured stderr-equivalent by calling `handle.abort()` and asserting the timeout did not occur.
   - For each received `(identity, event)` pair, assert `identity == "stress-worker"` and extract `seq` from `WorkerEvent::Pong { seq }`, asserting it equals the expected value (`i + 1`).
   - After the loop, assert the background task completed with `handle.await.expect(...)`.

3. **Timeout strategy.** Per `ENVIRONMENT.md §11.5`, every blocking socket operation is wrapped in `tokio::time::timeout(Duration::from_secs(5), ...)`. A timeout failure aborts the background DEALER task and asserts with a clear error message. The 5-second per-message timeout is generous for loopback TCP (sub-millisecond latency expected) but provides a safety net against hangs.

4. **No additional test functions.** The task specifies a single 1000-round-trip test. The `roundtrip_tests.rs` file already covers individual message serialization, single-roundtrip send/recv, concurrent send/recv lock independence, and malformed frame handling.

## Public API Surface

No new public items are introduced. This task reads only existing public API:
- `anvilml_ipc::RouterTransport::bind() -> Result<Self, IpcError>` (async)
- `anvilml_ipc::RouterTransport::send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), IpcError>` (async)
- `anvilml_ipc::RouterTransport::recv(&self) -> Result<(String, WorkerEvent), IpcError>` (async)
- `anvilml_ipc::WorkerMessage::Ping { seq: u64 }`
- `anvilml_ipc::WorkerEvent::Pong { seq: u64 }`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/tests/stress_test.rs` | 1000-round-trip ROUTER/DEALER stress test (GATE) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-ipc/tests/stress_test.rs` | `test_1000_roundtrips` | All 1000 Ping→Pong round trips complete with matching seq values, zero loss, zero reordering, within timeout | `RouterTransport` exists with bind/send/recv; zeromq 0.6 ROUTER/DEALER works on loopback | 1000 `WorkerMessage::Ping { seq: 1..=1000 }` sent to simulated DEALER | 1000 `WorkerEvent::Pong { seq }` received with seq matching 1..=1000 in order | `cargo test -p anvilml-ipc --test stress_test --release` exits 0 |

## CI Impact

No CI changes required. The new test file is automatically picked up by `cargo test --workspace --features mock-hardware` (the standard CI test command per `ENVIRONMENT.md §6 Step 6`). The test uses only the `anvilml-ipc` crate's existing dependencies — no new crate, feature flag, or build configuration is needed. The `--release` flag in the acceptance command is the stress test's own invocation; CI runs the debug build via the standard workspace test command, which also passes.

## Platform Considerations

None identified. The test uses only loopback TCP (`127.0.0.1`) and the `zeromq` crate's cross-platform transport layer. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ZeroMQ ROUTER socket may not route messages to the simulated DEALER within the timeout if the DEALER's connection registration is delayed relative to the first send. | Low | Medium | The existing `roundtrip_tests.rs` already uses a `tokio::time::sleep(50ms)` between DEALER connect and first send. The stress test will do the same: sleep 100ms after connecting the DEALER before starting the send loop, giving ZeroMQ time to register the new connection. |
| The 1000-iteration send loop may take longer than expected under release mode, causing the overall test to exceed a CI timeout. | Low | Low | Each round-trip on loopback is sub-millisecond; 1000 iterations should complete in well under 1 second. The 5-second per-message timeout is generous. If needed, the per-message timeout can be reduced to 1 second without risk. |
| `WorkerEvent::Pong` deserialization may fail if msgpack serialisation order differs between send and receive. | Very Low | High | `rmp_serde::to_vec_named` and `from_slice` use serde's tag-based deserialization (`#[serde(tag = "_type")]`), which is order-independent. The existing roundtrip tests in `roundtrip_tests.rs` already confirm this works for all event variants. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --test stress_test --release` exits 0, printing 1 test passed, with all 1000 round trips completing within the test's timeout and zero assertion failures.
