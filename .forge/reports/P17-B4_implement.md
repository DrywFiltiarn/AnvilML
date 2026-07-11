# Implementation Report: P17-B4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-B4                                      |
| Phase       | 17 — Cancellation                           |
| Description | worker/worker_main.py: Execute handler failure path sends WorkerEvent::Failed |
| Implemented | 2026-07-11T14:05:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Added exception handling around `execute_graph()` in `worker_main.py`'s Execute handler so that when a node raises an unhandled exception during a real job, the dispatch loop sends `WorkerEvent::Failed{job_id, error, traceback}` instead of leaving the job silently hung. The implementation uses a shared result dict written by the background thread (no lock needed — `thread.join()` provides the happens-before guarantee) and checked by the main loop after `join()`. On success, `Completed` is sent; on failure, `Failed` is sent with the error message and formatted traceback. An ERROR-level log with structured `job_id` and `error` fields is emitted after sending the Failed event.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| (none) |           |                  |                |

This task uses only Python standard library modules (`traceback`, `threading`) and existing project modules. No new external dependencies are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/worker_main.py` | Added `import traceback`; rewrote Execute handler to use shared result dict for success/failure communication; send `Failed` event on exception with error and traceback fields |
| MODIFY | `worker/tests/test_worker_main.py` | Added `TestDispatchLoopExecuteFailure` class with 3 tests for failure path |
| MODIFY | `docs/TESTS.md` | Added 3 test catalogue entries for the new tests |
| MODIFY | `.forge/reports/P17-B4_plan.md` | Plan report (pre-existing) |
| MODIFY | `.forge/state/CURRENT_TASK.md` | State tracking (pre-existing) |
| MODIFY | `.forge/state/state.json` | State tracking (pre-existing) |

## Commit Log

```
 .forge/reports/P17-B4_plan.md    | 109 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 ++--
 docs/TESTS.md                    |  30 +++++++
 worker/tests/test_worker_main.py | 164 +++++++++++++++++++++++++++++++++++++++
 worker/worker_main.py            |  66 +++++++++++++---
 6 files changed, 369 insertions(+), 19 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 28 items

worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys PASSED [  3%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool PASSED [  7%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false PASSED [ 10%]
worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load PASSED [ 14%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_ipc_connect PASSED [ 17%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_cpu_skips_cuda_set_device PASSED [ 21%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_probe_capabilities PASSED [ 25%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_exit_path PASSED [ 28%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_sends_ready_event PASSED [ 32%]
worker/tests/test_worker_main.py::TestNoMockGate::test_import_nodes_returns_registered_nodes PASSED [ 35%]
worker/tests/test_worker_main.py::TestNoMockGate::test_dispatch_loop_exists_and_is_callable PASSED [ 39%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_no_nonzero_exit_for_cpu PASSED [ 42%]
worker/tests/test_worker_main.py::TestNoMockGate::test_mock_startup_sends_ready_event PASSED [ 46%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_in_main_block PASSED [ 50%]
worker/tests/test_worker_main.py::TestDispatchLoopPing::test_ping_receives_matching_pong PASSED [ 53%]
worker/tests/test_worker_main.py::TestDispatchLoopPing::test_multiple_pings_each_get_matching_pong PASSED [ 57%]
worker/tests/test_worker_main.py::TestDispatchLoopPing::test_non_ping_message_gets_no_pong PASSED [ 60%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_shutdown_message_exits_loop_cleanly PASSED [ 64%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_shutdown_after_other_messages_still_exits PASSED [ 67%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_keyboard_interrupt_during_recv_exits_cleanly PASSED [ 71%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_keyboard_interrupt_after_other_messages_still_exits_cleanly PASSED [ 75%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_triggers_execute_graph_with_job_scoped_ctx_factory PASSED [ 78%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_success_sends_completed_with_elapsed_ms PASSED [ 82%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_on_background_thread_stays_responsive PASSED [ 85%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_graph_called_with_correct_graph PASSED [ 89%]
worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_sends_failed_event PASSED [ 92%]
worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_error_contains_exception_message PASSED [ 96%]
worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_traceback_is_populated PASSED [100%]

============================== 28 passed in 1.99s ==============================
```

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift
```

## Platform Cross-Check

Not applicable — task wrote no Rust source files.

## Project Gates

```
cargo test -p anvilml --features mock-hardware -- config_reference
# Exit 0 — config reference gate passed (filtered out, no matching test in this crate)
```

## Public API Delta

```
# No new pub items introduced.
```

The grep returned nothing — this task modifies only private functions (`_dispatch_loop`) and adds private test methods. No new public API items.

## Deviations from Plan

None. The implementation matches the approved plan exactly:
- Shared result dict approach for thread communication (as specified in the plan)
- `traceback.format_exc()` for formatted traceback
- `WorkerEvent::Failed{job_id, error, traceback}` dict sent via `ipc.send_event()`
- ERROR-level log with structured `job_id` and `error` fields
- Three tests: Failed event sent, error contains exception message, traceback is populated
- No deferrals (task's `defers_to` is empty)

## Blockers

None.
