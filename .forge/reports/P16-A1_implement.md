# Implementation Report: P16-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-A1                                      |
| Phase       | 016 — Job Cancellation                        |
| Description | worker: cooperative cancel — check cancel_flag between nodes |
| Implemented | 2026-06-10T13:00:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Added cooperative cancellation support to the Python worker (`worker/worker_main.py`). When the worker receives a `CancelJob{job_id}` IPC message during an in-flight `Execute`, it sets an internal per-job cancel flag. A background message reader thread continuously reads from the IPC socket so that `CancelJob` messages can be received while `_execute_mock` is running. The mock executor checks the cancel flag before each node; if set, it emits `Cancelled{job_id}` and returns without emitting `Completed`. Also added `ANVILML_MOCK_NODE_DELAY_MS` environment variable support for per-node sleep between nodes, making cancellation observable in integration tests.

## Resolved Dependencies

Not applicable — no new dependencies added.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Add `CancelJob` handling via background reader thread, cancel flag tracking, `_execute_mock` signature change with cancel check per node, `ANVILML_MOCK_NODE_DELAY_MS` delay support, module docstring update |
| Modify | `worker/tests/test_worker_main.py` | Add `select` import, 3 new tests: `test_cancel_job_during_execute`, `test_cancel_before_execute`, `test_mock_node_delay_ms` |

No Rust files modified. No version bumps needed.

## Commit Log

```
 worker/tests/test_worker_main.py | 244 +++++++++++++++++++++++++++++++++++++++
 worker/worker_main.py            |  81 ++++++++++++-
 2 files changed, 322 insertions(+), 3 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 14 items

worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [  7%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 14%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED  [ 21%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 28%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 35%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [ 42%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED [ 50%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED [ 57%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED [ 64%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED [ 71%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED [ 78%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED [ 85%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED [ 92%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_node_delay_ms PASSED [100%]

============================== 14 passed in 1.23s ==============================
```

Rust tests: 167 tests passed, 0 failed (full workspace with `mock-hardware` feature).
Config gate: `test_toml_key_set_matches_default` passed.

## Format Gate

```
(No output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.40s

# 2. Mock-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.62s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s

# 4. Real-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
```

All four cross-checks exit 0.

## Project Gates

```
# Config drift gate
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.30s
     Running tests/config_reference.rs
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

## Deviations from Plan

- **Background reader thread**: The original plan assumed a single-threaded main loop where `CancelJob` messages would be read between node iterations. However, this doesn't work because the main loop is blocked inside `_execute_mock` and cannot read messages. A background `_message_reader_thread` was added to continuously read from the IPC socket and update the `cancel_flag` concurrently with job execution. All messages go through a `queue.Queue` — the reader thread owns the socket read side, and the main loop reads from the queue. This ensures no messages are lost whether the main loop is in the message loop or inside `_execute_mock`.
- **Test reading with `select()`**: The `test_cancel_job_during_execute` test uses `select.select()` with small reads instead of `proc.stdout.read(4096)` to avoid blocking on a large buffer when the worker only sends small frames.

## Blockers

None.
