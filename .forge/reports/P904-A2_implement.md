# Implementation Report: P904-A2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P904-A2                         |
| Phase         | 904 — worker tokenizer paths    |
| Description   | Fix tokenizer asset directory depth in qwen3.py and clip_l.py |
| Implemented   | 2026-06-23T20:45:00Z            |
| Status        | COMPLETE                        |

## Summary

Fixed tokenizer asset directory path resolution in three CLIP architecture dispatch modules
(t5.py, qwen3.py, clip_l.py). All three files used `parent.parent.parent` (3 levels up from
`worker/nodes/arch/clip/`), which resolves to `worker/nodes/assets/` — a non-existent
directory. The actual tokenizer assets live at `worker/assets/`, requiring `parent.parent.parent.parent`
(4 levels up). The fix corrects the path depth in all three files and updates the inline
comments to accurately document the correction, matching the t5.py comment style.

## Resolved Dependencies

N/A — no dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modified | `worker/nodes/arch/clip/qwen3.py` | Fixed tokenizer_dir path from `parent.parent` to `parent.parent.parent.parent`; updated comment block for consistency |
| Modified | `worker/nodes/arch/clip/clip_l.py` | Fixed tokenizer_dir path from `parent.parent` to `parent.parent.parent.parent`; updated comment block for consistency |
| Modified | `worker/nodes/arch/clip/t5.py` | Fixed tokenizer_dir path from `parent.parent.parent` to `parent.parent.parent.parent`; corrected comment to accurately describe original plan |

## Commit Log

```
 .forge/reports/P904-A2_plan.md   | 157 +++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 ++--
 worker/nodes/arch/clip/clip_l.py |   9 ++-
 worker/nodes/arch/clip/qwen3.py  |   9 ++-
 worker/nodes/arch/clip/t5.py     |   8 +-
 6 files changed, 183 insertions(+), 19 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 8 items

worker/tests/test_arch_clip_qwen3.py::test_can_handle_qwen3 PASSED       [ 12%]
worker/tests/test_arch_clip_qwen3.py::test_can_handle_non_qwen3 PASSED   [ 25%]
worker/tests/test_arch_clip_qwen3.py::test_load_mock_returns_realclip PASSED [ 37%]
worker/tests/test_arch_clip_qwen3.py::test_load_mock_no_torch_import PASSED [ 50%]
worker/tests/test_arch_clip_l.py::test_can_handle_clip_l PASSED          [ 62%]
worker/tests/test_arch_clip_l.py::test_can_handle_non_clip_l PASSED      [ 75%]
worker/tests/test_arch_clip_l.py::test_load_mock_returns_realclip PASSED [ 87%]
worker/tests/test_arch_clip_l.py::test_load_mock_no_torch_import PASSED  [100%]

============================== 8 passed in 0.04s ===============================
```

All Rust tests: 0 failed (full workspace test suite passed).

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
```

All four platform cross-checks exit 0.

## Project Gates

Gate 1 — Config Surface Sync:
```
running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 — OpenAPI Drift: Not triggered (task modifies no handler function signatures,
utoipa annotations, or AppState fields).

Gate 3 — Node Parity: Not triggered (task modifies no node types in `worker/nodes/`).

## Public API Delta

No new pub items introduced. (Python files do not use the `pub` keyword.)

## Deviations from Plan

The approved plan specified `parent.parent.parent` (three levels up) for the corrected
path. However, inspection of the actual filesystem revealed that `worker/assets/` (where
the tokenizer directories actually live) is at `worker/assets/`, which requires
`parent.parent.parent.parent` (four levels up) from `worker/nodes/arch/clip/`:

- `parent` = `worker/nodes/arch/clip`
- `parent.parent` = `worker/nodes/arch`
- `parent.parent.parent` = `worker/nodes`
- `parent.parent.parent.parent` = `worker`

The plan's comment also incorrectly stated that `parent.parent` resolves to
`worker/nodes/assets/` — in fact, `parent.parent` resolves to `worker/nodes/arch/`.
The corrected comment accurately states that the original plan specified
`parent.parent.parent` (which resolves to `worker/nodes/`), and the actual layout
is one level higher at `worker/assets/`.

This deviation was also applied to `t5.py`, which had the same `parent.parent.parent`
path bug and an inconsistent comment. All three files now use the correct depth and
have consistent, accurate comments.

## Blockers

None.
