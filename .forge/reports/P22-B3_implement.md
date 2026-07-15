# Implementation Report: P22-B3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P22-B3                          |
| Phase         | 22 — Qwen3 CLIP Arch Module     |
| Description   | worker/nodes/arch/clip/qwen3.py: can_handle() + dispatch registration |
| Implemented   | 2026-07-15T14:20:00Z            |
| Status        | COMPLETE                        |

## Summary

Added `can_handle(key: str) -> bool` to `worker/nodes/arch/clip/qwen3.py` and registered the qwen3 module in the clip dispatcher's `_REGISTERED_MODULES` list. This gives the clip dispatcher its first real entry, transitioning from the zero-module stub state. Three new tests verify the function and dispatch registration. Two pre-existing clip dispatcher tests in `test_arch_dispatch.py` were updated to use a non-matching key ("unknown") since the registry is no longer empty.

## Resolved Dependencies

None. This task introduces no new external dependencies. All imports are from the Python standard library (`logging`, `re`, `typing`, `types.ModuleType`, `safetensors`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/clip/qwen3.py` | Added `can_handle(key: str) -> bool` function at end of file (16 lines) |
| MODIFY | `worker/nodes/arch/clip/__init__.py` | Added import of `qwen3` module and registered it in `_REGISTERED_MODULES` |
| MODIFY | `worker/tests/test_arch_clip_qwen3.py` | Added 3 new test functions for `can_handle` and `get_module` dispatch; updated imports |
| MODIFY | `worker/tests/test_arch_dispatch.py` | Updated 3 clip dispatcher tests to use "unknown" key instead of "qwen3" (registry no longer empty) |
| MODIFY | `docs/TESTS.md` | Added test catalogue entries for the 3 new tests |

## Commit Log

```
 .forge/reports/P22-B3_plan.md        | 149 +++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 +--
 docs/TESTS.md                        |  34 ++++++++
 worker/nodes/arch/clip/__init__.py   |   4 +-
 worker/nodes/arch/clip/qwen3.py      |  16 ++++
 worker/tests/test_arch_clip_qwen3.py |  49 +++++++++++-
 worker/tests/test_arch_dispatch.py   |  29 +++----
 8 files changed, 274 insertions(+), 26 deletions(-)
```

## Test Results

### Rust tests (full workspace, --features mock-hardware)
```
296 tests passed, 0 failed, 0 ignored
```
All crates compiled and tested successfully.

### Python mock-mode tests (`ANVILML_WORKER_MOCK=1 -m "not real_mode"`)
```
115 passed, 63 deselected in 5.96s
```
All 3 new tests collected and passed:
- `test_can_handle_matches_qwen3` — PASSED
- `test_can_handle_rejects_other_keys` — PASSED
- `test_get_module_returns_qwen3_for_matching_key` — PASSED

### Python real-mode tests (`-m real_mode`)
```
63 passed, 115 deselected in 6.45s
```
All real-mode tests passed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

### 1. Mock-hardware Linux
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.28s
```

### 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 58.35s
```

### 3. Real-hardware Linux
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 01s
```

### 4. Real-hardware Windows (x86_64-pc-windows-gnu)
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 04s
```

All four checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
Running tests/config_reference.rs ...
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```
Gate passed. No config fields were modified by this task.

## Public API Delta

```
+def can_handle(key: str) -> bool:
```

One new public item:
- `can_handle(key: str) -> bool` — function in `worker.nodes.arch.clip.qwen3`

This matches the plan's Public API Surface table exactly.

## Deviations from Plan

1. **`test_arch_dispatch.py` clip tests updated (3 tests):** The plan's `test_clip_get_module_returns_none_when_empty`, `test_clip_get_module_does_not_raise_for_various_key_types`, and `test_clip_get_module_skips_module_with_can_handle_false` all used `"qwen3"` as the dispatch key, expecting `None` from an empty registry. Since this task registers qwen3 in the clip dispatcher, the registry is no longer empty and `get_module("qwen3")` now returns the qwen3 module. These tests were updated to use `"unknown"` as the key, which no registered module handles, restoring the expected `None` return. This is a necessary fix — the plan's approach of registering qwen3 directly causes these tests to fail without it.

2. **Test import for identity check:** The `test_get_module_returns_qwen3_for_matching_key` test requires the qwen3 module object itself (for `result is qwen3`), not just `can_handle` from it. Added `import worker.nodes.arch.clip.qwen3 as qwen3_mod` to the test file.

3. **No version bump:** This task modifies only Python files. Per ENVIRONMENT.md §12, Python modules do not carry their own version identifiers, so no Cargo.toml bump was needed.

## Blockers

None.
