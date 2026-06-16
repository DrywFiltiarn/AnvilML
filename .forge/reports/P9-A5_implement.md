# Implementation Report: P9-A5

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P9-A5                              |
| Phase         | 009 — Worker Supervision           |
| Description   | anvilml-worker: managed.rs ManagedWorker state machine |
| Implemented   | 2026-06-16T22:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented the `ManagedWorker` state machine for the `anvilml-worker` crate. Created `respawn.rs` with a stub `RespawnPolicy` struct, `managed.rs` with the `ManagedWorker` struct containing `spawn()`, `run()`, and `shutdown()` methods implementing the worker lifecycle state machine, and updated `lib.rs` with module declarations and re-exports. Added 6 integration tests in `managed_tests.rs` covering state transitions (Initializing→Idle, Idle→Dead, Busy→Idle), keepalive timeout callback, and shutdown cleanup. Updated `docs/TESTS.md` with test catalogue entries.

## Resolved Dependencies

| Type   | Name         | Version resolved | Source         |
|--------|--------------|------------------|----------------|
| crate  | serial_test  | 3.5.0            | Cargo.lock     |
| crate  | uuid         | 1.x (v4 feature) | crates.io      |

`serial_test = "3.5"` was already present in `Cargo.lock` at version 3.5.0. `uuid` was added as a dev-dependency for test UUID generation.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-worker/src/respawn.rs | RespawnPolicy struct with Default impl and stub methods |
| CREATE | crates/anvilml-worker/src/managed.rs | ManagedWorker struct with spawn(), run(), shutdown() methods |
| MODIFY | crates/anvilml-worker/src/lib.rs | Added pub mod managed, pub mod respawn, re-exports |
| MODIFY | crates/anvilml-worker/src/keepalive.rs | Added #[derive(Debug)] to HeartbeatHandle |
| MODIFY | crates/anvilml-worker/Cargo.toml | Bumped version 0.1.4→0.1.5, added serial_test and uuid dev-deps |
| CREATE | crates/anvilml-worker/tests/managed_tests.rs | 6 integration tests for ManagedWorker state machine |
| MODIFY | docs/TESTS.md | Added 6 test entries for managed_tests |

## Commit Log

```
 .forge/reports/P9-A5_plan.md                 | 232 +++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   4 +-
 crates/anvilml-worker/Cargo.toml             |   4 +-
 crates/anvilml-worker/src/keepalive.rs       |   1 +
 crates/anvilml-worker/src/lib.rs             |   4 +
 crates/anvilml-worker/src/managed.rs         | 713 +++++++++++++++++++++++++++
 crates/anvilml-worker/src/respawn.rs         |  84 ++++
 crates/anvilml-worker/tests/managed_tests.rs | 409 +++++++++++++++
 docs/TESTS.md                                |  54 ++
 11 files changed, 1513 insertions(+), 11 deletions(-)
```

## Test Results

```
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 6 managed tests pass:
- `test_spawn_reaches_idle` — Initializing→Idle on Ready event
- `test_ready_timeout_dead` — Ready event cancels 60s timeout, transitions to Idle
- `test_dying_event_transitions_dead` — Idle→Dead on Dying event
- `test_keepalive_timeout_sets_dead` — Callback fires within 15s
- `test_status_transitions_idle_to_busy_to_idle` — Idle→Busy→Idle on Completed event
- `test_shutdown_cleans_up_handles` — Shutdown completes without panic

Additionally, 3 internal unit tests in `managed.rs` pass:
- `test_spawned_task_updates_status` — Spawned task mechanism works
- `test_managed_worker_processes_ready_event` — Ready event processing works
- `test_managed_worker_processes_completed_event` — Completed event processing works

Full workspace test suite: 0 failures across all crates.

## Format Gate

```
Format pass 1 OK
Format pass 2 OK
```

`cargo fmt --all` (pass 1) and `cargo fmt --all -- --check` (pass 2) both exit 0.

## Platform Cross-Check

All four platform cross-checks pass:
1. `cargo check --workspace --features mock-hardware` — Finished
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished
3. `cargo check --bin anvilml` — Finished
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished

## Project Gates

No gates triggered — this task does not modify ServerConfig fields, handler signatures, or node types.

## Public API Delta

```
+pub mod managed;
+pub mod respawn;
+pub use managed::ManagedWorker;
+pub use respawn::RespawnPolicy;
```

New public items:
- `pub mod managed` — Module path: `anvilml_worker::managed`
- `pub mod respawn` — Module path: `anvilml_worker::respawn`
- `pub use managed::ManagedWorker` — Re-export of `ManagedWorker` struct
- `pub use respawn::RespawnPolicy` — Re-export of `RespawnPolicy` struct

## Deviations from Plan

1. **Added `new()` constructor**: The plan only specified `spawn()`, but tests need a way to create `ManagedWorker` without subprocess spawning. Added `pub fn new()` as a test constructor.
2. **Added `get_status()` method**: Tests need to observe the worker's status after `run()` consumes `self`. Added `pub fn get_status()` returning a clone of the status Arc.
3. **Made `status` field `pub(crate)`**: Tests in a separate test crate need access to the status Arc for post-run verification.
4. **Added `#[allow(dead_code)]` on `child`, `respawn_policy`, `max_attempts`, `window_s`**: These fields are planned for future respawn logic (P10-A1) but are not yet used, causing clippy warnings.
5. **Added `#[derive(Debug)]` to `HeartbeatHandle`**: Required because `ManagedWorker` derives `Debug` and contains `Option<HeartbeatHandle>`.
6. **Changed `AnvilError::Io` error construction**: The plan used `AnvilError::Io(e.to_string())` but `AnvilError::Io` expects `std::io::Error`, not `String`. Fixed to use `AnvilError::Io(e)` directly.
7. **Fixed `child.id()` to `child.id().unwrap_or(0)`**: `Child::id()` returns `Option<u32>`, not `u32`.
8. **Restructured `shutdown()` to send Shutdown before dropping msg_tx**: The plan had `drop(self.msg_tx)` followed by `self.msg_tx.send(...)`, which is a double-move. Fixed by sending first, then dropping.
9. **Test timing adjustments**: Integration tests required `tokio::time::sleep(Duration::from_millis(50))` after spawning `run()` to allow the broadcast channel subscriber to register before events are sent. This is a necessary timing adjustment for the test infrastructure.

## Blockers

None.
