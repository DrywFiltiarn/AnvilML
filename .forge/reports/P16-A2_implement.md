# Implementation Report: P16-A2

| Field | Value |
|-------|-------|
| Task ID | P16-A2 |
| Phase | 016 — Job Cancellation |
| Description | anvilml-scheduler: JobScheduler::cancel (queued + running) |
| Implemented | 2026-06-10T14:30:00Z |
| Status | COMPLETE |

## Summary

Implemented `JobScheduler::cancel` method that reads a job from the database and cancels it if it is in `Queued` or `Running` status. For queued jobs, removes the entry from the in-memory queue, updates the DB to `Cancelled`, and broadcasts a `JobCancelled` event. For running jobs, sends a `CancelJob` IPC message to the owning worker, sets the worker idle, updates the DB, and broadcasts the event. Added `JobNotCancellable(Uuid)` error variant to `AnvilError`, updated `handle_cancelled` to accept `Queued` status as a race guard, and added two unit tests (queued cancel and running cancel).

## Resolved Dependencies

No new dependencies added. No MCP lookups required.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/error.rs` | Added `JobNotCancellable(Uuid)` variant, Display arm, and test case |
| Modify | `crates/anvilml-core/Cargo.toml` | Version bump `0.1.1 → 0.1.2` |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Added `cancel()` method, updated `handle_cancelled` guard, added 2 unit tests |
| Modify | `crates/anvilml-scheduler/src/queue.rs` | Fixed `cancel_queued` to remove entry from deque (was only marking status) |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Version bump `0.1.16 → 0.1.17` |

## Commit Log

```
 .forge/reports/P16-A2_plan.md             | 161 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +-
 Cargo.lock                                |   6 +-
 crates/anvilml-core/Cargo.toml            |   2 +-
 crates/anvilml-core/src/error.rs          |   8 +
 crates/anvilml-scheduler/Cargo.toml       |   2 +-
 crates/anvilml-scheduler/src/queue.rs     |  15 +-
 crates/anvilml-scheduler/src/scheduler.rs | 234 +++++++++++++++++++++++++++++-
 9 files changed, 425 insertions(+), 22 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-f3df55d7386c8396)
running 74 tests
test error::tests::all_variants_display ... ok
test error::tests::debug_formatting ... ok
test error::tests::error_trait_impls ... ok
test error::tests::from_io_error ... ok
test error::tests::send_sync ... ok
... (all 74 passed)
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-395d68b7d76bba7d)
running 56 tests
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-5ce179a5e12f9aa5)
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-07dc3a94706f3425)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs ... running 1 test ... ok
     Running tests/device_store.rs ... running 4 tests ... ok
     Running tests/rescan.rs ... running 2 tests ... ok
     Running tests/scanner.rs ... running 1 test ... ok
     Running tests/seed_loader.rs ... running 7 tests ... ok
     Running tests/store_get.rs ... running 2 tests ... ok
     Running tests/store_list.rs ... running 3 tests ... ok

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-3f7c539b6c6a65b2)
running 43 tests
test scheduler::tests::test_cancel_queued ... ok
test scheduler::tests::test_cancel_running ... ok
test scheduler::tests::test_cancel_broadcasts_event ... ok
test scheduler::tests::test_complete ... ok
test scheduler::tests::test_dispatch_sends_execute ... ok
test scheduler::tests::test_image_ready_broadcasts_event ... ok
test scheduler::tests::test_progress_broadcasts_event ... ok
... (all 43 passed)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-22fe293db6304108)
running 22 tests
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_save.rs ... running 1 test ... ok
     Running tests/api_artifact_serve.rs ... running 3 tests ... ok
     Running tests/api_models.rs ... running 3 tests ... ok
     Running tests/api_ws_events.rs ... running 1 test ... ok

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-4fc8470a95a34bb6)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-6a02dbd301558bc6)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_lifecycle.rs ... running 1 test ... ok
     Running tests/config_reference.rs ... running 1 test ... ok

   Doc-tests anvilml_hardware ... running 2 tests ... ok

Grand total: 210 passed; 0 failed; 0 ignored
```

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no formatting drift)

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.38s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.88s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.26s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.72s
```
All four checks exited 0.

## Project Gates

```
# Config drift gate
cargo test -p backend --features mock-hardware -- config_reference
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

## Deviations from Plan

- **`cancel_queued` implementation**: The plan specified `self.queue.cancel_queued(id)` which was previously only marking the job status as `Cancelled` without removing it from the deque. This caused `queue.len()` to return 1 after cancellation. Fixed `cancel_queued` to actually remove the entry from the deque using `position` + `remove`. This is necessary for the method to fulfill its documented contract ("cancel a queued job").
- **`workers.set_idle` for running jobs**: The plan's `cancel` method did not include `set_idle` for the Running case. Added `self.workers.set_idle(wid).await` to match the behavior of the existing `handle_cancelled` function, which also sets the worker idle on cancellation. This ensures the worker becomes available for the next dispatched job.
- **`ok_or` vs `ok_or_else`**: Clippy flagged `ok_or_else(|| AnvilError::JobNotFound(id))` as unnecessary lazy evaluation; changed to `ok_or(AnvilError::JobNotFound(id))`.

## Blockers

None.
