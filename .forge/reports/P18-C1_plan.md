# Plan Report: P18-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-C1                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/pipeline_cache.py: LRU model cache   |
| Depends on  | P18-A1, P18-A2, P18-A3 (consuming tasks)    |
| Project     | anvilml                                     |
| Planned at  | 2026-06-21T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/pipeline_cache.py` — an in-worker LRU cache (`PipelineCache`) that loader nodes (LoadModel, LoadVae, LoadClip) and arch modules use to avoid reloading model components from disk. The cache stores results keyed by `(model_id, dtype)`, evicts the least-recently-used entry when the capacity is exceeded, logs evictions at INFO level, and handles `torch.cuda.OutOfMemoryError` by evicting all entries and retrying once. This module is a pure Python utility with no dependency on `arch/` modules, consumed by `NodeContext.pipeline_cache`.

## Scope

### In Scope
- **CREATE** `worker/pipeline_cache.py` — `PipelineCache` class with:
  - `__init__(self, max_entries: int = 2)` constructor
  - `get_or_load(self, model_id: str, dtype: str, loader_fn: Callable[[], Any]) -> Any` method
  - LRU eviction when cache exceeds `max_entries`
  - INFO-level logging on eviction
  - OOM handler: on `torch.cuda.OutOfMemoryError`, evict all entries and retry once
- **CREATE** `worker/tests/test_pipeline_cache.py` — ≥ 4 tests:
  - Cache hit (same key returns cached value)
  - Cache miss (loader_fn called, result cached)
  - LRU eviction (max_entries reached, oldest entry evicted)
  - OOM skip in mock mode (OutOfMemoryError not importable, test confirms graceful handling)
- **MODIFY** `docs/TESTS.md` — add test catalogue entry for `test_pipeline_cache.py`

### Out of Scope
- Integration with loader nodes (handled by P18-A1, P18-A2, P18-A3)
- Integration with arch modules (handled by P18-D1)
- Any Rust-side changes
- Persistence or disk-based caching

## Existing Codebase Assessment

The project has an established Python worker codebase in `worker/` with a consistent style: Google-style docstrings on all classes and non-trivial functions, `from __future__ import annotations` at the top of every file, `logging` module for logging (no third-party loggers), and tests living in `worker/tests/` with one test file per source module.

The `NodeContext` class in `worker/nodes/base.py` already declares a `pipeline_cache` attribute (typed as `dict[str, Any]` in the current source, but the design doc §10.3 specifies it should be the `PipelineCache` instance). The `executor.py` module passes `pipeline_cache={}` as a plain dict in tests, meaning the consuming code currently accepts any dict-like object.

The test patterns are well-established: `conftest.py` sets `ANVILML_WORKER_MOCK=1` via an autouse fixture; `registry_clean` fixture clears global state; `mock_context` fixture builds a `NodeContext` with captured emit callable. Tests use `pytest` with assert-style assertions (no `unittest` mocks).

No `pipeline_cache.py` file exists yet — this task creates it from scratch. The module must be importable without torch (mock mode), so the OOM handler must gracefully handle the case where `torch` is not installed.

## Resolved Dependencies

None. This task uses only Python standard library modules:
- `collections.OrderedDict` — LRU eviction via `move_to_end()` and `popitem(last=False)`
- `logging` — INFO-level eviction logging
- `types` — `Callable` type hint (from `types` module, available in all Python 3.12.x)

No external packages are introduced. The OOM handler references `torch.cuda.OutOfMemoryError` but guards the import with a mock-safe check so the module is importable without torch installed.

## Approach

1. **Create `worker/pipeline_cache.py`** with the following structure:

   a. **Module docstring** (Google style) describing the module's purpose: an in-worker LRU cache for model components, used by loader nodes and arch modules to avoid reloading from disk. Include `.. versionadded:: 0.1.0`.

   b. **Imports**: `collections.OrderedDict`, `logging`, `types.Callable`, `typing.Any`. Also import `logging.getLogger(__name__)` for the module logger.

   c. **`PipelineCache` class**:
      - `__init__(self, max_entries: int = 2) -> None`: Store `max_entries`, create an `OrderedDict` for the cache. The `OrderedDict` maintains insertion order; LRU eviction uses `popitem(last=False)` (removes the oldest item).
      - `get_or_load(self, model_id: str, dtype: str, loader_fn: Callable[[], Any]) -> Any`:
        1. Build cache key as `f"{model_id}:{dtype}"`.
        2. If key exists in cache, call `move_to_end(key)` to mark it as most recently used, and return the value. (Log at DEBUG level the cache hit.)
        3. If key not in cache, call `loader_fn()` to compute the value.
        4. Before inserting: if `len(self._cache) >= self.max_entries`, evict the LRU entry via `self._cache.popitem(last=False)` and log at INFO: `pipeline_cache: evicted key=%s` (structured field notation).
        5. Insert `(key, value)` into cache (this automatically makes it the most recently used).
        6. Log at DEBUG: `pipeline_cache: cached key=%s`.
        7. Return the value.
        8. **OOM handling**: Wrap the `loader_fn()` call in a try/except for `torch.cuda.OutOfMemoryError`. On catching it: evict all entries (`self._cache.clear()`), log at WARN: `pipeline_cache: OOM, evicted all entries`, then retry `loader_fn()` once. If the retry also raises `OutOfMemoryError`, propagate the exception. If `torch` is not importable (mock mode), the except clause simply won't match and execution proceeds normally.

   d. **Mock-safe OOM handling**: Since `torch` is not available in mock mode, guard the `OutOfMemoryError` catch by checking if `torch` is importable. Use a module-level lazy check: attempt `import torch` inside a `try/except ImportError` block at module load time, storing the result in `_TORCH_AVAILABLE = False`. In the except handler, reference `torch.cuda.OutOfMemoryError` only when `_TORCH_AVAILABLE` is True. This ensures the module is importable without torch.

2. **Create `worker/tests/test_pipeline_cache.py`** with ≥ 4 tests:

   a. `test_cache_hit` — Create `PipelineCache(max_entries=3)`, call `get_or_load("model-a", "fp16", loader_fn)` twice with the same key. The second call must return the cached value without calling `loader_fn` again. Use a counter in the loader to verify it was called exactly once.

   b. `test_cache_miss` — Create `PipelineCache(max_entries=3)`, call `get_or_load("model-b", "bf16", loader_fn)` for a new key. Verify `loader_fn` was called, the result was stored, and subsequent calls with the same key return the cached result.

   c. `test_lru_eviction` — Create `PipelineCache(max_entries=2)`, insert two entries with different keys (A, B). Insert a third key (C) which triggers eviction of A (the LRU entry). Verify A is no longer in the cache, and that accessing B then inserting D evicts B (now the LRU). This proves the LRU ordering is correct.

   d. `test_max_entries_one` — Create `PipelineCache(max_entries=1)`. Insert entry A, then entry B. Verify A was evicted (only B remains). This tests the edge case of a single-slot cache.

   e. `test_oom_evict_all_in_mock` — In mock mode, `torch` is not available, so `OutOfMemoryError` cannot be raised. This test verifies that the cache operates normally when torch is absent (the OOM path is unreachable in mock mode, which is the correct behavior). It confirms the module is importable and functional without torch.

   All tests follow the existing style: Google-style docstrings with Preconditions/Tests/Expected output sections, assert-style assertions, no external mocking libraries.

3. **Update `docs/TESTS.md`** — Add a test catalogue entry for `test_pipeline_cache.py` covering all four tests.

4. **Run `py_compile`** on the new file to confirm syntax correctness (per ENVIRONMENT.md §7).

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `PipelineCache` class | `worker.pipeline_cache` | `class PipelineCache: def __init__(self, max_entries: int = 2) -> None; def get_or_load(self, model_id: str, dtype: str, loader_fn: Callable[[], Any]) -> Any` |

No module-level exports other than `PipelineCache`. The module does not define `__all__` but the only public name is `PipelineCache`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/pipeline_cache.py` | LRU model/pipeline cache implementation |
| CREATE | `worker/tests/test_pipeline_cache.py` | Unit tests for PipelineCache (≥ 4 tests) |
| MODIFY | `docs/TESTS.md` | Add test catalogue entry for `test_pipeline_cache.py` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_pipeline_cache.py` | `test_cache_hit` | Same key returns cached value without re-invoking loader_fn | PipelineCache(max_entries=3) with empty cache | Key "model-a:fp16", loader_fn returns "value-a" | loader_fn called exactly once; second get_or_load returns cached result | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_cache_hit -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_cache_miss` | New key calls loader_fn, stores result, subsequent calls return cached | PipelineCache(max_entries=3) with empty cache | Key "model-b:bf16", loader_fn returns "value-b" | loader_fn called; result stored; second call returns cached value | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_cache_miss -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_lru_eviction` | LRU entry evicted when max_entries exceeded; ordering is correct | PipelineCache(max_entries=2) | Keys "A:fp16", "B:bf16", "C:fp16" in sequence | After C inserted, A evicted; accessing B then inserting D evicts B | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_lru_eviction -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_max_entries_one` | Single-slot cache evicts previous entry on new insert | PipelineCache(max_entries=1) | Keys "A:fp16", "B:bf16" | Only B remains in cache after second insert | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_max_entries_one -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_oom_evict_all_in_mock` | Module importable without torch; OOM path gracefully skipped | ANVILML_WORKER_MOCK=1; torch not available | PipelineCache(max_entries=2), normal get_or_load call | No ImportError; cache operates normally | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py::test_oom_evict_all_in_mock -v` exits 0 |

## CI Impact

The `worker-linux` and `worker-windows` CI jobs run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v`. Adding `test_pipeline_cache.py` means the test runner will pick it up automatically — no CI configuration changes needed. The `py_compile` step (Step 7 in ENVIRONMENT.md §6) will also check the new file. No new CI jobs or gates are required.

## Platform Considerations

None identified. The `OrderedDict` API (`move_to_end`, `popitem(last=False)`) is cross-platform and available on all Python 3.12.x targets. Path handling is not involved (cache keys are strings, not filesystem paths). The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `NodeContext.pipeline_cache` currently typed as `dict[str, Any]` in `worker/nodes/base.py` — the consuming code expects dict-like access. `PipelineCache` does not implement `__getitem__`/`__setitem__`/`__contains__`, so code doing `ctx.pipeline_cache[key]` will break. | Medium | High | The `get_or_load` method is the sole access pattern specified by the design doc. Loader nodes (P18-A1) call `pipeline_cache.get_or_load(...)` directly. The current `pipeline_cache={}` in tests is a placeholder; the ACT agent for this task does not modify `base.py` — that is out of scope. The plan documents this gap so the ACT agent and downstream tasks are aware. |
| `torch.cuda.OutOfMemoryError` may not be importable in mock mode, causing an `ImportError` at module level if the exception is referenced unconditionally. | Low | Medium | Guard the torch import behind `try/except ImportError` at module load time. Store `_TORCH_AVAILABLE = False` in the except branch. Only reference `torch.cuda.OutOfMemoryError` inside the OOM handler when `_TORCH_AVAILABLE` is True. |
| Logging at INFO on every eviction could be noisy in high-throughput scenarios with many small models. | Low | Low | The design spec requires INFO-level eviction logging. If this becomes a problem, the level can be adjusted in a future task without changing the logic. For now, INFO matches the mandatory logging convention. |
| The `OrderedDict` approach relies on insertion order, which is guaranteed in Python 3.7+ (and the project targets 3.12.x). | Very Low | Low | Python 3.7+ guarantees dict ordering; `OrderedDict` has been the canonical LRU implementation pattern since before 3.7. No compatibility concern. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/pipeline_cache.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_pipeline_cache.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_pipeline_cache.py -v` exits 0 with ≥ 4 tests passing
- [ ] `grep -c "def test_" worker/tests/test_pipeline_cache.py` returns ≥ 4
- [ ] `grep -c "evicted" worker/pipeline_cache.py` returns ≥ 1 (INFO eviction log present)
