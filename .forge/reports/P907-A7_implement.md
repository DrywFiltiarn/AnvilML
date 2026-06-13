# Implementation Report: P907-A7

| Field       | Value                                                |
|-------------|------------------------------------------------------|
| Task ID     | P907-A7                                               |
| Phase       | 907 — ZeroMQ IPC Transport                           |
| Description | anvilml-worker: managed.rs update mock-hardware tests for ZeroMQ |
| Implemented | 2026-06-13T21:45:00Z                                  |
| Status      | COMPLETE                                              |

## Summary

Added test-only IPC infrastructure (`TestIpcHandles` struct, `run_loop_with_pair` function, `reset_ipc_tx_for_test` method) to the `managed.rs` module. Updated `inject_handles_for_test` to create a fresh mpsc channel and spawn a `run_loop_with_pair` task instead of injecting directly into the existing run_loop. Added a new `inject_handles_for_test` test function. Rewrote `eof_sets_dead` and `respawn_after_death` tests to use the new test IPC infrastructure. All 4 modified tests pass. The 4 integration tests that spawn real Python workers (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) fail due to a pre-existing issue with the Python worker not reaching Ready state — confirmed by stashing changes and running the same tests with identical failures.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|------------------|--------|
| (none) | — | — | — |

No new dependencies were added. The `zeromq` crate version (0.4.1) was verified to not include `PairSocket` — only Dealer, Pub, Pull, Push, Rep, Req, Router, and Sub socket types are available.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `TestIpcHandles` struct, `run_loop_with_pair` function, `reset_ipc_tx_for_test` method; update `inject_handles_for_test` to use fresh mpsc channel + `run_loop_with_pair`; rewrite `eof_sets_dead` and `respawn_after_death` tests; add `inject_handles_for_test` test function |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version from `0.1.25` to `0.1.26` |

## Commit Log

```
 .forge/reports/P907-A7_plan.md       | 337 +++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  11 +-
 .forge/tasks/tasks_phase907.json     |  15 +-
 Cargo.lock                           |   2 +-
 crates/anvilml-worker/Cargo.toml     |   2 +-
 crates/anvilml-worker/src/managed.rs | 336 +++++++++++++++++++++++++++++++---
 7 files changed, 669 insertions(+), 40 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-...)

running 20 tests
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_ipc_port ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::restart_exits_0_and_returns_to_idle ... ok
test managed::tests::eof_sets_dead ... ok
test managed::tests::inject_handles_for_test ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::respawn_after_death ... ok
test pool::tests::shutdown_all_stops_all ... ok
test managed::tests::spawn_ping_pong ... FAILED
test managed::tests::status_transitions ... FAILED
test managed::tests::handshake_completes_once ... FAILED
test managed::tests::spawn_reaches_idle ... FAILED

test result: FAILED. 16 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out

--- Modified tests (all passing) ---
test managed::tests::eof_sets_dead ... ok
test managed::tests::inject_handles_for_test ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::respawn_after_death ... ok

--- Pre-existing failures (confirmed by stashing changes) ---
test managed::tests::spawn_ping_pong ... FAILED
  Io(Custom { kind: Other, error: "worker did not reach Ready state in time" })
test managed::tests::status_transitions ... FAILED
  Io(Custom { kind: Other, error: "worker did not reach Ready state in time" })
test managed::tests::handshake_completes_once ... FAILED
  Io(Custom { kind: Other, error: "worker did not reach Ready state in time" })
test managed::tests::spawn_reaches_idle ... FAILED
  Io(Custom { kind: Other, error: "worker did not reach Ready state in time" })
```

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.69s

# Check 2: Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.15s

# Check 3: Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.63s

# Check 4: Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.79s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1: Config surface sync
cargo test -p backend --features mock-hardware -- config_reference
# test result: ok. 0 passed; 0 failed; 0 ignored
```

Gate 1 passes (no config fields were added/removed/renamed by this task).

## Deviations from Plan

- **PAIR sockets unavailable**: `PairSocket` does not exist in the `zeromq` 0.4.1 crate. The available socket types are: Dealer, Pub, Pull, Push, Rep, Req, Router, Sub. Implemented using `DealerSocket` instead, which is identical to the production `IpcHandles` type. The structural changes (`TestIpcHandles`, `run_loop_with_pair`, `reset_ipc_tx_for_test`) are preserved as specified in the plan.
- **4 pre-existing test failures**: `spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, and `spawn_reaches_idle` fail with "worker did not reach Ready state in time". Confirmed pre-existing by stashing changes and running the same tests — identical failures occurred. These tests spawn real Python workers and are unrelated to the mock-IPC changes in this task.

## Blockers

None. All modified tests pass. All cross-checks and gates pass. The 4 failing integration tests are pre-existing failures unrelated to this task's scope.
