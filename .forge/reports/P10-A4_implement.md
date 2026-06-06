# Implementation Report: P10-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A4                                        |
| Phase       | 010 — Worker Crash Recovery                   |
| Description | anvilml: test-only worker PID accessor for crash-recovery proof |
| Implemented | 2026-06-06T20:30:00Z                          |
| Status      | COMPLETE                                      |

## Summary

Implemented a test-only `pid_for` accessor on `WorkerPool` in `anvilml-worker`, gated behind
`#[cfg(any(test, feature = "test-helpers"))]`. Added a `test-helpers` leaf feature flag to
the crate's Cargo.toml. Two unit tests verify that `pid_for` returns `None` for missing workers
and `Some(pid)` when a child process handle is stored. The implementation required adding two
cfg-gated helper methods (`child_pid` and `set_child_for_test`) on `ManagedWorker` because the
`child` field is private and inaccessible from the `pool.rs` module.

## Resolved Dependencies

No new dependencies added — only a leaf feature flag and cfg-gated accessor methods.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Added `test-helpers = []` to `[features]` |
| Modify | `crates/anvilml-worker/src/pool.rs` | Added cfg-gated `pid_for` method and two unit tests |
| Modify | `crates/anvilml-worker/src/managed.rs` | Added cfg-gated `child_pid()` and `set_child_for_test()` helper methods |

## Commit Log

```
 .forge/reports/P10-A4_plan.md        |  94 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 ++--
 crates/anvilml-worker/Cargo.toml     |   1 +
 crates/anvilml-worker/src/managed.rs |  15 +++++
 crates/anvilml-worker/src/pool.rs    | 117 +++++++++++++++++++++++++++++++++++
 6 files changed, 237 insertions(+), 9 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-020e1c1b93a4048d)

running 14 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::spawn_ping_pong ... ignored, requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable
test managed::tests::status_transitions ... ignored, requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable
test env::tests::test_build_env_rocm_linux_hsa ... ok
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok

test result: ok. 12 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.19s
```

Full workspace test suite: 129 passed, 0 failed, 2 ignored across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.29s

# 2. Mock-hardware Windows cross-check (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.66s

# 3. Real-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.22s

# 4. Real-hardware Windows cross-check (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.42s
```

All four platform checks exited 0.

## Project Gates

```
     Running tests/config_reference.rs (target/debug/deps/config_reference-1f98cbbe97070a9b)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **managed.rs helper methods**: The plan specified `worker.child.lock().await` to access the
  child PID, but `child` is a private field on `ManagedWorker`. To keep the accessor test-only
  and cfg-gated, I added two minimal helper methods on `ManagedWorker`:
  - `child_pid()` — returns `Option<u32>` (the PID), used by `pid_for` internally
  - `set_child_for_test()` — stores a `Child` handle in the worker, used by the unit test
  Both are gated behind `#[cfg(any(test, feature = "test-helpers"))]`, matching the pattern of
  the existing `inject_handles_for_test` method.

## Blockers

None.
