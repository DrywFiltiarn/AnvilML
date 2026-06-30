# Implementation Report: P7-B2

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P7-B2                                       |
| Phase         | 7 — IPC Foundations                         |
| Description   | anvilml-ipc: RouterTransport send()/recv() split-lock methods |
| Implemented   | 2026-06-30T23:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented `pub async fn send()` and `pub async fn recv()` on `RouterTransport` in `crates/anvilml-ipc/src/transport.rs`, each locking only its own half (`sender` or `receiver`), completing the split-lock design that structurally prevents the v3 shutdown deadlock. The `send()` method serializes a `WorkerMessage` via `rmp_serde::to_vec_named`, builds a 3-frame multipart ROUTER message (worker_id identity + empty delimiter + msgpack payload), and sends it over the locked send half. The `recv()` method receives a 3-frame multipart message, validates the frame count, extracts the worker identity and payload, and deserializes it via `rmp_serde::from_slice` into a `WorkerEvent`. Five new integration tests were added covering roundtrip, concurrency deadlock regression, seq preservation, complex Execute message, and malformed frame rejection.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | bytes     | 1.12             | Cargo.toml     |
| crate  | tracing   | 0.1              | Cargo.toml     |
| crate  | rmp-serde | 1.3.1            | Cargo.toml (moved from dev-dependencies) |

**bytes 1.12:** Added as a new dependency to `anvilml-ipc` — required for `Bytes::from()` to construct `ZmqMessage` frames. The zeromq crate uses `bytes::Bytes` internally for frame data.

**tracing 0.1:** Added as a new dependency to `anvilml-ipc` — required for `#[tracing::instrument]` on the async `send()` and `recv()` methods, and for `tracing::debug!()` log calls inside both methods.

**rmp-serde 1.3.1:** Moved from `[dev-dependencies]` to `[dependencies]` because `send()` and `recv()` use `rmp_serde::to_vec_named` and `rmp_serde::from_slice` in production code, not just tests.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7; add `bytes = "1.12"`, `tracing = "0.1"` to `[dependencies]`; move `rmp-serde = "1.3.1"` from `[dev-dependencies]` to `[dependencies]` |
| Modify | `crates/anvilml-ipc/src/transport.rs` | Add `send()` and `recv()` methods; remove `#[allow(dead_code)]` suppressions; add `ZmqMessage`, `Bytes`, `WorkerMessage`, `WorkerEvent` imports; add `#[tracing::instrument]` to both methods |
| Modify | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Add 5 new integration tests: `test_send_recv_roundtrip`, `test_concurrent_send_recv_does_not_block`, `test_send_ping_then_recv_pong`, `test_send_execute_message_roundtrip`, `test_recv_malformed_frames_returns_error`; add `make_dealer()` helper function; add imports for `IpcError`, `Bytes`, `Arc`, `timeout`, `PeerIdentity`, `SocketOptions`, `DealerSocket`, `ZmqMessage` |
| Modify | `docs/TESTS.md` | Add 5 new test catalogue entries for the new integration tests |

## Commit Log

```
 .forge/reports/P7-B2_plan.md                | 160 ++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   4 +-
 crates/anvilml-ipc/Cargo.toml               |   6 +-
 crates/anvilml-ipc/src/transport.rs         | 123 +++++++++--
 crates/anvilml-ipc/tests/roundtrip_tests.rs | 323 ++++++++++++++++++++++++++++
 docs/TESTS.md                               |  60 ++++++
 8 files changed, 669 insertions(+), 26 deletions(-)
```

## Test Results

```
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

  - test_ping_roundtrip ... ok
  - test_shutdown_roundtrip ... ok
  - test_execute_roundtrip ... ok
  - test_cancel_job_roundtrip ... ok
  - test_memory_query_roundtrip ... ok
  - test_ready_roundtrip ... ok
  - test_pong_roundtrip ... ok
  - test_dying_roundtrip ... ok
  - test_memory_report_roundtrip ... ok
  - test_progress_roundtrip ... ok
  - test_image_ready_roundtrip ... ok
  - test_completed_roundtrip ... ok
  - test_failed_roundtrip ... ok
  - test_cancelled_roundtrip ... ok
  - test_publish_zero_subscribers ... ok
  - test_publish_one_subscriber_delivers ... ok
  - test_publish_multiple_subscribers_independent_copies ... ok
  - test_subscribe_returns_valid_receiver ... ok
  - test_bind_returns_nonzero_port ... ok
  - test_two_binds_get_different_ports ... ok
  - test_bind_port_is_listening ... ok
  - test_send_recv_roundtrip ... ok
  - test_concurrent_send_recv_does_not_block ... ok
  - test_send_ping_then_recv_pong ... ok
  - test_send_execute_message_roundtrip ... ok
  - test_recv_malformed_frames_returns_error ... ok
```

All 26 tests pass, including 5 new integration tests and 21 pre-existing tests.

## Format Gate

```
cargo fmt --all -- --check
```

Exit 0 — no formatting drift detected.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.84s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.79s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.22s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.18s
```

All four platform cross-checks exit 0.

## Project Gates

Gate 1 (Config Surface Sync) — not applicable: this task does not modify any `ServerConfig` fields or nested config structs.

Gate 2 (OpenAPI Drift) — not applicable: this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

Gate 3 (Node Parity) — not applicable: this task does not modify any node types in `worker/nodes/` or `crates/anvilml-core/src/node_registry.rs`.

Gate 4 (Mock/Real Parity Markers) — not applicable: this task does not add or modify a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

```
+    pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), IpcError> {
+    pub async fn recv(&self) -> Result<(String, WorkerEvent), IpcError> {
```

Two new `pub` items introduced:
- `RouterTransport::send` — `pub async fn send(&self, worker_id: &str, msg: &WorkerMessage) -> Result<(), IpcError>` (module: `anvilml_ipc::transport`)
- `RouterTransport::recv` — `pub async fn recv(&self) -> Result<(String, WorkerEvent), IpcError>` (module: `anvilml_ipc::transport`)

Both match the plan's Public API Surface table exactly. No other `pub` items were added or removed.

## Deviations from Plan

- **Dependency additions not in plan:** Added `bytes = "1.12"` and `tracing = "0.1"` as new dependencies. The plan mentioned `rmp_serde` staying in dev-dependencies, but it was moved to `[dependencies]` because `send()` and `recv()` use it in production code paths. The plan also did not explicitly list `bytes` and `tracing` as new dependencies, but they are required for the implementation: `bytes::Bytes` is needed to construct `ZmqMessage` frames, and `tracing` is needed for `#[tracing::instrument]` and `tracing::debug!()` calls.

- **ZmqMessage construction:** The plan described using `ZmqMessage::default()` with `push_front`/`push_back` for frame ordering. The actual zeromq 0.6.0 crate does not implement `Default` for `ZmqMessage`. Instead, the implementation uses `ZmqMessage::from(worker_id)` to create a 1-frame message with the worker identity, then `push_back` for the delimiter and payload frames. This produces the same 3-frame layout: `[worker_id, "", payload]`.

- **DEALER identity requirement:** The plan described spawning a DEALER socket to communicate with the ROUTER in tests. The zeromq ROUTER socket routes messages based on the peer identity registered during connection. Without explicitly setting the DEALER's peer identity to match the worker_id used in `send()`, the ROUTER returns "Destination client not found by identity". The implementation adds a `make_dealer()` helper that sets the peer identity via `SocketOptions::peer_identity(PeerIdentity::try_from(Bytes::from(identity)))`.

- **Race condition with 50ms delay:** Tests include a `tokio::time::sleep(50ms)` delay between spawning the DEALER and sending/receiving to ensure the DEALER has connected and registered its identity with the ROUTER. This is a test-only synchronization point.

## Blockers

None.
