# Plan Report: P17-B5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-B5                                      |
| Phase       | 017 — Cancellation                           |
| Description | worker/worker_main.py: dispatch loop handles WorkerMessage::CancelJob |
| Depends on  | P17-B4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-11T15:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend `worker_main.py`'s `_dispatch_loop()` to handle `WorkerMessage::CancelJob` messages by matching the incoming `job_id` against the currently-executing job and setting its `NodeContext.cancel_flag` (a `threading.Event`). If the `job_id` does not match the current job, log DEBUG and ignore. When the executor stops due to the flag being set, send `WorkerEvent::Cancelled{job_id}` back to the supervisor. This completes the worker side of the cooperative cancellation chain introduced in Phase 17.

## Scope

### In Scope
- Modify `worker/worker_main.py`: add a `CancelJob` branch to `_dispatch_loop()` that matches `job_id` against the currently-executing job and sets `cancel_flag`.
- Add tracking variables in `_dispatch_loop()` for the currently-executing `job_id` and its `cancel_flag`.
- Send `WorkerEvent::Cancelled{job_id}` when `execute_graph()` returns `{"cancelled": True}`.
- Add `>=4` tests in `worker/tests/test_worker_main.py` covering: CancelJob sets cancel_flag for current job, CancelJob for non-matching job_id is ignored without error, cancelled execution sends Cancelled event, and the overall test suite passes in mock mode.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope.

## Existing Codebase Assessment

**What already exists:**
- `worker_main.py`'s `_dispatch_loop()` (line 78) already handles `Ping` (→ Pong), `Shutdown` (→ break), and `Execute` (→ background thread calling `execute_graph()` with success/failure paths). The `Execute` branch builds a `ctx_factory` that captures `cancel_flag = threading.Event()` and passes it to `NodeContext`. The `CancelJob` branch currently only logs `"dispatch_loop: received unknown message type=CancelJob"` and continues (a placeholder left by Phase 9's P9-D2).
- `executor.py`'s `execute_graph()` (line 124) already checks `ctx.cancel_flag.is_set()` before each node execution and returns `{"cancelled": True}` when the flag is set.
- `NodeContext` (in `worker/nodes/base.py`, line 37) already has a `cancel_flag` attribute of type `threading.Event`.
- `WorkerEvent::Cancelled { job_id: Uuid }` is defined in the Rust IPC enum (ANVILML_DESIGN.md §8.6).
- `worker/tests/test_worker_main.py` already has 28+ tests covering mock probe, no-torch-import, real startup, dispatch loop ping/shutdown/execute/failure paths.

**Established patterns:**
- Tests use `monkeypatch.setattr(worker.ipc, ...)` to mock IPC, feeding messages via an `iter()` of dicts with a fake `recv_message()` that raises `ConnectionError` to break the loop.
- Env var isolation uses save/restore in try/finally blocks.
- The `Execute` handler (lines 164–258) uses a shared `result` dict (no lock needed due to `thread.join()` happens-before guarantee).
- Logging uses `logger.info(...)` for operational events and `logger.debug(...)` for internal state, with structured fields via `%s`/`%d` formatting.

**Gap between design doc and current source:**
The design doc (ANVILML_DESIGN.md §14.5) states `cancel_flag` semantics as "a `threading.Event` checked cooperatively, never a forceful interrupt." The current source matches this: `execute_graph()` checks `is_set()` between nodes. The gap is purely in the dispatch loop's `CancelJob` branch — it exists but does nothing.

## Resolved Dependencies

None. This task introduces no new external crates or packages. It only modifies existing Python code that uses standard library modules (`threading`, `logging`) already present in the codebase.

## Approach

**Step 1 — Add tracking variables to `_dispatch_loop()`.**

At the top of the `while True` loop body (after `msg = ipc.recv_message()`), add two local variables initialized to `None`:

```python
current_job_id: str | None = None
current_cancel_flag: threading.Event | None = None
```

These track the currently-executing job's identity and its cancel signal. They are reset to `None` after each job completes (success, failure, or cancellation).

**Step 2 — Add the `CancelJob` branch to the dispatch loop.**

In the existing `if/elif/elif` chain that handles `Ping`, `Shutdown`, and `Execute`, add a new `elif` for `CancelJob`:

```python
elif msg_type == "CancelJob":
    cancel_job_id = msg["job_id"]
    # Compare against the currently-executing job.
    if current_job_id == cancel_job_id and current_cancel_flag is not None:
        # Set the cancel flag so execute_graph() observes it before
        # the next node's execute() call. This is cooperative —
        # we never interrupt a node mid-execute.
        logger.info("dispatch_loop: cancelling job_id=%s", cancel_job_id)
        current_cancel_flag.set()
    else:
        # The cancel was for a job that already completed, or a stale
        # message. This is normal — a race between job completion and
        # the cancel message arrival. Log at DEBUG, not error.
        logger.debug(
            "dispatch_loop: CancelJob for non-current job_id=%s, ignoring",
            cancel_job_id,
        )
```

The comparison `current_job_id == cancel_job_id` uses string equality because `job_id` in the message is a string (extracted from the msgpack dict), and `current_job_id` is also stored as a string.

**Step 3 — Track the current job in the `Execute` branch.**

In the existing `Execute` handler (line 164), after extracting `job_id` and `graph` but before spawning the background thread, set the tracking variables:

```python
job_id = msg["job_id"]
graph = msg["graph"]
logger.info("dispatch_loop: executing job_id=%s", job_id)

# Track this job so CancelJob messages can target it.
current_job_id = job_id
current_cancel_flag = cancel_flag  # defined in run_execute() closure
```

Note: `cancel_flag` is created inside `run_execute()` (line 198), so we need to move the `cancel_flag = threading.Event()` line out of `run_execute()` and into the outer scope, so it's available both for the `NodeContext` closure and for the `CancelJob` handler's comparison.

**Step 4 — Send `WorkerEvent::Cancelled` when `execute_graph()` returns cancelled.**

After `thread.join()`, add a check for the cancelled case. The background thread's `run_execute()` function writes `{"success": True}` on normal completion. When `execute_graph()` returns early due to `cancel_flag` being set, it returns `{"cancelled": True}` — but the current `run_execute()` doesn't distinguish between "cancelled" and "success" in the `result` dict. We need to add a third outcome:

```python
# In run_execute(), after execute_graph() returns:
result_data = execute_graph(graph, ctx_factory)
if result_data.get("cancelled"):
    result["cancelled"] = True
else:
    result["success"] = True
```

Then in the main thread after `thread.join()`:

```python
if result.get("cancelled"):
    # Executor stopped due to cancel flag — send Cancelled event.
    ipc.send_event({"_type": "Cancelled", "job_id": job_id})
    logger.info("dispatch_loop: job cancelled job_id=%s", job_id)
elif result.get("success"):
    # ... existing Completed path ...
else:
    # ... existing Failed path ...
```

**Step 5 — Reset tracking variables after each job.**

After the success/failure/cancelled handling (at the end of the `Execute` handler's outer scope, before the `elif` chain ends), reset tracking:

```python
# Reset tracking for the next job.
current_job_id = None
current_cancel_flag = None
```

**Step 6 — Write tests.**

Four tests in `worker/tests/test_worker_main.py`, following the established `monkeypatch` pattern:

1. `test_canceljob_sets_cancel_flag_for_current_job` — Feed Execute then CancelJob for same job_id, mock `execute_graph` to verify cancel_flag gets set.
2. `test_canceljob_for_nonmatching_job_id_is_ignored` — Feed Execute with job_id "job-a" then CancelJob for "job-b", verify no error is raised and no flag is set.
3. `test_cancelled_execution_sends_cancelled_event` — Feed Execute then CancelJob, mock `execute_graph` to return cancelled, verify `Cancelled` event is sent.
4. `test_canceljob_after_job_completed_is_ignored` — Feed Execute (completes), then CancelJob for the same job_id, verify no error and no event sent.

## Public API Surface

No new public items. This task modifies the private `_dispatch_loop()` function in `worker/worker_main.py`. No `pub` (Python `def` at module level that isn't prefixed with `_`) items are introduced. The only change to existing code is adding an `elif` branch inside `_dispatch_loop()`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Add CancelJob branch to `_dispatch_loop()`, track current job, send Cancelled event |
| Modify | `worker/tests/test_worker_main.py` | Add 4 new tests for CancelJob handling |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_canceljob_sets_cancel_flag_for_current_job` | CancelJob for the currently-executing job sets `NodeContext.cancel_flag` so `execute_graph()` observes it | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_sets_cancel_flag_for_current_job -v` |
| `worker/tests/test_worker_main.py` | `test_canceljob_for_nonmatching_job_id_is_ignored` | CancelJob for a non-matching job_id is logged at DEBUG and ignored without error | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_for_nonmatching_job_id_is_ignored -v` |
| `worker/tests/test_worker_main.py` | `test_cancelled_execution_sends_cancelled_event` | When executor stops due to cancel_flag, `WorkerEvent::Cancelled{job_id}` is sent back to supervisor | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_cancelled_execution_sends_cancelled_event -v` |
| `worker/tests/test_worker_main.py` | `test_canceljob_after_job_completed_is_ignored` | CancelJob for a completed job (job_id no longer current) is ignored without error or event | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_after_job_completed_is_ignored -v` |

Full suite acceptance: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0.

## CI Impact

No CI changes required. The new tests have no `@pytest.mark.real_mode` marker, so they run in both mock-mode and real-mode CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) per the convention that unmarked tests are assumed mock-compatible. No new file types, new gates, or new test modules are added — only new test methods in an existing file.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `threading.Event` API is platform-neutral (works identically on Linux and Windows). The `cancel_flag.set()` call is a no-op if already set, so the cancellation logic is safe on both platforms. No `#ifdef` or `#[cfg]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Moving `cancel_flag` out of `run_execute()` into outer scope changes the closure semantics — `current_cancel_flag` must be assigned before `run_execute()` is defined, or the inner function will reference an unbound variable. | Medium | High | Assign `cancel_flag = threading.Event()` before defining `run_execute()`, and pass it to both the `NodeContext` closure and store in `current_cancel_flag`. The plan's Step 3 explicitly handles this ordering. |
| The `thread.join()` call blocks the dispatch loop, so a CancelJob message arriving while `execute_graph()` runs cannot be processed until the join returns. This means cancellation is inherently delayed until the next checkpoint in `execute_graph()`, which is the correct cooperative behavior but could appear as "slow cancellation" if a single node takes a long time. | Low | Low | This is by design — cooperative cancellation means we never interrupt mid-node. The plan does not attempt to fix this; it's a known constraint of the cooperative model. |
| A CancelJob for a job_id that completed *between* the `thread.join()` return and the tracking reset could match the stale `current_job_id`. | Low | Medium | The comparison happens in the `CancelJob` handler, not after `thread.join()`. Since the dispatch loop is single-threaded (the loop processes one message at a time), a CancelJob arriving after the job completes but before the next Execute will see `current_job_id` set (not yet reset), but the CancelJob branch checks `current_cancel_flag is not None` — if the previous job's flag is already consumed/reset, this is safe. Actually, since `thread.join()` blocks, CancelJob messages arrive *during* execution, not after. The "after completion" case is handled by Step 6's reset. |
| `msg["job_id"]` may not exist if the CancelJob message is malformed. | Low | Medium | The existing code pattern (e.g., `job_id = msg["job_id"]` in the Execute branch) uses direct dict access without `.get()`. Malformed messages are a protocol-level issue handled upstream. This task follows the same pattern for consistency. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_sets_cancel_flag_for_current_job -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_for_nonmatching_job_id_is_ignored -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_cancelled_execution_sends_cancelled_event -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_after_job_completed_is_ignored -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0
