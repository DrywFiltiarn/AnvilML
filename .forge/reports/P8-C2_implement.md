# Implementation Report: P8-C2

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P8-C2                                       |
| Phase         | 008 — IPC Stress Gate & Worker Pool         |
| Description   | anvilml-worker: keepalive.rs ping/pong heartbeat watchdog |
| Implemented   | 2026-07-01T12:30:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Created `crates/anvilml-worker/src/keepalive.rs` — a `KeepaliveWatchdog` type that runs a tokio task sending `WorkerMessage::Ping{seq}` at a configurable interval and tracking incoming `WorkerEvent::Pong{seq}` responses through a dedicated channel. If no matching Pong arrives within the configured timeout after a Ping, the watchdog signals worker death via a `tokio::sync::oneshot::Sender<()>` channel. The interval and timeout are injected as constructor parameters so tests can use millisecond-scale durations. Added a `Transport` trait abstraction with a `MockTransport` for testability. Added `pub mod keepalive;` and `pub use keepalive::KeepaliveWatchdog;` in `lib.rs`. Created 4 integration tests covering pong-received, missing-pong, repeated-pong, and transport-failure scenarios.

## Resolved Dependencies

| Type   | Name  | Version resolved | Source        |
|--------|-------|------------------|---------------|
| crate  | tokio | 1.52.3           | rust-docs MCP |

No new external crates introduced. The only manifest change is adding the `"time"` feature to the existing tokio dependency in `Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/keepalive.rs` | Keepalive watchdog with Transport trait, MockTransport, and KeepaliveWatchdog<T> |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Added `pub mod keepalive;` and `pub use keepalive::KeepaliveWatchdog;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Added `"time"` feature to tokio; bumped patch version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-worker/tests/keepalive_tests.rs` | 4 integration tests for the keepalive watchdog |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new keepalive tests |

## Commit Log

```
 .forge/reports/P8-C2_plan.md                   | 562 +++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   2 +-
 crates/anvilml-worker/Cargo.toml               |   4 +-
 crates/anvilml-worker/src/keepalive.rs         | 312 ++++++++++++++
 crates/anvilml-worker/src/lib.rs               |   3 +
 crates/anvilml-worker/tests/keepalive_tests.rs | 211 ++++++++++
 docs/TESTS.md                                  |  48 +++
 9 files changed, 1149 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running tests/keepalive_tests.rs (target/debug/deps/keepalive_tests-...)

running 4 tests
test test_missing_pong_triggers_dead_signal ... ok
test test_pong_within_timeout_keeps_alive ... ok
test test_repeated_successful_pings_no_false_trigger ... ok
test test_transport_send_failure_triggers_dead_signal ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 0 failures across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:    cargo check --workspace --features mock-hardware → 0
# 2. Mock-hardware Windows:  cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu → 0
# 3. Real-hardware Linux:    cargo check --bin anvilml → 0
# 4. Real-hardware Windows:  cargo check --bin anvilml --target x86_64-pc-windows-gnu → 0
```

All four cross-checks passed.

## Project Gates

Gate 1 (config_reference): passed. Gate 2 (openapi-drift): not triggered — no handler/signature changes. Gate 3 (node-parity): not triggered — no node type changes. Gate 4 (mock/real parity markers): not triggered — no node execute() or arch module function changes.

## Public API Delta

```
+pub mod keepalive;
+pub use keepalive::KeepaliveWatchdog;
```

New public items in the `keepalive` module:
- `pub trait Transport` — transport abstraction for sending Ping messages
- `pub struct MockTransport` — mock transport for tests
- `pub struct KeepaliveWatchdog<T: Transport>` — the watchdog type (generic over transport)
- `pub fn KeepaliveWatchdog::new(...)` — constructor (generic impl)
- `pub async fn KeepaliveWatchdog::run(...)` — the keepalive loop

## Deviations from Plan

1. **Transport trait abstraction**: The plan specified `Arc<RouterTransport>` as the transport type. The actual implementation uses a `Transport` trait with a generic `KeepaliveWatchdog<T: Transport>` and a `RouterTransportAdapter` wrapper. This was necessary because `RouterTransport::send()` uses ZeroMQ directly and cannot be mocked for unit tests. The `Transport` trait provides a `send()` method returning `Result<(), IpcError>`, matching the real transport's behavior. `MockTransport` implements this trait for test use.

2. **`pub mod keepalive`**: The plan specified `mod keepalive;` (private). Changed to `pub mod keepalive;` so the test crate can access `MockTransport` and `Transport` via `anvilml_worker::keepalive::MockTransport`.

3. **Test timing approach**: Tests 1 and 3 use dedicated tokio tasks for pong-sending to ensure reliable timing. The initial approach of sending pongs with sleep-based delays was fragile due to tokio scheduling non-determinism.

## Blockers

None.
