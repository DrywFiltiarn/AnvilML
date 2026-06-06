# Implementation Report: P10-B4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-B4                                            |
| Phase       | 010 — Worker Crash Recovery                       |
| Description | anvilml-worker: end-to-end spawn→Ready→Idle regression test validating epoll fix |
| Implemented | 2026-06-07T01:15:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Added a new canonical regression test `spawn_reaches_idle` in `crates/anvilml-worker/src/managed.rs` that validates the epoll edge-trigger fix (P10-B3) by spawning a `ManagedWorker` with mock hardware, confirming it reaches `Idle` status without any sleep or timing workarounds. Tightened the docstring of `test_double_init_exits` in `worker/tests/test_worker_main.py` to accurately describe current Python worker behavior (second InitializeHardware produces no response event). Bumped anvilml-worker crate patch version from 0.1.5 to 0.1.6. All 168 Rust tests pass (including the new test), all 10 Python integration tests pass, all four platform cross-checks pass, and the config reference gate passes.

## Resolved Dependencies

No new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `spawn_reaches_idle` test in `#[cfg(test)] mod tests` (45 lines) |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.5 → 0.1.6 |
| Modify | `worker/tests/test_worker_main.py` | Tighten docstring of `test_double_init_exits` to match actual Python worker behavior |

## Commit Log

```
 .forge/reports/P10-B4_plan.md            | 155 ++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  11 +--
 Cargo.lock                               |   2 +-
 crates/anvilml-worker/Cargo.toml         |   2 +-
 crates/anvilml-worker/src/managed.rs     |  45 ++++++++++
 worker/tests/test_worker_main.py         |   7 +-
 7 files changed, 216 insertions(+), 12 deletions(-)
```

## Test Results

```
Running unittests src/lib.rs (target/debug/deps/anvilml_worker-6bac9ee08669d98a)

running 16 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::status_transitions ... ok
test managed::tests::spawn_reaches_idle ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::handshake_completes_once ... ok
test managed::tests::spawn_ping_pong ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
```

All 168 Rust tests across the workspace: **ok. 168 passed; 0 failed; 0 ignored**

Python tests:
```
worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED
worker/tests/test_ipc.py::TestWindowsGuard::test_windows_binary_mode_guard_present SKIPPED
worker/tests/test_ipc.py::TestWindowsGuard::test_guard_code_exists_in_source PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED

10 passed, 1 skipped in 0.33s
```

## Format Gate

```
cargo fmt --all -- --check
# exit 0 — no output (no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.46s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.69s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.49s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.03s
```

All four platform cross-checks exit 0.

## Project Gates

Config Surface Sync (Gate 1):
```
Running tests/config_reference.rs (target/debug/deps/config_reference-188f25138daffb7d)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

None. All implementation steps followed exactly as specified in the approved plan.

## Blockers

None.
