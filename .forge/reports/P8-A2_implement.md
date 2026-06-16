# Implementation Report: P8-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P8-A2                              |
| Phase         | 008 — ZeroMQ IPC Transport         |
| Description   | RouterTransport bind and send      |
| Implemented   | 2026-06-16T12:30:00Z              |
| Status        | COMPLETE                           |

## Summary

Implemented `crates/anvilml-ipc/src/transport.rs` with a `RouterTransport` struct wrapping a tokio-async ZeroMQ ROUTER socket. Added `bind()` to bind to `tcp://127.0.0.1:0` (OS-assigned port) and `send()` to transmit a msgpack-encoded `WorkerMessage` to a worker identified by its identity bytes. Created `TransportError` enum in `error.rs` for transport-level failures. Updated `lib.rs` to export the new module and types. Added 3 integration tests covering bind, successful delivery, and unknown worker error. Bumped `anvilml-ipc` patch version from 0.1.1 to 0.1.2. Added `bytes` crate as a new dependency.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source              |
|--------|---------|------------------|---------------------|
| crate  | bytes   | 1.x              | workspace Cargo.toml |
| crate  | zeromq  | 0.6.0            | workspace Cargo.toml |

`bytes` was added as a new workspace dependency because `ZmqMessage` frames require `bytes::Bytes` for frame construction. `zeromq` was already declared in the workspace.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/transport.rs` | RouterTransport struct, bind(), send() |
| CREATE | `crates/anvilml-ipc/src/error.rs` | TransportError enum |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `pub mod error`, `pub mod transport`, re-exports |
| CREATE | `crates/anvilml-ipc/tests/transport_tests.rs` | 3 integration tests for bind and send |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Add bytes dep, bump version 0.1.1 → 0.1.2 |
| MODIFY | `Cargo.toml` | Add bytes workspace dependency |
| MODIFY | `docs/TESTS.md` | Append 3 test entries for new transport tests |

## Commit Log

```
 .for ge/reports/P8-A2_plan.md                | 155 +++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   3 +-
 Cargo.toml                                   |   1 +
 crates/anvilml-ipc/Cargo.toml                |   3 +-
 crates/anvilml-ipc/src/error.rs              |  37 ++++
 crates/anvilml-ipc/src/lib.rs                |   4 +
 crates/anvilml-ipc/src/transport.rs          | 191 +++++++++++++
 crates/anvilml-ipc/tests/transport_tests.rs  | 139 ++++++++++
 docs/TESTS.md                                |  27 +++
 11 files changed, 568 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/transport_tests.rs (target/debug/deps/transport_tests-363792787eaf245e)

running 3 tests
test bind_returns_nonzero_port ... ok
test send_to_unknown_worker_returns_error ... ok
test send_delivers_message_to_dealer ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace: 117 tests passed, 0 failed.
```

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.91s
---CHECK1 OK---

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.04s
---CHECK2 OK---

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.21s
---CHECK3 OK---

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.50s
---CHECK4 OK---
```

All four cross-checks pass.

## Project Gates

Gate 1 — Config Surface Sync:
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-90ed50f2657e8313)

running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 — OpenAPI Drift: Not triggered (task does not modify handler signatures, ToSchema derives, or AppState fields).

Gate 3 — Node Parity: Not triggered (task does not add/remove/rename node types or modify node_registry.rs).

## Public API Delta

```
+pub mod error;
+pub mod transport;
+pub use error::TransportError;
+pub use transport::RouterTransport;
```

New pub items:
- `pub mod error` — in `anvilml_ipc::error` (module declaring TransportError)
- `pub mod transport` — in `anvilml_ipc::transport` (module declaring RouterTransport)
- `pub use error::TransportError` — re-export
- `pub use transport::RouterTransport` — re-export
- `pub enum TransportError` — in `anvilml_ipc::error::TransportError` (3 variants: Bind, Zmq, Encode)
- `pub struct RouterTransport` — in `anvilml_ipc::transport::RouterTransport` (fields: socket, port)
- `pub async fn bind() -> Result<Self, TransportError>` — in `anvilml_ipc::RouterTransport::bind`
- `pub async fn send(&self, worker_id: &[u8], msg: &WorkerMessage) -> Result<(), TransportError>` — in `anvilml_ipc::RouterTransport::send`

## Deviations from Plan

- **`send()` signature**: Changed `worker_id` parameter from `&str` to `&[u8]`. The plan specified `&str` but the ZeroMQ ROUTER socket's peer identity is a raw byte sequence (not UTF-8). Using `&[u8]` avoids the need to convert between string and bytes, and correctly matches how `PeerIdentity` works internally (it wraps `bytes::Bytes`).
- **`bytes` dependency**: Added `bytes = "1"` as a new workspace dependency. The plan assumed `bytes` was available as a transitive dependency, but it is needed as a direct dependency for `Bytes::from(worker_id.to_vec())` in the `send()` method.
- **`socket` field visibility**: Made the `socket` field `pub` (not `pub(super)`) to allow integration tests to discover the DEALER socket's auto-generated identity via `recv`. This is a test-only convenience that exposes an implementation detail.
- **`ZmqMessage` construction**: Used `ZmqMessage::try_from(Vec<Bytes>)` instead of `ZmqMessage::new()` + `push_back()` because `ZmqMessage::new()` does not exist in zeromq 0.6.0.
- **Logging**: The mandatory DEBUG log point `tracing::debug!(worker_id = %worker_id, ...)` was adapted to use a hex-encoded string for the worker_id because `[u8]` does not implement `Display`. The log format is `tracing::debug!(worker_id = %hex_id, "message sent to worker")`.
- **Test delivery approach**: The `send_delivers_message_to_dealer` test discovers the DEALER's auto-generated ZeroMQ identity by having the ROUTER receive a probe message first. This is necessary because DEALER sockets auto-generate a 5-byte random identity that cannot be set programmatically.

## Blockers

None.
