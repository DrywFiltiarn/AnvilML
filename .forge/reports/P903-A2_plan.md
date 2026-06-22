# Plan Report: P903-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P903-A2                                     |
| Phase       | 903 — Pipeline Cache & Model Path Resolution Retrofit |
| Description | worker/worker_main.py: wire real PipelineCache into NodeContext |
| Depends on  | P18-C1 (pipeline_cache.py implemented and tested) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T07:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace the `pipeline_cache={}` empty-dict placeholder in `worker/worker_main.py` with a single real `PipelineCache` instance, created once at worker process scope (module-level), so that cache entries persist across jobs dispatched to the same worker process. This fixes a defect where every real loader node calling `ctx.pipeline_cache.get_or_load(...)` would fail with `AttributeError: 'dict' object has no attribute 'get_or_load'`.

## Scope

### In Scope
- Import `PipelineCache` from `worker.pipeline_cache` at module level in `worker/worker_main.py`.
- Create exactly one `PipelineCache()` instance at module scope (before `main()`).
- Pass the same instance as `pipeline_cache=` to every `NodeContext(...)` constructed inside the Execute handler.
- Add one test in `worker/tests/test_worker_main.py` asserting the same `PipelineCache` instance (by `id()`) is reused across two sequential Execute messages in one worker process.

### Out of Scope
- Modifying `worker/pipeline_cache.py` — it is already correct and tested (P18-C1).
- Model ID → filesystem path resolution — that is P903-A1's scope.
- Any Rust-side changes.
- Adding logging calls — the `PipelineCache` class already has its own logging; no new log points are needed in `worker_main.py`.

## Existing Codebase Assessment

**What already exists:**
- `worker/pipeline_cache.py` (P18-C1) implements a fully tested `PipelineCache` class with LRU eviction, OOM handling, and `get_or_load(model_id, dtype, loader_fn)` method. It is importable as `from worker.pipeline_cache import PipelineCache`.
- `worker/nodes/base.py` defines `NodeContext.__init__` accepting `pipeline_cache: dict[str, Any]` — the type hint says `dict[str, Any]` but `PipelineCache` is duck-type compatible for the single method nodes call (`get_or_load`). The contract in `TASKS_PHASE903.md` confirms the declared type hint is intentionally left unchanged.
- `worker/worker_main.py` currently creates `NodeContext(..., pipeline_cache={})` per job inside the Execute handler (line 235), which is the defect this task corrects.

**Established patterns:**
- Module-level singletons: `_cancel_flag: list[bool] = [False]` at module scope (line 47) shows the pattern for process-scope state in this file.
- Test style: subprocess-based tests with bounded ROUTER receives via `_recv_with_timeout`, env isolation via `_make_worker_env`, and cleanup in `finally` blocks.
- `ANVILML_WORKER_MOCK=1` is the standard mock-mode trigger for Python worker tests.

**Gap between design doc and source:** The design doc (ANVILML_DESIGN.md §13) specifies `pipeline_cache: The shared LRU model/pipeline cache` on `NodeContext`, but the current source passes an empty dict. This task closes that gap.

## Resolved Dependencies

None. This task uses only existing, already-imported modules within the worker package (`worker.pipeline_cache`). No new external packages or crates are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | — | — | — | — |

No new dependencies. The `PipelineCache` class was already installed in P18-C1.

## Approach

1. **Add import at module level** in `worker/worker_main.py`:
   - After the existing imports from `worker.ipc`, `worker.nodes`, `worker.nodes.base`, and `worker.executor`, add:
     ```python
     from worker.pipeline_cache import PipelineCache
     ```
   - This is a standard package-internal import; no feature flags or version concerns.

2. **Create a module-level `PipelineCache` singleton** in `worker/worker_main.py`:
   - Add `_pipeline_cache: PipelineCache = PipelineCache()` immediately after the `_cancel_flag` definition (around line 47), following the established pattern of module-level state containers:
     ```python
     # Module-level PipelineCache instance — created once at worker process
     # startup so cache entries (loaded model components) persist across
     # all jobs dispatched to this worker process.
     _pipeline_cache: PipelineCache = PipelineCache()
     ```
   - This is the single source of truth for caching in this worker process.

3. **Update the Execute handler** in `worker/worker_main.py`:
   - Replace the `pipeline_cache={}` argument on the `NodeContext(...)` call (line 235) with `pipeline_cache=_pipeline_cache`:
     ```python
     ctx = NodeContext(
         job_id=job_id,
         device=device,
         cancel_flag=_cancel_flag,
         emit=send_event,
         pipeline_cache=_pipeline_cache,  # shared LRU cache (see module-level _pipeline_cache)
     )
     ```
   - Update the comment above the NodeContext construction (lines 227-228) to reflect that `pipeline_cache` is now a real LRU cache shared across jobs, not an empty dict.

4. **Add one test** in `worker/tests/test_worker_main.py`:
   - Write `test_pipeline_cache_reused_across_jobs()` that:
     - Spawns a worker subprocess with a ROUTER socket (same pattern as existing tests).
     - Waits for the Ready event (drain it via `_recv_with_timeout`).
     - Sends two sequential Execute messages to the worker via ROUTER.
     - The worker's `NodeContext` construction must use the same `_pipeline_cache` instance for both jobs.
     - Since the test runs the worker as a subprocess, we cannot directly inspect `ctx.pipeline_cache` inside the worker process. Instead, the test verifies the behavior indirectly: it sends two Execute messages with identical model parameters and checks that the worker processes both successfully (the second job benefits from cache hits). The key assertion is that the worker handles two sequential jobs without error, confirming the cache instance is shared.
     - Actually, the task requires asserting `id(ctx1.pipeline_cache) == id(ctx2.pipeline_cache)`. The cleanest approach is to monkey-patch `NodeContext.__init__` inside the worker subprocess to record the `id()` of the `pipeline_cache` argument in a temp file, then read it back after the jobs complete. But that adds complexity. A simpler approach: monkey-patch `PipelineCache.__init__` to record `id(self)` at module level, and have the worker report it via a custom event. 
     - The simplest correct approach: monkey-patch `NodeContext.__init__` in the worker to capture the `pipeline_cache` id into a temporary file, then after two Execute messages, read the file and assert the two ids match. The worker writes to a temp file using the job_id as a key.
     - Even simpler: use a subprocess-level shared mechanism. Patch `worker.nodes.base.NodeContext.__init__` to store `id(kwargs.get("pipeline_cache"))` in `os.environ` or write to a temp file. Given that each test runs the worker as a subprocess, writing to a temp file is cleanest.
     - The test will:
       1. Spawn worker with a temp file path in env.
       2. Send two Execute messages.
       3. Drain Completed events for both.
       4. Read the temp file (should contain two id() values).
       5. Assert the two values are identical.

## Public API Surface

No new public items are introduced. This task only modifies:
- An existing import in `worker/worker_main.py` (adds `PipelineCache` to the existing import block).
- A module-level variable `_pipeline_cache` (private, underscore-prefixed).
- An argument change in the existing `NodeContext(...)` call.
- One new private test function `test_pipeline_cache_reused_across_jobs` in `worker/tests/test_worker_main.py`.

No changes to any `pub` items in Rust crates, no new `pub` Python classes or functions.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/worker_main.py` | Import `PipelineCache`, create module-level `_pipeline_cache`, pass it to `NodeContext` instead of `{}` |
| MODIFY | `worker/tests/test_worker_main.py` | Add `test_pipeline_cache_reused_across_jobs` test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_pipeline_cache_reused_across_jobs` | The same `PipelineCache` instance (by `id()`) is passed to `NodeContext` for two sequential Execute messages in one worker process | Worker process handling two Execute messages in sequence | Two Execute messages sent to a mock-mode worker; temp file path for capturing pipeline_cache id | Both jobs complete successfully; temp file contains two identical `id()` values | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0 |

The existing 4 tests (`test_mock_startup_sends_ready`, `test_ping_returns_pong`, `test_shutdown_exits_cleanly`, `test_env_vars_read_from_environment`) continue to pass unchanged — the test count goes from 4 to 5.

## CI Impact

No new CI jobs. `worker-linux`/`worker-windows` CI runners execute `pytest worker/tests/` which auto-discovers the new test. No changes to any CI workflow files.

## Platform Considerations

None identified. The `PipelineCache` class is pure Python with no platform-specific code. The temp file approach for the test uses `tempfile.NamedTemporaryFile` which is cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Monkey-patching `NodeContext.__init__` inside the worker subprocess to capture `pipeline_cache` id may interfere with normal node execution or fail if the patch is applied after node modules are already imported | Medium | Medium | Patch at the right import path (`worker.nodes.base.NodeContext`) before any Execute handler runs. The patch should be minimal — only capture the id and call the original `__init__`. Use a try/finally to restore the original method. |
| Temp file from worker subprocess may not be readable by the test if the worker exits before the test reads it | Low | Low | Read the temp file after both jobs complete but before terminating the worker. Add a small `time.sleep(0.2)` after the second job completes to ensure the worker has written the data. |
| Existing tests may fail if they depend on `pipeline_cache` being a plain dict with specific dict methods (e.g. `keys()`, `__setitem__`) | Low | Low | `PipelineCache` does not expose dict-like mutation methods — but no existing test exercises `pipeline_cache` as a dict. The existing tests only check Ready, Ping/Pong, Shutdown, and env vars — none touch `NodeContext.pipeline_cache`. Verify by running full test suite. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0 with 5 tests (same 4 existing + 1 new)
- [ ] `grep -n "pipeline_cache={}" worker/worker_main.py` returns no hits (exit code 1 from grep = no matches found)
- [ ] `worker/.venv/bin/python -m py_compile worker/worker_main.py` exits 0 (syntax check per ENVIRONMENT.md §7)
- [ ] `worker/.venv/bin/python -c "from worker.pipeline_cache import PipelineCache; print(type(PipelineCache()))"` outputs `<class 'worker.pipeline_cache.PipelineCache'>`
