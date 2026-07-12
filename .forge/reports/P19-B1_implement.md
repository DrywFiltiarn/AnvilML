# Implementation Report: P19-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P19-B1                          |
| Phase         | 19 — Model Loading Contract Groundwork |
| Description   | worker/pipeline_cache.py: get_or_load() LRU component cache |
| Implemented   | 2026-07-12T23:30:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `worker/pipeline_cache.py` with the `PipelineCache` class — an LRU cache using `collections.OrderedDict` for per-component model caching in the Python worker. Implemented `get_or_load(key, loader_fn)` with O(1) cache hits, O(1) LRU recency refresh, and O(1) eviction via `popitem(last=False)`. Created 6 pytest tests in `worker/tests/test_pipeline_cache.py` covering cache hit, cache miss, LRU eviction order, recency refresh on access, custom max_entries, and evicted entry re-loading. Updated `docs/TESTS.md` with entries for all 6 tests. All gates passed: compile check, clippy, 4 platform cross-checks, full Rust test suite, 6 Python tests, config_reference gate, and format check.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | (stdlib)  | 3.12             | ENVIRONMENT.md |

No external dependencies — only `collections.OrderedDict`, `typing.Callable`, `typing.Any` from Python stdlib.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/pipeline_cache.py` | New module: `PipelineCache` LRU component cache |
| CREATE | `worker/tests/test_pipeline_cache.py` | New test file: 6 tests for `PipelineCache` |
| MODIFY | `docs/TESTS.md` | Added 6 test catalogue entries |

## Commit Log

```
 .forge/reports/P19-B1_plan.md       | 111 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md        |  33 +-----
 .forge/state/state.json             |  13 ++-
 docs/TESTS.md                       |  72 ++++++++++++
 worker/pipeline_cache.py            | 102 +++++++++++++++++
 worker/tests/test_pipeline_cache.py | 212 ++++++++++++++++++++++++++++++++++++
 6 files changed, 508 insertions(+), 35 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 6 items

worker/tests/test_pipeline_cache.py::test_get_or_load_cached_returns_without_calling_loader PASSED [ 16%]
worker/tests/test_pipeline_cache.py::test_get_or_load_different_keys_each_call_loader PASSED [ 33%]
worker/tests/test_pipeline_cache.py::test_lru_eviction_removes_least_recently_used PASSED [ 50%]
worker/tests/test_pipeline_cache.py::test_access_refreshes_recency PASSED [ 66%]
worker/tests/test_pipeline_cache.py::test_custom_max_entries PASSED      [ 83%]
worker/tests/test_pipeline_cache.py::test_evicted_entry_is_truly_removed PASSED [100%]

============================== 6 passed in 0.07s ===============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.30s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 54.97s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.67s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.94s
```

All four checks exited 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  Running tests/config_reference.rs
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passed. No config fields were added/modified by this task.

## Public API Delta

Python does not use `pub` visibility — the public API consists of the `PipelineCache` class and its two public methods (`__init__`, `get_or_load`), both documented with Google-style docstrings. No `__all__` is needed since the module is imported explicitly by loader nodes.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
