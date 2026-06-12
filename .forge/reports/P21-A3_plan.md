# Plan Report: P21-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A3                                      |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: pipeline_cache.py LRU + OOM trap     |
| Depends on  | P21-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-12T19:07:29Z                        |
| Attempt     | 1                                            |

## Objective

Create `worker/pipeline_cache.py` implementing an LRU pipeline cache backed by `collections.OrderedDict` keyed on `(model_id, dtype)`, with eviction driven by estimated VRAM consumption and `torch.cuda.empty_cache()` calls. Add an OOM trap in `worker/executor.py` that catches `torch.cuda.OutOfMemoryError` during node execution, drops partial state, runs `empty_cache()`, emits `Failed{error:'cuda_oom'}`, and returns the worker to Idle. Skip the OOM trap path when `ANVILML_WORKER_MOCK=1`. Provide a test suite in `worker/tests/test_pipeline_cache.py` that exits 0 under mock mode.

## Scope

### In Scope
- Create `worker/pipeline_cache.py` with `PipelineCache(max_entries=4)` class:
  - `OrderedDict` keyed by `(model_id, dtype)` tuple → `{ pipeline, est_vram_mib }`
  - `get_or_load(model_id, dtype, loader)` method:
    - **Hit**: `move_to_end()` (MRU), return cached pipeline
    - **Miss**: evict LRU entries while `free_vram < est_vram` and cache non-empty (delete pipeline ref + `torch.cuda.empty_cache()` once per eviction), then invoke `loader()` to load new pipeline
  - `_estimate_vram_mib(model_id, dtype)` helper returning a heuristic VRAM estimate
  - `_free_vram_mib()` helper querying `torch.cuda.mem_get_info()` (0 in mock)
- Modify `worker/executor.py` to add `torch.cuda.OutOfMemoryError` catch before the existing generic `Exception` catch in the per-node try/except block:
  - Drop partial node outputs, call `torch.cuda.empty_cache()` once, emit `Failed{error:'cuda_oom', job_id, traceback}`, return `{"status": "failed", "error": "cuda_oom", ...}`
  - Skip OOM trap when `ANVILML_WORKER_MOCK=1` (torch is `None`)
- Modify `worker/worker_main.py` Execute handler to instantiate `PipelineCache()` and pass it to `run_graph()` (currently passes `None`)
- Create `worker/tests/test_pipeline_cache.py` with pytest tests covering:
  - Cache hit → `move_to_end` + return
  - Cache miss → loader called, entry inserted
  - Eviction when VRAM exceeded → LRU evicted, `empty_cache()` called
  - OOM trap in executor → `Failed{error:'cuda_oom'}` emitted, worker stays alive
  - Mock mode: OOM path skipped when `ANVILML_WORKER_MOCK=1`

### Out of Scope
- Changes to `worker/nodes/base.py`, `worker/nodes/zit.py`, or `worker/nodes/common.py` (handled by P21-A1, P21-A5)
- Changes to `worker/defaults.py` (handled by P21-A4)
- Rust-side changes (this is a Python worker task only)
- Integration with the Rust supervisor beyond the existing IPC emit_fn contract
- `worker_main.py` changes beyond wiring the `PipelineCache` instance into `run_graph()`

## Approach

1. **Create `worker/pipeline_cache.py`** with the `PipelineCache` class:
   - Import `collections.OrderedDict`, `logging`, and conditionally `torch` (same guard as `worker_main.py`: `if ANVILML_WORKER_MOCK == "1"` then `torch = None`).
   - `_estimate_vram_mib(model_id, dtype)`: heuristic based on model file size from a registry or a fixed default (e.g. 2048 MiB for unknown models), scaled by dtype factor (`bf16` → 1.0, `fp16` → 1.0, `fp32` → 2.0).
   - `_free_vram_mib()`: if torch is not None and `torch.cuda.is_available()`, call `torch.cuda.mem_get_info()` and return free in MiB; else return a large sentinel (e.g. 8192) so eviction logic doesn't spuriously trigger in CPU/mock paths.
   - `get_or_load(model_id, dtype, loader)`:
     - Key = `(model_id, dtype)`.
     - If key in cache: `move_to_end()`, return `cache[key]["pipeline"]`.
     - Else: while `_free_vram_mib() < est_vram` and `len(cache) > max_entries`: pop `first_key = next(iter(cache))`, delete pipeline reference from cache, call `torch.cuda.empty_cache()`, log DEBUG eviction.
     - Call `loader()` to produce the pipeline object.
     - Store `{ "pipeline": pipeline, "est_vram_mib": est_vram }` in cache, `move_to_end()`.
     - Return pipeline.
   - Add `import logging` and a module-level logger.

2. **Modify `worker/executor.py`** to add the OOM trap:
   - In the per-node try/except block (around line 144–178), insert a `except torch.cuda.OutOfMemoryError as e:` handler *before* the generic `except Exception as e:` handler.
   - The OOM handler: log ERROR with `error=` and traceback, emit `Failed{error:'cuda_oom', job_id, traceback}`, call `torch.cuda.empty_cache()`, return `{"status": "failed", "error": "cuda_oom", "traceback": tb}`.
   - Guard with `if torch is not None` (or `not _mock`) so mock mode skips the OOM path entirely. Since torch is conditionally imported in `worker_main.py` and passed as module-level, the executor will need to check `torch` availability. The simplest approach: import torch conditionally at module level in executor.py (same pattern as worker_main.py), or check `sys.modules.get("torch")` at runtime.
   - Actually, the cleanest approach: the executor already receives `pipeline_cache` which is a `PipelineCache` instance. The OOM trap should be inside the executor's per-node execution, and since `torch` is imported conditionally in `worker_main.py` at module level, the executor should do the same or check for the exception type dynamically.
   - Best approach: in `executor.py`, add `import torch` guarded by `ANVILML_WORKER_MOCK` check (same pattern as `worker_main.py`). Then use `except torch.cuda.OutOfMemoryError as e:` in the try/except chain. In mock mode, `torch` is `None` so the except clause won't match any exception (the name won't resolve, but since torch is `None`, `torch.cuda` will raise `AttributeError` — so we need a different approach).
   - Refined approach: use a try/except with a string-based check or a sentinel module. The safest: define `torch = None` in mock mode at module level in executor.py, then in the except clause use `except Exception as e:` and check `type(e).__module__` contains `"torch"` or `isinstance(e, RuntimeError)` with `"CUDA out of memory"` in the str. Actually, the cleanest Python pattern: import torch conditionally, and in the except clause, reference `torch.cuda.OutOfMemoryError` only when torch is not None. We can do this with a runtime check:
     ```python
     oom_exc = torch.cuda.OutOfMemoryError if torch is not None else None
     try:
         ...
     except oom_exc as e:  # works even if oom_exc is None? No, that's a syntax error.
     ```
   - Better: use a dual-except pattern:
     ```python
     try:
         ...
     except Exception as e:
         if torch is not None and isinstance(e, torch.cuda.OutOfMemoryError):
             # OOM trap path
             ...
             return {"status": "failed", "error": "cuda_oom", ...}
         # fall through to general error handling
         ...
     ```
   - This is clean, works in mock mode (torch is None, so the isinstance check short-circuits), and properly prioritizes OOM over general errors.

3. **Modify `worker/worker_main.py`** Execute handler:
   - Import `PipelineCache` from `worker.pipeline_cache`.
   - Instantiate `pipeline_cache = PipelineCache()` before calling `run_graph()`.
   - Pass `pipeline_cache` instead of `None` to `run_graph()`.

4. **Create `worker/tests/test_pipeline_cache.py`**:
   - Use `unittest.mock` to mock `torch` and `torch.cuda` functions.
   - Test 1: `test_cache_hit` — load a pipeline, call `get_or_load` again with same key → returns cached, no loader call.
   - Test 2: `test_cache_miss` — call `get_or_load` with new key → loader called, entry in cache.
   - Test 3: `test_eviction_on_vram_pressure` — set free_vram low, fill cache beyond `max_entries`, trigger eviction of LRU entry.
   - Test 4: `test_oom_trap_emits_failed` — mock a node that raises `torch.cuda.OutOfMemoryError`, verify `Failed{error:'cuda_oom'}` emitted.
   - Test 5: `test_oom_trap_skipped_in_mock` — when `ANVILML_WORKER_MOCK=1`, OOM error falls through to general exception handler (no special cuda_oom handling).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/pipeline_cache.py` | PipelineCache LRU class with OrderedDict, VRAM-aware eviction, OOM handling |
| Modify | `worker/executor.py` | Add torch.cuda.OutOfMemoryError trap in per-node try/except (before generic Exception catch) |
| Modify | `worker/worker_main.py` | Wire PipelineCache instance into run_graph() call |
| Create | `worker/tests/test_pipeline_cache.py` | pytest suite: cache hit, miss, eviction, OOM trap, mock mode skip |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `worker/tests/test_pipeline_cache.py` | `test_cache_hit` | Same key returns cached pipeline without reloading |
| `worker/tests/test_pipeline_cache.py` | `test_cache_miss` | New key invokes loader and stores result |
| `worker/tests/test_pipeline_cache.py` | `test_eviction_on_vram_pressure` | LRU entry evicted when free VRAM below estimate |
| `worker/tests/test_pipeline_cache.py` | `test_oom_trap_emits_failed` | CUDA OOM → Failed event with error='cuda_oom', empty_cache called |
| `worker/tests/test_pipeline_cache.py` | `test_oom_trap_skipped_in_mock` | ANVILML_WORKER_MOCK=1 → no torch.cuda.OutOfMemoryError handling |

## CI Impact

The Python worker test suite (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`) gains a new test file (`test_pipeline_cache.py`). No Rust CI changes are required since this task only touches Python files in the `worker/` directory. The existing CI gate for Python workers will automatically pick up the new tests.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch.cuda.OutOfMemoryError` may not exist on ROCm builds (different exception name) | Medium | High — OOM trap silently fails on ROCm | Check both `torch.cuda.OutOfMemoryError` and use string-based fallback (`"CUDA out of memory"` in error message). Also check `torch.cuda.OutOfMemoryError` alias. |
| Mock tests need `torch` to be importable for `isinstance` checks | Medium | Medium — tests may fail if torch absent | Use `unittest.mock.patch` to inject a fake `torch.cuda.OutOfMemoryError` class; guard with `if torch is not None`. |
| VRAM estimation heuristic is inaccurate | Low | Medium — may over-evict or under-evict | Use a conservative estimate; the cache has `max_entries` as a hard cap. Fine-tune in P21-A4 when defaults.py provides model metadata. |
| Modifying `executor.py` may break existing exception handling | Low | High — regression in cycle detection, cancel, or general error paths | The OOM check is added as a sub-branch inside the existing `except Exception` handler; no structural changes to the try/except chain. Tests in `test_executor.py` continue to pass. |

## Acceptance Criteria

- [ ] `worker/pipeline_cache.py` exists with `PipelineCache(max_entries=4)` class using `OrderedDict` keyed on `(model_id, dtype)`
- [ ] `get_or_load()` implements hit (move_to_end + return) and miss (evict LRU while free_vram < est, then load)
- [ ] `torch.cuda.empty_cache()` called once per eviction
- [ ] `worker/executor.py` catches `torch.cuda.OutOfMemoryError` before generic `Exception`, emits `Failed{error:'cuda_oom'}`, calls `empty_cache()`, returns to Idle
- [ ] OOM trap skipped when `ANVILML_WORKER_MOCK=1` (torch absent)
- [ ] `worker/worker_main.py` passes `PipelineCache()` instance to `run_graph()` instead of `None`
- [ ] `pytest worker/tests/test_pipeline_cache.py` exits 0 under `ANVILML_WORKER_MOCK=1`
- [ ] Existing tests (`pytest worker/tests/`, `cargo test --workspace --features mock-hardware`) continue to pass
