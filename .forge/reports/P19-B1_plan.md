# Plan Report: P19-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P19-B1                                      |
| Phase       | 19 — Model Loading Contract Groundwork      |
| Description | worker/pipeline_cache.py: get_or_load() LRU component cache |
| Depends on  | P9-D3                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-12T22:58:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/pipeline_cache.py` with a `PipelineCache` class that provides an LRU-cached `get_or_load(key, loader_fn)` method. This cache is used by loader nodes (LoadModel, LoadVae, LoadClip) to avoid redundant model/component reloads within a single worker process's lifetime. The cache holds one device's models (small max_entries, default 4) and is keyed by `model_id` — it manages raw components only, not assembled pipelines.

## Scope

### In Scope
- Create `worker/pipeline_cache.py` with the `PipelineCache` class
- Implement `__init__(self, max_entries: int = 4)` — configurable LRU capacity
- Implement `get_or_load(self, key: str, loader_fn: Callable[[], Any]) -> Any` — returns cached value or calls loader_fn once and caches the result
- LRU eviction: when cache exceeds `max_entries`, evict the least-recently-used entry
- On eviction, only remove the dict entry — no explicit resource freeing
- Create `worker/tests/test_pipeline_cache.py` with ≥6 tests

### Out of Scope
None. This task's `defers_to` is `[]` — no scope is deferred. The cache is a complete, standalone module. Pipeline assembly from cached components is the diffusion arch module's `sample()` responsibility (separate cache key `f"{model_id}:pipeline"`), and that belongs to Phase 20+.

## Existing Codebase Assessment

The worker module (`worker/`) already contains `executor.py` (topological sort), `ipc.py` (ZeroMQ DEALER transport), `capability.py` (GPU capability probing), and `nodes/base.py` (BaseNode ABC, NodeContext, NODE_REGISTRY). The `NodeContext` class (defined in `worker/nodes/base.py` line 37) already declares `pipeline_cache` as one of its seven attributes — it expects a cache object with a `get_or_load(key, loader_fn)` callable. No `pipeline_cache.py` exists yet; this task creates it from scratch.

The established test patterns in `worker/tests/` are: plain pytest functions (no fixtures in conftest.py), Google-style docstrings on every function describing what is verified, preconditions, inputs, and expected output. Helper functions (e.g. `_make_graph` in `test_executor.py`) are used for test data construction. Tests import only the public interface of the module under test. No external dependencies beyond Python stdlib are used for these tests.

The dual-mode parity marker convention (§10.6) does NOT apply to this task — `PipelineCache.get_or_load()` is not a node `execute()` method or an arch module `load()`/`sample()`/`decode()` function. It is a utility class, not a node or arch module.

## Resolved Dependencies

None. This module uses only Python standard library (`collections.OrderedDict` for LRU tracking, `typing.Callable`/`Any` for type hints). No external crates or packages are introduced.

## Approach

1. **Create `worker/pipeline_cache.py`** with the `PipelineCache` class:
   - Use `collections.OrderedDict` as the underlying storage — it supports O(1) `move_to_end()` for LRU recency tracking and `popitem(last=False)` for LRU eviction.
   - `__init__(self, max_entries: int = 4)`: Store `max_entries` as a private attribute. Initialize an empty `OrderedDict` for the cache.
   - `get_or_load(self, key: str, loader_fn: Callable[[], Any]) -> Any`:
     a. If `key` exists in the cache, call `move_to_end(key)` to mark it as most-recently-used (this refreshes recency so the entry survives future evictions — standard LRU access pattern).
     b. Return the cached value.
     c. If `key` is not in the cache, call `loader_fn()` exactly once, store the result in the dict with `self._cache[key] = value`, call `move_to_end(key)` to mark it as most-recently-used, then evict if needed.
     d. After inserting, if `len(self._cache) > self.max_entries`, call `self._cache.popitem(last=False)` to remove the oldest (least-recently-used) entry. The evicted value is discarded — Python's refcounting handles resource cleanup.
     e. Return the newly loaded value.
   - Add a module-level docstring explaining the cache's purpose (per-component caching, not pipeline assembly).
   - Add a class-level docstring describing the class, constructor parameters, and the `get_or_load` contract.

2. **Create `worker/tests/test_pipeline_cache.py`** with ≥6 tests:
   - Use plain pytest functions (no fixtures needed — the cache is self-contained).
   - Follow the established pattern: docstring on each test describing what it verifies, preconditions, inputs, and expected output.
   - Import only `PipelineCache` from `worker.pipeline_cache`.

No external API verification is needed — only stdlib types are used.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| Class | `worker/pipeline_cache.py::PipelineCache` | `class PipelineCache` |
| `__init__` | `PipelineCache` | `def __init__(self, max_entries: int = 4) -> None` |
| `get_or_load` | `PipelineCache` | `def get_or_load(self, key: str, loader_fn: Callable[[], Any]) -> Any` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/pipeline_cache.py` | New module: `PipelineCache` LRU component cache |
| CREATE | `worker/tests/test_pipeline_cache.py` | New test file: ≥6 tests for `PipelineCache` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_pipeline_cache.py` | `test_get_or_load_cached_returns_without_calling_loader` | Repeated calls with the same key call loader_fn exactly once | `python -m pytest worker/tests/test_pipeline_cache.py::test_get_or_load_cached_returns_without_calling_loader -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_get_or_load_different_keys_each_call_loader` | Different keys each produce their own independent loader_fn call | `python -m pytest worker/tests/test_pipeline_cache.py::test_get_or_load_different_keys_each_call_loader -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_lru_eviction_removes_least_recently_used` | When cache exceeds max_entries, the oldest entry is evicted | `python -m pytest worker/tests/test_pipeline_cache.py::test_lru_eviction_removes_least_recently_used -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_access_refreshes_recency` | Accessing a cached entry moves it to most-recently-used position, protecting it from eviction | `python -m pytest worker/tests/test_pipeline_cache.py::test_access_refreshes_recency -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_custom_max_entries` | Cache respects a non-default max_entries value | `python -m pytest worker/tests/test_pipeline_cache.py::test_custom_max_entries -v` exits 0 |
| `worker/tests/test_pipeline_cache.py` | `test_evicted_entry_is_truly_removed` | After eviction, get_or_load for the evicted key calls loader_fn again (proves the entry is gone, not just overwritten) | `python -m pytest worker/tests/test_pipeline_cache.py::test_evicted_entry_is_truly_removed -v` exits 0 |

Full acceptance: `python -m pytest worker/tests/test_pipeline_cache.py -v` exits 0 with ≥6 tests collected.

## CI Impact

No CI changes required. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) run `pytest worker/tests/ -v` which already picks up any new test file in the directory. No new file types, new gates, or new test modules need CI configuration changes — the pytest discovery pattern is directory-wide.

## Platform Considerations

None identified. The `collections.OrderedDict` API is identical across all supported platforms (Linux, Windows). No `#[cfg(unix)]`/`#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `loader_fn` raises an exception — should the cache record the failed key? | Low | Low | Per the design doc, the cache only stores successful results. A failed loader call does not populate the cache, so subsequent calls retry. This is correct behavior — a transient failure should be retried, not cached as a sentinel. Document this in the docstring. |
| Thread safety — multiple node execute() calls on different threads could race on the OrderedDict | Medium | High | The design doc (§14.4 worker process model) shows the worker runs a single dispatch loop (no threading). The executor processes nodes sequentially. If this assumption changes, a `threading.Lock` should be added. Document the single-threaded assumption in the class docstring. |
| `max_entries=0` — edge case where every call evicts immediately | Low | Low | Allow `max_entries=0` as a valid no-op cache (the cache dict is never populated). This matches the "per-worker component cache" use case where a user might want to disable caching. The eviction check `len > max_entries` naturally handles this: inserting one entry makes len=1 > 0, so it is evicted immediately. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/pipeline_cache.py` exits 0
- [ ] `python -m py_compile worker/tests/test_pipeline_cache.py` exits 0
- [ ] `python -m pytest worker/tests/test_pipeline_cache.py -v` exits 0 with ≥6 tests collected
