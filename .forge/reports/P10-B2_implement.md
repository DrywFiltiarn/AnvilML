# Implementation Report: P10-B2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-B2                                            |
| Phase       | 010 — Worker Crash Recovery                       |
| Description | anvilml-worker: end-to-end handshake regression test (spawn → Ready → Idle) |
| Implemented | 2026-06-06T23:45:00Z                              |
| Status      | PARTIAL                                           |

## Summary

Implemented all plan items: removed `#[ignore]` from two existing tests, updated venv path to use `ANVILML_VENV_PATH` env var, added `handshake_completes_once` Rust test, and added `test_double_init_exits` Python test. Also fixed a pre-existing bug where the spawn function used `worker_main.py` instead of `worker/worker_main.py`, preventing the Python worker from being found. The Python test suite passes fully (6/6). The three Rust integration tests that spawn real Python workers (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`) fail due to a pre-existing infrastructure issue with tokio's async I/O on ChildStdout — data written by the Python worker to stdout is not visible to tokio's reader_task. This issue was masked by the original `#[ignore]` attributes. The non-spawning unit tests (12/12) all pass.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source         |
|--------|-------------------|-----------------|----------------|
| Rust   | anvilml-worker    | 0.1.4 (bump)    | Local Cargo.toml |
| Python | msgpack           | 1.7.0 (existing) | system venv  |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| Modify | `crates/anvilml-worker/src/managed.rs` | Remove `#[ignore]` from 2 tests; update venv paths to use env var; add `handshake_completes_once` test; fix script path from `worker_main.py` to `worker/worker_main.py`; add `_repo_root_for_worker()` helper |
| Add | `worker/tests/test_worker_main.py` | Add `test_double_init_exits` test method |

## Commit Log

```
 .forge/reports/P10-B2_plan.md            | 104 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 ++--
 Cargo.lock                               |   2 +-
 crates/anvilml-worker/Cargo.toml         |   2 +-
 crates/anvilml-worker/src/managed.rs     | 126 +++++++++++++++++++++++---
 worker/tests/test_worker_main.py         |  43 +++++++++
 7 files changed, 277 insertions(+), 19 deletions(-)
```

## Test Results

### Rust non-spawning tests (12 passed, 0 failed)

```
running 12 tests
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
test managed::tests::respawn_after_death ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out
```

### Rust spawning tests (3 failed, blocked by infrastructure issue)

All three fail with: `"worker did not reach Ready state in time"` after 10s timeout.
The Python worker process starts and is alive (verified via `kill -0`), but the
reader_task never receives any event from stdout despite the worker sending a Ready
frame. This is a tokio async I/O issue with ChildStdout — data written by the child
process to stdout is not visible to tokio's async read on the pipe.

### Python tests (6 passed, 0 failed)

```
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED

6 passed in 0.31s
```

The Python test suite confirms the worker behaves correctly: InitializeHardware → Ready,
duplicate InitializeHardware is silently ignored, Shutdown → Dying{reason: shutdown},
exit 0.

## Format Gate

```
(not applicable — exit 0, no drift)
```

## Platform Cross-Check

All four checks passed:

1. `cargo check --workspace --features mock-hardware` — Finished (Linux mock)
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished (Windows mock)
3. `cargo check --bin anvilml` — Finished (Linux real)
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished (Windows real)

## Project Gates

Config reference gate: `cargo test -p backend --features mock-hardware -- config_reference` — passed (0 tests matched filter, which is expected).

## Deviations from Plan

1. **Script path fix**: Changed `cmd.arg("worker_main.py")` to `cmd.arg("worker/worker_main.py")` and added `.current_dir(_repo_root_for_worker())`. This is a necessary production code fix — the original code used a hardcoded script name that doesn't exist at the repo root (`worker_main.py` is at `worker/worker_main.py`). Without this fix, no Python worker can be spawned from any working directory.

2. **Pre-existing test infrastructure issue**: The three Rust integration tests that spawn real Python workers fail due to a tokio async I/O issue with ChildStdout. This was pre-existing and masked by `#[ignore]`. The plan stated these tests "will now pass unconditionally" but they don't in this environment. The root cause is that tokio's `ChildStdout` doesn't see data written by the child process to stdout — verified through extensive debugging (os.read works, Python subprocess.Popen works, but tokio async read blocks forever).

## Blockers

1. **Tokio async I/O on ChildStdout**: The three Rust integration tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`) that spawn real Python worker processes fail because tokio's `framing::read_frame(&mut stdout)` never receives data from the child process's stdout. The child process IS alive and DOES write Ready frames to stdout (verified with os.read on raw fd), but tokio's async read blocks forever. This is a pre-existing infrastructure issue that was masked by the original `#[ignore]` attributes. The Python test suite (`test_double_init_exits`) provides equivalent coverage for the double-init scenario and passes fully.
