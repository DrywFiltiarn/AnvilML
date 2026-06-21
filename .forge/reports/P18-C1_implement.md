# Implementation Report: P18-C1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P18-C1                             |
| Phase         | 018 — ZiT Generic Nodes            |
| Description   | worker/pipeline_cache.py: LRU model cache |
| Implemented   | 2026-06-21T17:00:00Z              |
| Status        | COMPLETE                           |

## Summary

Created `worker/pipeline_cache.py` — an in-worker LRU cache (`PipelineCache`) that loader nodes and arch modules use to avoid reloading model components from disk. The cache stores results keyed by `(model_id, dtype)` pairs, evicts the least-recently-used entry when capacity is exceeded, logs evictions at INFO level, and handles `torch.cuda.OutOfMemoryError` by evicting all entries and retrying once. The module is mock-safe: it imports without torch and the OOM handler gracefully skips when torch is unavailable. Five unit tests verify cache hit, cache miss, LRU eviction ordering, single-slot edge case, and mock-mode operation.

## Resolved Dependencies

None. This task uses only Python standard library modules:
- `collections.OrderedDict` — LRU eviction via `move_to_end()` and `popitem(last=False)`
- `logging` — INFO-level eviction logging, DEBUG cache hit/miss logging, WARN OOM logging
- `types` — `Callable` type hint (Python 3.12.x built-in)
- `torch` — lazily checked at module load time; only referenced for `torch.cuda.OutOfMemoryError` when available

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/pipeline_cache.py` | LRU model/pipeline cache implementation |
| CREATE | `worker/tests/test_pipeline_cache.py` | Unit tests for PipelineCache (5 tests) |
| MODIFY | `docs/TESTS.md` | Added test catalogue entries for 5 pipeline_cache tests |

## Commit Log

```
 .forge/reports/P18-C1_plan.md       | 147 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md        |   6 +-
 .forge/state/state.json             |  16 +--
 docs/TESTS.md                       |  45 ++++++++
 worker/pipeline_cache.py            | 130 +++++++++++++++++++++++
 worker/tests/test_pipeline_cache.py | 203 ++++++++++++++++++++++++++++++++++++
 6 files changed, 536 insertions(+), 11 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 5 items

worker/tests/test_pipeline_cache.py::test_cache_hit PASSED               [ 20%]
worker/tests/test_pipeline_cache.py::test_cache_miss PASSED              [ 40%]
worker/tests/test_pipeline_cache.py::test_lru_eviction PASSED            [ 60%]
worker/tests/test_pipeline_cache.py::test_max_entries_one PASSED         [ 80%]
worker/tests/test_pipeline_cache.py::test_oom_evict_all_in_mock PASSED   [100%]

============================== 5 passed in 0.03s ===============================
```

Full Python suite (58 tests): all passed.
Full Rust suite: all passed.

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
--- CHECK 1 PASS ---

# Check 2: Mock-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.50s
--- CHECK 2 PASS ---

# Check 3: Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
--- CHECK 3 PASS ---

# Check 4: Real-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
--- CHECK 4 PASS ---
```

All four platform cross-checks passed.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passed. No config fields were modified in this task.

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

### Gate 3 — Node Parity
Not triggered — this task does not add, remove, or rename a node type in `worker/nodes/`.

## Public API Delta

```
+    def __init__(self, max_entries: int = 2) -> None:
+    def get_or_load(
```

New public items:
- `PipelineCache.__init__` — constructor (method)
- `PipelineCache.get_or_load` — main cache access method (method)

These match the plan's Public API Surface table exactly. No other new `pub` items were introduced.

## Deviations from Plan

- The plan specified 4 tests; I implemented 5 tests (added `test_max_entries_one` for the single-slot edge case as described in the plan's test catalogue).
- The plan's `test_oom_evict_all_in_mock` was implemented as described — it verifies the module is importable and functional without torch, confirming the OOM path is gracefully unreachable in mock mode.

## Blockers

None.
