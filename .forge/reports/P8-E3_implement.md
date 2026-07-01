# Implementation Report: P8-E3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P8-E3                           |
| Phase         | 008 — IPC Stress Gate & Worker Pool |
| Description   | anvilml-worker: ManagedWorker::run() owns full lifecycle task |
| Implemented   | 2026-07-01T15:30:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented `ManagedWorker::run()` — the full lifecycle task that owns a single Python worker subprocess's lifetime. The implementation includes: replacing `WorkerStatus::Spawning` with `Initializing` and adding `Respawning` variant; adding `Demux::registered()` query method; implementing `ManagedWorker` struct with `new()` constructor and `run()` lifecycle method; updating `lib.rs` re-export; and adding 5 new tests (14 total) in `managed_tests.rs` verifying all exit paths against a mock IPC backend.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | zeromq    | 0.6.0            | rust-docs MCP  |
| crate  | serial_test | 3              | crates.io      |
| crate  | rmp-serde | 1                | crates.io      |
| crate  | bytes     | 1                | crates.io      |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/worker.rs` | Replace `Spawning` with `Initializing`, add `Respawning` variant to `WorkerStatus` enum |
| Modify | `crates/anvilml-core/tests/worker_tests.rs` | Update serde roundtrip test for 6 variants (was 5) |
| Modify | `crates/anvilml-worker/src/demux.rs` | Add `registered()` query method |
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `ManagedWorker` struct, `new()`, `run()`, and `handle_event()` |
| Modify | `crates/anvilml-worker/src/lib.rs` | Add `pub use managed::ManagedWorker;` re-export |
| Modify | `crates/anvilml-worker/tests/managed_tests.rs` | Update existing tests for `Initializing`/`Respawning`, add 5 new `ManagedWorker` tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump version 0.1.7→0.1.8, add dev-dependencies |
| Modify | `crates/anvilml-ipc/src/transport.rs` | Add `send_raw()` method for test use |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump version 0.1.8→0.1.9 |
| Modify | `docs/TESTS.md` | Update entries for `Initializing`/`Respawning`, add 5 new test entries |

## Commit Log

```
 crates/anvilml-core/Cargo.toml                         |  2 +-
 crates/anvilml-core/src/types/worker.rs                | 15 +++++++--
 crates/anvilml-core/tests/worker_tests.rs              |  8 ++---
 crates/anvilml-ipc/Cargo.toml                          |  2 +-
 crates/anvilml-ipc/src/transport.rs                    | 26 ++++++++++++++
 crates/anvilml-worker/Cargo.toml                       |  6 +++-
 crates/anvilml-worker/src/demux.rs                     | 17 ++++++++++
 crates/anvilml-worker/src/lib.rs                       |  2 +-
 crates/anvilml-worker/src/managed.rs                   | 53 +++++++++++++++++++++++++++++
 crates/anvilml-worker/tests/managed_tests.rs           | 62 ++++++++++++++++++++++++++++++++-
 docs/TESTS.md                                          | 40 +++++++++++++++-----
 11 files changed, 215 insertions(+), 28 deletions(-)
```

## Test Results

```
running 14 tests
test test_clone_shares_status ... ok
test test_clone_independent_worker_id ... ok
test test_concurrent_status_and_set_status_no_deadlock ... ok
test test_request_shutdown_is_idempotent ... ok
test test_request_shutdown_sends_signal ... ok
test test_set_status_callable_repeatedly ... ok
test test_set_status_changes_value ... ok
test test_set_status_visible_across_clone ... ok
test test_status_returns_current_value ... ok
test test_deregister_called_on_graceful_exit ... ok
test test_shutdown_rx_triggers_graceful_exit ... ok
test test_run_completes_on_ready_event ... ok
test test_deregister_called_on_crash ... ok
test test_deregister_called_on_initializing_timeout ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 262 tests passed, 0 failed.

## Format Gate

```
(No output — exit 0, formatting clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux: OK
# 2. Mock-hardware Windows: OK
# 3. Real-hardware Linux: OK
# 4. Real-hardware Windows: OK
All four checks exited 0.
```

## Project Gates

Gate 1 (config_reference): `cargo test -p anvilml --features mock-hardware -- config_reference` — OK (1 passed)

## Public API Delta

```
+    pub async fn send_raw(&self, worker_id: &str, payload: &[u8]) -> Result<(), IpcError> {
+    pub fn registered(&self, worker_id: &str) -> bool {
+pub use managed::{ManagedWorker, WorkerHandle};
+pub struct ManagedWorker {
+    pub fn new(
+    pub async fn run(mut self, mut shutdown_rx: oneshot::Receiver<()>) {
```

New items:
- `pub struct ManagedWorker` — `crates/anvilml-worker/src/managed.rs`
- `pub fn ManagedWorker::new(...)` — constructor
- `pub async fn ManagedWorker::run(...)` — lifecycle method
- `pub fn Demux::registered(...)` — query method
- `pub async fn RouterTransport::send_raw(...)` — test helper

## Deviations from Plan

1. **Registration timing**: The plan states `run()` calls `demux.register()` on entry. However, `Demux::register()` requires a `Sender<WorkerEvent>` argument which `ManagedWorker` doesn't have. Resolution: the pool registers the worker before spawning `run()`. `run()` only calls `deregister()` on exit. This is consistent with the existing `Demux` API.

2. **`ManagedWorker::new()` signature**: The plan's constructor didn't include a `status` parameter. Resolution: added `Arc<RwLock<WorkerStatus>>` parameter so `run()` can set status to `Initializing` and track lifecycle state.

3. **Test event delivery**: The plan's mock IPC backend setup described using `transport.send()` to send events. However, `RouterTransport::send()` only accepts `WorkerMessage` (Rust→Python direction). Resolution: added `send_raw()` method for raw byte sending, and the tests use the DEALER socket to send events in the correct direction (worker→ROUTER).

4. **`handle_event` return value**: The plan's `handle_event` was described as handling events and breaking on `Dying`. Resolution: `handle_event` returns `bool` (true = break) and the main loop checks this to decide whether to break.

## Blockers

None.
