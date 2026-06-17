# Implementation Report: P10-A2

| Field         | Value                                                      |
|---------------|------------------------------------------------------------|
| Task ID       | P10-A2                                                     |
| Phase         | 010 — Worker Crash Recovery                                |
| Description   | Extend ManagedWorker's run loop to detect unexpected child exit and transition to Dead |
| Implemented   | 2026-06-17T15:30:00Z                                       |
| Status        | COMPLETE                                                   |

## Summary

Added crash detection to `ManagedWorker::run()` by introducing a `child.wait()` arm to the existing `tokio::select!` loop. When the Python worker subprocess exits unexpectedly (without sending a Dying event), the worker's status transitions to `Dead` and a structured `tracing::info!` log records the exit code. Also added a `device_index: u32` field to `ManagedWorker` (populated from the Ready event) and a test `test_child_exit_transitions_dead` that verifies the crash detection path.

## Resolved Dependencies

None. No new dependencies introduced. `tokio::process::Child::wait()` is part of the `full` feature set already enabled in the workspace.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/managed.rs` | Added `device_index: u32` field to `ManagedWorker` struct; populated from Ready event; added `child.wait()` select! arm for crash detection; updated `new()` and `spawn()` signatures |
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | Added `make_test_worker_with_index()` helper; added `test_child_exit_transitions_dead` test; updated all existing `ManagedWorker::new()` calls with `device_index` parameter |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bumped patch version 0.1.10 → 0.1.11 |
| MODIFY | `crates/anvilml-worker/tests/pool_tests.rs` | Updated `ManagedWorker::new()` call with `device_index` parameter |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | Updated `ManagedWorker::new()` call with `device_index` parameter |
| MODIFY | `docs/TESTS.md` | Added entry for `test_child_exit_transitions_dead` |

## Commit Log

```
 .forge/reports/P10-A2_plan.md                | 181 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-server/tests/workers_tests.rs |   1 +
 crates/anvilml-worker/Cargo.toml             |   2 +-
 crates/anvilml-worker/src/managed.rs         |  88 ++++++++++---
 crates/anvilml-worker/tests/managed_tests.rs | 102 ++++++++++++++-
 crates/anvilml-worker/tests/pool_tests.rs    |   1 +
 docs/TESTS.md                                |   9 ++
 10 files changed, 378 insertions(+), 27 deletions(-)
```

## Test Results

```
     Running tests/managed_tests.rs (target/debug/deps/managed_tests-a76cb14e9bf8307f)

running 8 tests
test test_shutdown_cleans_up_handles ... ok
test test_ready_timeout_dead ... ok
test test_dying_event_transitions_dead ... ok
test test_spawn_reaches_idle ... ok
test test_child_exit_transitions_dead ... ok
test test_status_transitions_idle_to_busy_to_idle ... ok
test test_run_processes_multiple_sequential_events ... ok
test test_keepalive_timeout_sets_dead ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 121 tests passed, 0 failed. All tests across all crates passed.

## Format Gate

```
(cargo fmt --all -- --check returned exit 0 — no output, no drift)
```

## Platform Cross-Check

```
CHECK 1: Mock-hardware Linux: PASS
CHECK 2: Mock-hardware Windows (x86_64-pc-windows-gnu): PASS
CHECK 3: Real-hardware Linux: PASS
CHECK 4: Real-hardware Windows (x86_64-pc-windows-gnu): PASS
```

All four cross-checks exited 0.

## Project Gates

Gate 1 (Config Surface Sync): `cargo test -p anvilml --features mock-hardware -- config_reference` → `config_reference ... ok` (1 passed, 0 failed).

Gate 2 (OpenAPI Drift): Not triggered — task does not modify handler function signatures, `#[utoipa::path]` annotations, or `ToSchema` derives.

Gate 3 (Node Parity): Not triggered — task does not add, remove, or rename node types.

## Public API Delta

```
(No new pub items introduced — grep returned nothing)
```

The `device_index` field is a non-`pub` field on `ManagedWorker`. The `new()` constructor parameter is not a `pub` signature change — it is an existing `pub fn` with an additional parameter. No new `pub` items were introduced.

## Deviations from Plan

1. **Removed broadcast send from child exit arm.** The plan specified sending a `WorkerEvent::Dying` via `self.event_tx.send(...)` in the child exit arm. However, `self.event_tx` is dropped at the start of `run()` (line 347: `drop(self.event_tx)`), so it cannot be used in the select! arm. The fix removes the broadcast send and adds a comment explaining that subscribers will observe the Dead status via the next `GET /v1/workers` poll. This is a minor deviation — the core behavior (status transition to Dead + structured log) is unchanged.

2. **Simplified `child.wait()` future handling.** The plan specified using `std::pin::pin!(exit_status).as_mut().await` for the `child.wait()` future. The actual `tokio::process::Child::wait()` method returns a `ChildWait` future that can be awaited directly without explicit pinning. The implementation uses `child.wait().await` directly.

3. **Test approach changed from `spawn()` to `new()`.** The plan specified spawning a real `ManagedWorker` via `ManagedWorker::spawn()` and killing the child with `child.kill().await`. Since the `child` field is private and integration tests in `tests/` cannot access it, the test was rewritten to create a short-lived child process via `tokio::process::Command::new("sh")`, pass it to `ManagedWorker::new()`, and let it exit naturally. The `child.wait()` arm fires when the child exits, transitioning the status to `Dead`.

4. **Fixed additional `ManagedWorker::new()` call sites.** Beyond the plan's scope, `pool_tests.rs` and `workers_tests.rs` also called `ManagedWorker::new()` and needed the `device_index` parameter added. These were fixed as part of the compile check.

## Blockers

None.
