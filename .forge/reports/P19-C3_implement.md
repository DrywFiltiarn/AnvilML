# Implementation Report: P19-C3

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P19-C3                                          |
| Phase         | 019 — Model Loading Contract Groundwork         |
| Description   | worker/nodes/loader.py: LoadVae, LoadClip node skeletons (mock-mode only) |
| Implemented   | 2026-07-13T11:00:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Added `LoadVae` and `LoadClip` node classes to `worker/nodes/loader.py`, completing the loader node trio alongside the existing `LoadModel`. Each node follows the identical mock/real-placeholder pattern: mock-mode returns a sentinel dict with no real loading, and real-mode delegates to `pipeline_cache.get_or_load()` with a lambda that raises `NotImplementedError("no diffusion arch module registered yet")`. Both nodes carry the mandatory `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker pair and are registered via `@register`. Added 10 new tests in `worker/tests/test_nodes_loader.py` covering both mock and real paths for each node, plus registry and cache-key tests.

## Resolved Dependencies

No new external dependencies. All dependencies (`BaseNode`, `NodeContext`, `SlotSpec`, `register`, `PipelineCache`) are internal to the project's `worker/` package.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Added `LoadVae` and `LoadClip` classes after `LoadModel` (146 lines added) |
| MODIFY | `worker/tests/test_nodes_loader.py` | Added 10 new test functions (264 lines added) |
| MODIFY | `docs/TESTS.md` | Added 10 test catalogue entries for the new tests (120 lines added) |

## Commit Log

```
 .forge/reports/P19-C3_plan.md     | 298 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +-
 docs/TESTS.md                     | 120 +++++++++++++++
 worker/nodes/loader.py            | 146 +++++++++++++++++++
 worker/tests/test_nodes_loader.py | 264 +++++++++++++++++++++++++++++++++
 6 files changed, 838 insertions(+), 9 deletions(-)
```

## Test Results

### Rust tests (cargo test --workspace --features mock-hardware)

All 316 tests passed across all crates. No failures.

### Python mock-mode tests (ANVILML_WORKER_MOCK=1 pytest -m "not real_mode")

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 123 items / 31 deselected / 92 selected

worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel PASSED [ 57%]
worker/tests/test_nodes_loader.py::test_load_model_in_registry PASSED    [ 58%]
worker/tests/test_nodes_loader.py::test_load_vae_mock_returns_sentinel PASSED [ 59%]
worker/tests/test_nodes_loader.py::test_load_vae_in_registry PASSED      [ 60%]
worker/tests/test_nodes_loader.py::test_load_clip_mock_returns_sentinel PASSED [ 61%]
worker/tests/test_nodes_loader.py::test_load_clip_in_registry PASSED     [ 63%]
...
====================== 92 passed, 31 deselected in 7.73s =======================
```

### Python real-mode tests (pytest -m real_mode)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 123 items / 92 deselected / 31 selected

worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented PASSED [ 41%]
worker/tests/test_nodes_loader.py::test_load_model_real_cache_key_format PASSED [ 45%]
worker/tests/test_nodes_loader.py::test_load_model_real_raises_no_diffusion_arch PASSED [ 48%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented PASSED [ 51%]
worker/tests/test_nodes_loader.py::test_load_vae_real_cache_key_format PASSED [ 54%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_no_diffusion_arch PASSED [ 58%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_not_implemented PASSED [ 61%]
worker/tests/test_nodes_loader.py::test_load_clip_real_cache_key_format PASSED [ 64%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_no_diffusion_arch PASSED [ 67%]
...
====================== 31 passed, 92 deselected in 2.23s =======================
```

## Format Gate

```
cargo fmt --all -- --check
# exited 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.81s — EXIT: 0

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 03s — EXIT: 0

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 06s — EXIT: 0

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 04s — EXIT: 0
```

All four cross-checks passed with exit code 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
EXIT: 0
```

### Gate 3 — Node Parity
`worker/tests/test_parity.py` does not exist in the repository — Gate 3 is not applicable.

### Gate 4 — Mock/Real Parity Markers
```
# Markers present in loader.py:
grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" worker/nodes/loader.py
# 37:    # REAL_PATH_VERIFIED: ...test_load_model_real_raises_not_implemented
# 38:    # MOCK_PATH_VERIFIED: ...test_load_model_mock_returns_sentinel
# 109:   # REAL_PATH_VERIFIED: ...test_load_vae_real_raises_not_implemented
# 110:   # MOCK_PATH_VERIFIED: ...test_load_vae_mock_returns_sentinel
# 182:   # REAL_PATH_VERIFIED: ...test_load_clip_real_raises_not_implemented
# 183:   # MOCK_PATH_VERIFIED: ...test_load_clip_mock_returns_sentinel

# Files lacking REAL_PATH_VERIFIED: (none — exit 1 = no matches)
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
# (empty — no files lacking marker)

# Files lacking MOCK_PATH_VERIFIED: (none — exit 1 = no matches)
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
# (empty — no files lacking marker)
```

All parity markers present. No gaps.

## Public API Delta

No new Rust `pub` items — this task only modifies Python files. Python uses no `pub` keyword; the public API surface is the two new class definitions (`LoadVae`, `LoadClip`) which are registered in `NODE_REGISTRY` via `@register`.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- `LoadVae` class with `NODE_TYPE="LoadVae"`, `INPUT_SLOTS=[SlotSpec("model_id", "STRING")]`, `OUTPUT_SLOTS=[SlotSpec("vae", "VAE")]`
- `LoadClip` class with `NODE_TYPE="LoadClip"`, `INPUT_SLOTS=[SlotSpec("model_id", "STRING"), SlotSpec("clip_type", "STRING", optional=True)]`, `OUTPUT_SLOTS=[SlotSpec("clip", "CLIP")]`
- Both classes decorated with `@register` and carrying both `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers
- 10 new tests (exceeds the >=8 requirement; total is now 15)
- Cache key namespaces: `vae:{model_id}` for LoadVae, `clip:{model_id}` for LoadClip

## Blockers

None.
