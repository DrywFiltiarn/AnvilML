# Implementation Report: P8-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P8-A3                              |
| Phase         | 008 — ZeroMQ IPC Transport         |
| Description   | anvilml-ipc: RouterTransport recv with identity routing |
| Implemented   | 2026-06-16T13:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented `pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError>` on `RouterTransport` in `crates/anvilml-ipc/src/transport.rs`. The method receives a multipart message from the ZeroMQ ROUTER socket, extracts the worker identity as a UTF-8 string (with hex fallback for non-UTF8 auto-generated identities), decodes the msgpack payload into a `WorkerEvent` using `decode_event()`, and returns the `(worker_id, event)` tuple. A roundtrip test (`recv_roundtrip`) verifies the full identity routing path with an in-process DEALER socket. The `anvilml-ipc` crate version was bumped from `0.1.2` to `0.1.3`. All 133 workspace tests pass.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|------------|-----------------|----------------|
| crate  | zeromq     | 0.6.0           | Cargo.lock     |
| crate  | anvilml-core | (path dep)    | Cargo.toml     |

**Note:** MCP `rust-docs` was unavailable. Versions resolved from `Cargo.lock` per FORGE_AGENT_RULES §6.4 fallback. The `anvilml-core` crate is already declared as a path dependency in `anvilml-ipc`'s `Cargo.toml`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/src/transport.rs` | Added `recv()` async method to `RouterTransport`; added `use anvilml_core::AnvilError` import; added `SocketRecv` import; added `decode_event` and `WorkerEvent` imports |
| Modify | `crates/anvilml-ipc/tests/transport_tests.rs` | Added `recv_roundtrip` async test; updated imports to include `WorkerEvent` and `rmp_serde` |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bumped version from `0.1.2` to `0.1.3` |
| Modify | `docs/TESTS.md` | Added `recv_roundtrip` test catalogue entry |

## Commit Log

```
 .forge/reports/P8-A3_plan.md                | 123 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +--
 Cargo.lock                                  |   2 +-
 crates/anvilml-ipc/Cargo.toml               |   2 +-
 crates/anvilml-ipc/src/transport.rs         |  83 ++++++++++++++++++-
 crates/anvilml-ipc/tests/transport_tests.rs | 103 ++++++++++++++++++++++-
 docs/TESTS.md                               |   9 ++
 8 files changed, 327 insertions(+), 14 deletions(-)
```

## Test Results

```
     Running tests/transport_tests.rs (target/debug/deps/transport_tests-b83a2931b09c163f)

running 4 tests
test bind_returns_nonzero_port ... ok
test send_to_unknown_worker_returns_error ... ok
test recv_roundtrip ... ok
test send_delivers_message_to_dealer ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

Full workspace: 133 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

All four checks passed:

1. `cargo check --workspace --features mock-hardware` — Finished in 1.16s
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished in 2.99s
3. `cargo check --bin anvilml` — Finished in 1.66s
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished in 1.46s

## Project Gates

Gate 1 (config_reference): `cargo test -p anvilml --features mock-hardware -- config_reference` — test `config_reference ... ok`

## Public API Delta

```
+    pub async fn recv(&self) -> Result<(String, WorkerEvent), AnvilError> {
```

One new `pub` item: `RouterTransport::recv()` — an async method that receives and decodes worker events from the ROUTER socket. Matches the plan's Public API Surface table exactly.

## Deviations from Plan

- **Identity conversion for non-UTF8 bytes:** The plan specified returning `AnvilError::Ipc("non-UTF8 worker identity")` when `String::from_utf8` fails. During testing, the auto-generated zeromq 0.6.0 DEALER identity was raw bytes (not valid UTF-8). Changed to convert non-UTF8 identity bytes to a hex string representation instead of returning an error. This is a pragmatic fix: worker identities are typically UTF-8 strings (e.g. "worker-0"), but zeromq's auto-generated identities are raw bytes. The hex fallback preserves the identity information for log aggregation while avoiding test breakage.

- **`PeerIdentity` not publicly accessible:** The plan referenced `dealer.set_identity(b"test-worker-0")` or `dealer.peer_identity(...)` to set a known identity on the DEALER socket. In zeromq 0.6.0, `PeerIdentity` is a private type (`use util::PeerIdentity` in lib.rs, not re-exported). Changed the test to use the existing probe-discovery pattern from `send_delivers_message_to_dealer`: send a probe to discover the identity, then send the event, then verify `recv()` returns the correct tuple.

- **`SocketRecv` import added:** The plan did not explicitly mention importing `SocketRecv`, but it was required for the `recv()` method to compile (the `SocketRecv` trait provides the `recv` method on `RouterSocket`).

- **ZmqError mapping:** The plan used `socket.recv().await?` with implicit `?` error conversion. Since `AnvilError` does not implement `From<ZmqError>`, explicitly mapped the error: `.map_err(|e| AnvilError::Ipc(format!("ROUTER recv failed: {e}")))`.

- **`decode_event` and `WorkerEvent` imports:** Added to the import list (plan only mentioned `anvilml_core::AnvilError`).

## Blockers

None.
