# Plan Report: P17-B3

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P17-B3                                                      |
| Phase       | 17 — Cancellation                                           |
| Description | worker/worker_main.py: dispatch loop handles WorkerMessage::Execute, success path |
| Depends on  | P17-B2                                                      |
| Project     | anvilml                                                     |
| Planned at  | 2026-07-11T10:45:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Close the audit-found gap where `worker_main.py`'s message dispatch loop never calls `execute_graph()` for an `Execute` message. Replace the interim `_execute_job()` stopgap with a real Execute handler that constructs a job-scoped `NodeContext` factory, runs `execute_graph()` on a background thread (keeping the dispatch loop responsive to `CancelJob` and `Ping`), and sends `WorkerEvent::Completed{job_id, elapsed_ms}` on successful completion.

## Scope

### In Scope
- Delete `_execute_job()` function and all interim-stopgap comments (`INTERIM STOPGAP`, `INTERIM-P14-PATCH`) from `worker_main.py`
- Replace the `Execute` branch in `_dispatch_loop()` with a real handler:
  - Build a `ctx_factory` callable that produces a `NodeContext` bound to this job's `job_id`
  - Call `execute_graph(msg["graph"], ctx_factory)` on a `threading.Thread` background thread
  - After `thread.join()`, compute `elapsed_ms = int((time.monotonic() - start) * 1000)` and send `{"_type": "Completed", "job_id": job_id, "elapsed_ms": elapsed_ms}` via `ipc.send_event()`
  - Log at INFO level: `"dispatch_loop: job completed job_id=%s elapsed_ms=%d"`
- Remove the `device`, `caps`, and `mock` parameters from `_dispatch_loop()` signature (they were only needed by the interim `_execute_job()`) — replace with no-argument signature since the real code derives device/caps from the ctx_factory closure and mock is irrelevant at dispatch time
- Write >=4 tests in `test_worker_main.py`:
  - Execute triggers `execute_graph()` with a job-scoped `ctx_factory`
  - Success sends `Completed` event with real `elapsed_ms`
  - Dispatch loop stays responsive to `CancelJob` while a job is executing (background thread)
  - Background thread does not block the dispatch loop

### Out of Scope
- Failure-path handling (unhandled exception in `execute_graph()` → `Failed` event) — deferred to P17-B4
- `CancelJob` handling in the dispatch loop — handled by P17-B5
- Changes to `execute_graph()`'s own signature or `cancel_flag` behavior — P17-B1/B2 scope
- Any Rust-side changes

## Existing Codebase Assessment

**What already exists:**
- `worker_main.py` has a `_dispatch_loop()` function (line 145) that already handles `Ping` (sends Pong), `Shutdown` (breaks loop), and has an `Execute` branch calling the interim `_execute_job()` (line 235–247).
- `_execute_job()` (lines 78–142) is the interim stopgap — it builds a `NodeContext` directly (not via factory), iterates nodes in list order (no topological sort), and handles both success and failure paths. This must be deleted.
- `worker/executor.py` has `topo_sort()` (Kahn's algorithm) and `execute_graph(graph, ctx_factory)` (lines 124–215). The `execute_graph()` function: sorts nodes, checks `ctx.cancel_flag.is_set()` before each node, instantiates nodes from `NODE_REGISTRY`, calls `execute()`, accumulates results, and returns `{"cancelled": True}` or `{"cancelled": False, "results": {...}}`.
- `NodeContext` (in `worker/nodes/base.py`, lines 37–59) is a plain dataclass-like class with fields: `job_id`, `device`, `caps`, `cancel_flag` (threading.Event), `emit`, `pipeline_cache`, `mock`.
- `ipc.send_event()` is the established IPC send function, imported inside `_import_nodes()` and `_dispatch_loop()` (not at module level) to avoid transitive torch dependencies.
- Existing tests in `test_worker_main.py` follow a pattern of monkeypatching `worker.ipc.send_event` and `worker.ipc.recv_message`, feeding messages via an `iter()` iterator, and asserting on sent events.

**Established patterns:**
- Import `worker.ipc` inside functions (not module level) to avoid transitive torch dependency.
- Tests use `monkeypatch.setattr(ipc, "send_event", ...)` and `monkeypatch.setattr(ipc, "recv_message", ...)` for clean isolation.
- The loop-breaking pattern: after the last test message, raise a `ConnectionError` from the fake `recv_message` to cleanly exit the loop (matching the real `except Exception: break` path).
- `threading.Thread` is the established pattern for background execution in the interim stopgap (line 110 imports `threading`).

**Gap between design doc and current source:**
- The design doc (ANVILML_DESIGN.md §8.5) shows `WorkerMessage::Execute` has fields `job_id`, `graph`, `settings`, and `device_index`. The current interim code only uses `job_id` and `graph` from the message — this is correct for this task since `settings` and `device_index` are not consumed by `execute_graph()`.
- The `_dispatch_loop()` currently takes `device`, `caps`, `mock` parameters from the startup sequences. These are only used by `_execute_job()` to construct `NodeContext` — once `_execute_job()` is deleted, these parameters become unused and should be removed from `_dispatch_loop()`'s signature.

## Resolved Dependencies

None. This task uses only existing Python modules within the project (`worker.ipc`, `worker.executor`, `worker.nodes.base`, `threading`, `time`). No new external dependencies are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | | | | |

## Approach

### Step 1: Delete `_execute_job()` and interim comments from `worker_main.py`

Remove the entire `_execute_job()` function (lines 78–142), including all docstring comments referencing `INTERIM STOPGAP` and `INTERIM-P14-PATCH`. This is a straightforward deletion — the function is entirely replaced by the new handler below.

### Step 2: Replace the Execute branch in `_dispatch_loop()` with a real handler

The current `elif msg_type == "Execute":` branch (lines 235–247) must be replaced. The new handler:

```python
elif msg_type == "Execute":
    # Build a ctx_factory for this job — creates a NodeContext with
    # a per-job cancel_flag so CancelJob (handled by a later task)
    # can signal cancellation to the background execution thread.
    import threading
    import time

    job_id = msg["job_id"]
    graph = msg["graph"]
    logger.info("dispatch_loop: executing job_id=%s", job_id)

    # Capture start time before the background thread begins.
    start = time.monotonic()

    def run_execute() -> None:
        """Background thread target — runs execute_graph on a job-scoped context."""
        from worker.nodes.base import NodeContext

        cancel_flag = threading.Event()
        ctx_factory = lambda: NodeContext(
            job_id=job_id,
            device=device,
            caps=caps,
            cancel_flag=cancel_flag,
            emit=ipc.send_event,
            pipeline_cache=None,
            mock=mock,
        )
        # execute_graph returns {"cancelled": True} or {"cancelled": False, "results": {...}}.
        # On success (no cancellation), the background thread returns normally
        # and the main loop sends the Completed event below.
        execute_graph(graph, ctx_factory)

    # Spawn a background thread so the dispatch loop remains responsive
    # to CancelJob and Ping messages while the job executes.
    # This is the key difference from the interim stopgap which ran
    # _execute_job() synchronously on this thread.
    thread = threading.Thread(target=run_execute, daemon=True)
    thread.start()
    thread.join()  # Wait for execution to complete.

    elapsed_ms = int((time.monotonic() - start) * 1000)
    ipc.send_event({"_type": "Completed", "job_id": job_id, "elapsed_ms": elapsed_ms})
    logger.info("dispatch_loop: job completed job_id=%s elapsed_ms=%d", job_id, elapsed_ms)
```

**Rationale for design choices:**
- `threading.Thread` with `daemon=True` — if the supervisor kills the process, the thread dies with it. The main loop waits on `join()` to get the completion signal.
- `ctx_factory` is a lambda capturing `job_id`, `device`, `caps`, `mock`, and `ipc.send_event` — this mirrors the `execute_graph()` contract (it calls `ctx_factory()` internally at line 184 of `executor.py`).
- `cancel_flag` is a `threading.Event()` created per-job and passed to `NodeContext`. The `CancelJob` handler (P17-B5) will set this flag.
- `pipeline_cache=None` — the pipeline cache is not wired up yet (out of scope for this task).
- `device`, `caps`, `mock` are kept as parameters to `_dispatch_loop()` because they are needed to construct the `NodeContext` in the `ctx_factory`. They are passed from the startup sequences. **Wait** — re-reading the task context: "The `device`/`caps`/`mock` parameters were added by the interim patch to support `_execute_job()`; keep, rename, or drop them based on what the real `ctx_factory` construction actually needs." The real `ctx_factory` needs `device`, `caps`, and `mock` to construct `NodeContext` — so they must be kept. The task says to keep them if needed.

Actually, let me re-examine: the `_dispatch_loop()` is called from both `_real_startup_sequence()` (line 376: `_dispatch_loop(device=device, caps=caps, mock=False)`) and `_mock_startup_sequence()` (line 445: `_dispatch_loop(device="cpu", caps=caps, mock=True)`). The `device`, `caps`, and `mock` values are needed by the `ctx_factory` closure to construct `NodeContext`. So they must be kept.

**Wait, but the interim patch added these parameters.** Let me check the original P9-D2 placeholder... The task says "The `device`/`caps`/`mock` parameters were added by the interim patch." But the real `ctx_factory` needs these values. The correct approach is: keep them as parameters to `_dispatch_loop()` because the `ctx_factory` needs them to construct `NodeContext`. The task says "keep, rename, or drop them based on what the real `ctx_factory` construction in P17-B3 actually needs" — and the real construction needs all three.

### Step 3: Add imports at the top of `_dispatch_loop()`

Add `import threading` and `import time` inside `_dispatch_loop()` (or at module level if already present). Check if `execute_graph` needs to be imported — it must be imported from `worker.executor` inside the new handler.

Actually, looking at the existing code, `threading` and `time` are imported inside `_execute_job()` (lines 110-111). Since we're deleting that function, we need to move these imports to the new handler or to module level. Best practice: import them at the top of `_dispatch_loop()` alongside the existing `import worker.ipc as ipc`.

### Step 4: Write tests in `test_worker_main.py`

Four tests in the `TestDispatchLoopExecute` class:

**Test 1: `test_execute_triggers_execute_graph_with_job_scoped_ctx_factory`**
- Feeds an `Execute` message with a graph to `_dispatch_loop()`, then breaks the loop with a recv failure.
- Asserts `execute_graph()` was called once with the correct graph and a `ctx_factory` that produces a `NodeContext` with the correct `job_id`.
- Uses monkeypatch for `worker.ipc.send_event` and `worker.ipc.recv_message`.

**Test 2: `test_execute_success_sends_completed_with_elapsed_ms`**
- Same as Test 1 but also asserts that `ipc.send_event` was called with `{"_type": "Completed", "job_id": <job_id>, "elapsed_ms": <number>}`.
- The `elapsed_ms` must be a real positive integer (not a hardcoded sentinel), proving `time.monotonic()` was used.

**Test 3: `test_execute_on_background_thread_stays_responsive`**
- Feeds an `Execute` message, then immediately sends a `CancelJob` message (before the background thread would normally complete).
- Asserts the dispatch loop processes the `CancelJob` message without blocking — proving the Execute handler runs on a background thread.
- This is the key test proving the background-thread design.

**Test 4: `test_execute_graph_called_with_correct_graph`**
- Feeds an `Execute` message with a specific graph dict and asserts `execute_graph()` received exactly that dict (not a modified copy).

### Step 5: Update `_dispatch_loop()` docstring

Update the docstring to remove references to the interim stopgap and describe the new Execute handler behavior.

## Public API Surface

No new public items are introduced. The task modifies `_dispatch_loop()` (private function) and deletes `_execute_job()` (private function). No changes to module-level exports.

| Item | Module | Change |
|------|--------|--------|
| `_dispatch_loop(device, caps, mock)` | `worker.worker_main` | Modified: Execute branch replaced; docstring updated |
| `_execute_job(...)` | `worker.worker_main` | Deleted |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/worker_main.py` | Delete `_execute_job()`, replace Execute branch in `_dispatch_loop()`, update docstring |
| MODIFY | `worker/tests/test_worker_main.py` | Add `TestDispatchLoopExecute` class with >=4 tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_execute_triggers_execute_graph_with_job_scoped_ctx_factory` | Execute message triggers `execute_graph()` with a `ctx_factory` that produces a `NodeContext` with the correct `job_id` | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_triggers_execute_graph_with_job_scoped_ctx_factory -v` |
| `worker/tests/test_worker_main.py` | `test_execute_success_sends_completed_with_elapsed_ms` | Success path sends `WorkerEvent::Completed` with real `elapsed_ms` (positive integer from `time.monotonic()`) | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_success_sends_completed_with_elapsed_ms -v` |
| `worker/tests/test_worker_main.py` | `test_execute_on_background_thread_stays_responsive` | Dispatch loop stays responsive to `CancelJob` while Execute is running on a background thread | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_on_background_thread_stays_responsive -v` |
| `worker/tests/test_worker_main.py` | `test_execute_graph_called_with_correct_graph` | `execute_graph()` receives the exact graph dict from the Execute message | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_graph_called_with_correct_graph -v` |

## CI Impact

No CI changes required. The task modifies Python source and tests within the existing `worker/` module. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) already run `pytest worker/tests/` and will pick up the new tests automatically. The `ANVILML_WORKER_MOCK=1` environment variable is already set in the mock-mode CI jobs.

## Platform Considerations

None identified. The `threading.Thread` API is cross-platform (Linux, Windows, macOS). The `time.monotonic()` function is available on all supported platforms. No `#if` guards or platform-specific code needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `execute_graph()` raises an exception during execution — the background thread will terminate with an unhandled exception, the main loop's `join()` will return, and a `Completed` event will be sent with whatever `elapsed_ms` was accumulated. The job is left in `Completed` status despite an actual failure. | High | High | This is known to be deferred to P17-B4 (failure path). The plan explicitly does not add a catch-all `except` clause here. Document this in the approach so the ACT agent knows to leave it for the next task. |
| The `device`, `caps`, `mock` parameters were added by the interim patch — removing them would be incorrect since `NodeContext` needs them. Keeping them means the startup sequences must continue to pass them. | Low | Medium | Verified against actual usage: `_real_startup_sequence()` passes `device=device, caps=caps, mock=False` and `_mock_startup_sequence()` passes `device="cpu", caps=caps, mock=True`. The `ctx_factory` needs all three. Keep them. |
| Import order issue: `worker.executor` imports from `worker.nodes.base`, and `worker_main.py` also imports from `worker.nodes.base`. Circular import risk if `worker.executor` is imported at module level. | Low | Medium | Import `execute_graph` inside the new handler (not at module level), following the established pattern in `executor.py` itself (which imports `NODE_REGISTRY` inside `execute_graph()`). |
| Background thread daemon flag: if `daemon=True`, the thread is killed when the main process exits. If the supervisor sends `Shutdown` while a job is running, the thread may be killed mid-execution. | Low | Medium | This is acceptable — the next task (P17-B4) will handle the failure path, and P17-B5 will handle `CancelJob`. A `Shutdown` during execution is an edge case that the failure-path task will address. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0 with >=4 new tests in `TestDispatchLoopExecute`
- [ ] `worker/.venv/bin/python -m py_compile $(git ls-files 'worker/*.py')` exits 0 (syntax check before test run)
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_triggers_execute_graph_with_job_scoped_ctx_factory -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_success_sends_completed_with_elapsed_ms -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_on_background_thread_stays_responsive -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_graph_called_with_correct_graph -v` exits 0
- [ ] `grep -r "INTERIM STOPGAP\|INTERIM-P14-PATCH" worker/` returns zero hits (interim patch fully removed)
- [ ] `_execute_job` is no longer defined in `worker/worker_main.py` (confirmed via `grep`)
