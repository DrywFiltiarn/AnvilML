# Plan Report: P17-B4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-B4                                      |
| Phase       | 17 — Cancellation                           |
| Description | worker/worker_main.py: Execute handler failure path sends WorkerEvent::Failed |
| Depends on  | P17-B3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-11T13:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Add an outer exception catch around `execute_graph()` in `worker_main.py`'s Execute handler so that when a node raises an unhandled exception during a real job, the dispatch loop sends `WorkerEvent::Failed{job_id, error, traceback}` instead of leaving the job silently hung with no terminal event. This closes the failure path that `P17-B3` deferred.

## Scope

### In Scope
- Modify `worker/worker_main.py`: wrap the `execute_graph()` call in the Execute handler's background thread with a `try/except Exception` that sends `WorkerEvent::Failed{job_id, error: str(exc), traceback: formatted}` on failure.
- Add >=3 new tests in `worker/tests/test_worker_main.py`: verify Failed is sent (not Completed or silence), error contains the exception message, and traceback is populated and non-empty.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No functionality is deferred to any other task.

## Existing Codebase Assessment

**What already exists:** `worker_main.py`'s `_dispatch_loop()` has an Execute handler (lines 163–212) that builds a `ctx_factory`, spawns a background thread calling `execute_graph(graph, ctx_factory)`, joins it, and sends `Completed{job_id, elapsed_ms}`. There is no exception handling around `execute_graph()` — any unhandled exception (e.g. `ValueError` from a cycle in `topo_sort()`, `KeyError` from an unknown node type in `NODE_REGISTRY`, or any exception from a node's `execute()` method) propagates out of `run_execute()`, crashes the background thread, and leaves the dispatch loop waiting on `thread.join()` or proceeding to send `Completed` without ever having received a terminal event from the worker.

**Established patterns:** Tests use `monkeypatch` on `worker.ipc.send_event` and `worker.ipc.recv_message`, feeding messages via `iter()` and breaking the loop with a `ConnectionError` on `StopIteration`. The `TestDispatchLoopExecute` class (already present from P17-B3) uses this exact pattern. Mock `execute_graph` is patched via `unittest.mock.patch("worker.executor.execute_graph", side_effect=...)`.

**Gap between design and source:** The design doc (§14.5) defines `WorkerEvent::Failed` with fields `{job_id, error, traceback}` but no current code sends this event from `worker_main.py`. The `Failed` event type is defined in `anvilml-ipc/messages.rs` as a Rust enum variant; the Python side must produce a matching dict. This task is the first to bridge that gap.

## Resolved Dependencies

None. This task uses only Python standard library modules (`traceback`) and existing project modules. No new external dependencies are introduced.

## Approach

1. **Modify `worker_main.py` Execute handler (lines 163–212):** Add a `try/except Exception` around the `execute_graph()` call inside `run_execute()`. On exception, format the traceback using `traceback.format_exc()` and send `WorkerEvent::Failed{job_id, error: str(exc), traceback: formatted}` via `ipc.send_event()`. Log the failure at ERROR level with structured fields `job_id` and `error`. Do NOT re-raise the exception — the dispatch loop must continue processing subsequent messages.

   Specifically, change the Execute handler to:
   ```python
   elif msg_type == "Execute":
       # ... existing setup (imports, ctx_factory, start time) ...
       thread = threading.Thread(target=run_execute, daemon=True)
       thread.start()
       thread.join()

       # Check if the thread encountered an exception.
       # We need a way to communicate whether execution succeeded or failed.
       # Approach: use a result container that the background thread writes to,
       # and the main thread reads after join().
   ```

   The implementation will use a shared result dict (or a `threading.Event` + result storage) so that `run_execute()` can communicate whether it succeeded or failed. After `thread.join()`, the main loop checks the result: on success, send `Completed`; on failure, send `Failed`.

   This approach avoids the complexity of catching exceptions inside the daemon thread (which would need to propagate to the main thread via a shared mutable container anyway) and keeps the exception handling localized to where it belongs: the dispatch loop that owns the IPC send.

2. **Add tests in `test_worker_main.py`** under a new test class `TestDispatchLoopExecuteFailure`:
   - Test 1: A node raising inside `execute_graph()` results in `Failed` being sent (not `Completed`, not silence). Mock `execute_graph` to raise `ValueError("test error")`, feed an Execute message, verify `Failed` event is sent with correct `job_id`.
   - Test 2: The `error` field contains the exception message. Assert `event["error"]` includes the original exception's string representation.
   - Test 3: The `traceback` field is populated and non-empty. Assert `event["traceback"]` is a non-empty string containing traceback formatting markers (e.g., "Traceback").

   Each test follows the established pattern: monkeypatch `ipc.send_event` to capture sent events, feed messages via `iter()`, break the loop with `ConnectionError`, and patch `execute_graph` via `unittest.mock.patch`.

3. **Add logging:** At the ERROR level, log `dispatch_loop: execute failed job_id=%s error=%s` after sending the Failed event. This satisfies the mandatory logging obligation.

## Public API Surface

No new public items. This task modifies an existing private function (`_dispatch_loop`) and adds private test methods. The `WorkerEvent::Failed` dict shape is an IPC contract, not a Python-level API.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/worker_main.py` | Add exception handling in Execute handler's background thread; send Failed on exception |
| MODIFY | `worker/tests/test_worker_main.py` | Add >=3 new tests for the failure path |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_execute_failure_sends_failed_event` | When `execute_graph()` raises, `Failed` is sent (not `Completed`, not silence) with correct `job_id` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -k "test_execute_failure_sends_failed_event"` exits 0 |
| `worker/tests/test_worker_main.py` | `test_execute_failure_error_contains_exception_message` | The `error` field in the `Failed` event contains the original exception's string representation | Same pytest command with `-k "test_execute_failure_error_contains_exception_message"` exits 0 |
| `worker/tests/test_worker_main.py` | `test_execute_failure_traceback_is_populated` | The `traceback` field in the `Failed` event is a non-empty string with traceback formatting | Same pytest command with `-k "test_execute_failure_traceback_is_populated"` exits 0 |

## CI Impact

No CI changes required. The existing mock-mode test command (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v -m "not real_mode"`) automatically picks up new tests in `test_worker_main.py`. No new file types, new gates, or new test modules are added.

## Platform Considerations

None identified. The `traceback` module and `str(exc)` are platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `Failed` event dict shape may not match what the Rust `WorkerEvent::Failed` expects. The Rust side expects `job_id: String`, `error: String`, and `traceback: Option<String>`. Python sends a plain dict — if field names or types don't match, msgpack deserialization will fail on the Rust side. | Medium | High | Check `anvilml-ipc/src/messages.rs` for the exact `WorkerEvent::Failed` struct fields before writing the dict. Use matching key names (`job_id`, `error`, `traceback`). |
| The background thread's exception state must be communicated to the main thread after `thread.join()`. Using a shared mutable dict requires careful synchronization (though `thread.join()` provides a happens-before guarantee). | Low | Medium | Use a simple shared dict `{"failed": bool, "error": str, "traceback": str}` written by the background thread in the `except` block and read by the main thread after `join()`. No lock needed because `join()` establishes the memory barrier. |
| Existing P17-B3 tests may break if the new exception handling changes the event ordering or introduces additional events. | Low | Low | Review existing P17-B3 tests and ensure the new code path doesn't alter their assertions. The failure path is only taken when `execute_graph()` raises, which the existing tests don't trigger. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -k "test_execute_failure"` exits 0 (all 3 new tests pass)
- [ ] `grep -c "Failed" worker/tests/test_worker_main.py` returns >= 3 (three tests reference Failed events)
- [ ] `python -m py_compile worker/worker_main.py` exits 0 (syntax check before test run)
