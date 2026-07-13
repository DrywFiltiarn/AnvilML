# Implementation Report: P19-C2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P19-C2                          |
| Phase         | 019 — Model Loading Contract Groundwork |
| Description   | worker/nodes/loader.py: LoadModel real branch, deferred-raise + markers |
| Implemented   | 2026-07-13T09:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Replaced the bare `raise NotImplementedError("LoadModel real loading deferred to P19-C2")` in `LoadModel.execute()` real branch with `ctx.pipeline_cache.get_or_load(inputs["model_id"], loader_fn)`, where `loader_fn` raises `NotImplementedError("no diffusion arch module registered yet")`. This establishes the caching infrastructure for Phase 20 while keeping the loader_fn as a deliberate raise. Updated the existing real-mode test's match pattern, added two new real-mode tests, and updated `docs/TESTS.md` with entries for all new and modified tests.

## Resolved Dependencies

None. This task introduces no new external dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Replace bare raise with `ctx.pipeline_cache.get_or_load()` call; updated real-branch comment |
| MODIFY | `worker/tests/test_nodes_loader.py` | Updated `_make_ctx()` to accept `pipeline_cache` kwarg; updated existing real-mode test match pattern and docstring; added two new real-mode tests |
| MODIFY | `docs/TESTS.md` | Updated `test_load_model_real_raises_not_implemented` entry; added entries for `test_load_model_real_cache_key_format` and `test_load_model_real_raises_no_diffusion_arch` |

## Commit Log

```
docs/TESTS.md                   |  42 +++++++++++++++++---
 worker/nodes/loader.py          |  20 +++++++---
 worker/tests/test_nodes_loader.py |  73 +++++++++++++++++++++++++++++----
 3 files changed, 117 insertions(+), 18 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 5 items

worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel PASSED [ 20%]
worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented PASSED [ 40%]
worker/tests/test_nodes_loader.py::test_load_model_in_registry PASSED    [ 60%]
worker/tests/test_nodes_loader.py::test_load_model_real_cache_key_format PASSED [ 80%]
worker/tests/test_nodes_loader.py::test_load_model_real_raises_no_diffusion_arch PASSED [100%]

============================== 5 passed in 0.08s ===============================
```

Full Python mock-mode suite: 88 passed, 25 deselected
Full Python real-mode suite: 25 passed, 88 deselected
Rust workspace suite: all crates passed (366+ tests)

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
1. cargo check --workspace --features mock-hardware:
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.70s

2. cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu:
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 03s

3. cargo check --bin anvilml:
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 06s

4. cargo check --bin anvilml --target x86_64-pc-windows-gnu:
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 08s
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 4 — Mock/Real Parity Markers:**
- `grep -L "REAL_PATH_VERIFIED:" worker/nodes/*.py | grep -v __init__ | grep -v base.py` → empty (all files have the marker)
- `grep -L "MOCK_PATH_VERIFIED:" worker/nodes/*.py | grep -v __init__ | grep -v base.py` → empty (all files have the marker)
- All marker-named tests are collectible:
  - `test_load_model_real_raises_not_implemented` ✓
  - `test_load_model_mock_returns_sentinel` ✓
  - `test_execute_real_returns_input` ✓
  - `test_execute_mock_returns_input` ✓

## Public API Delta

```
(no output — no new pub items introduced)
```

No new public API items. The task modifies existing internal code only:
- `LoadModel.execute()` (existing method, real branch implementation changes)

## Deviations from Plan

- **`ctx.pipeline_cache` vs `self.pipeline_cache`:** The approved plan referenced `self.pipeline_cache` implicitly. Inspection of `NodeContext` (in `worker/nodes/base.py`) showed that `pipeline_cache` is an attribute of the context object, not the node. Changed to `ctx.pipeline_cache` to match the established pattern. This is a plan deviation necessitated by the actual codebase structure.

- **`_make_ctx()` signature change:** The approved plan's test 4a called `_make_ctx(mock=False, pipeline_cache=cache)`, but the existing `_make_ctx()` helper only accepted `mock`. Updated the helper to accept an optional `pipeline_cache` kwarg with a default of `{}` (preserving backward compatibility with existing tests).

- **Existing real-mode test needed `PipelineCache`:** The existing test `test_load_model_real_raises_not_implemented` previously used `pipeline_cache={}` which was fine because the old real branch was a bare raise. After switching to `ctx.pipeline_cache.get_or_load()`, the test needed a real `PipelineCache` instance. Updated the test to pass `PipelineCache()` as the cache.

- **Plan Step 2 (markers):** The approved plan stated the existing markers "already reference the correct test function names" and "do not need changing." This was confirmed — the markers remain unchanged and correct.

## Blockers

None.
