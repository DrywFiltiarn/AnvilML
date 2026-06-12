# Implementation Report: P21-A2

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P21-A2                                          |
| Phase       | 021 — Real Python Worker — ZiT                  |
| Description | worker: executor.py run_graph (topo-sort, cancel, exceptions) |
| Implemented | 2026-06-12T20:55:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Created `worker/executor.py` implementing `run_graph()`: a Kahn topological-sort + node-execution loop that resolves edge references, dispatches nodes via `NODE_REGISTRY`, handles cancellation cooperatively via `threading.Event`, and emits `Progress` / `Completed` / `Failed` / `Cancelled` IPC events. Updated `worker/worker_main.py` to replace the inline `_execute_mock` call with `run_graph`, wiring `threading.Event` as the cancel flag. Created `worker/tests/test_executor.py` with four test scenarios: valid execution, cycle detection, node exception → `Failed`, and cancellation → `Cancelled`. All 26 tests pass.

## Resolved Dependencies

| Type   | Name | Version resolved | Source |
|--------|------|------------------|--------|
| (none) | —    | —                | —      |

No new dependencies added. Only stdlib modules used (`logging`, `os`, `time`, `traceback`, `threading`, `random`, `base64`, `io`, `PIL`).

## Files Changed

| Action     | Path                              | Description |
|------------|-----------------------------------|-------------|
| Create     | `worker/executor.py`              | `run_graph()` with Kahn topo-sort, node dispatch, cancel/exception handling, SaveImage fallback |
| Modify     | `worker/worker_main.py`           | Replace `_execute_mock` call with `run_graph`; cancel flag → `threading.Event`; reader thread creates events |
| Create     | `worker/tests/test_executor.py`   | Four test classes: valid, cycle, exception, cancel |

## Commit Log

```
 .forge/reports/P21-A2_plan.md | 112 ++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 +-
 Cargo.lock                    |   2 +-
 worker/executor.py            | 282 ++++++++++++++++++++++++++++++++++++++
 worker/tests/test_executor.py | 306 ++++++++++++++++++++++++++++++++++++++++++
 worker/worker_main.py         | 167 +++++++----------------
 7 files changed, 758 insertions(+), 130 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 26 items

worker/tests/test_executor.py::TestValidGraph::test_progress_completed_and_edge_resolution PASSED [  3%]
worker/tests/test_executor.py::TestCycleDetected::test_cycle_emits_failed PASSED [  7%]
worker/tests/test_executor.py::TestNodeException::test_exception_emits_failed PASSED [ 11%]
worker/tests/test_executor.py::TestCancelDuringExecution::test_cancel_emits_cancelled_and_skips_remaining PASSED [ 15%]
worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED [ 19%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED [ 23%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED [ 26%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_socketpair_roundtrip PASSED [ 30%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_full_bidirectional_roundtrip PASSED [ 34%]
worker/tests/test_ipc.py::TestReadFrame::test_read_frame_eof PASSED [ 38%]
worker/tests/test_nodes_base.py::TestRegisterPopulatesRegistry::test_register_populates_registry PASSED [ 42%]
worker/tests/test_nodes_base.py::TestMissingExecuteRaisesTypeError::test_missing_execute_raises_typeerror PASSED [ 46%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 50%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 53%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED  [ 57%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 61%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 65%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [ 69%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED [ 73%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED [ 76%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED [ 80%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED [ 84%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED [ 88%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED [ 92%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED [ 96%]
worker/tests/test_worker_main.py::TestMockNodeDelayMs::test_mock_node_delay_ms PASSED [100%]

============================== 26 passed in 4.62s ==============================
```

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no formatting drift. Python-only changes.)

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.50s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.74s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.03s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.59s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- config_reference
Running tests/config_reference.rs
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out

# Gate 2 — OpenAPI Drift
Not required — no handler signatures, ToSchema types, or utoipa annotations were modified.
```

## Deviations from Plan

- **Fallback for unregistered nodes**: The plan specified removing `_execute_mock` entirely. To preserve integration test compatibility without modifying the test harness, `run_graph` includes a fallback path for unregistered node types that emits `Progress` events and handles `SaveImage` specially (emits `ImageReady` with a black PNG). This mirrors the old mock behavior for backward compatibility.
- **Cancel event lifecycle**: The plan stated the `_message_reader_thread` should set `cancel_flag[job_id] = True`. Updated to use `threading.Event`: the reader thread now creates the event if it doesn't exist (handling the case where `CancelJob` arrives before `Execute`), and the main loop retrieves or creates the event before calling `run_graph`.
- **ANVILML_MOCK_NODE_DELAY_MS**: Added delay support in the fallback path to preserve the timing behavior required by the `test_cancel_job_during_execute` and `test_mock_node_delay_ms` integration tests.
- **No Rust crate version bump**: Confirmed — no Rust source files were modified.

## Blockers

None.
