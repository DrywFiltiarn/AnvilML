# Implementation Report: P904-A1

| Field         | Value                                                           |
|---------------|-----------------------------------------------------------------|
| Task ID       | P904-A1                                                         |
| Phase         | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)           |
| Description   | worker/tests/test_nodes_decode.py: unconditional torch import breaks CI (no torch in base.txt) |
| Implemented   | 2026-06-23T20:30:00Z                                            |
| Status        | COMPLETE                                                        |

## Summary

Added a module-level guarded `import torch` and `pytest.importorskip("torch")` to `worker/tests/test_nodes_decode.py` so the file is importable even when torch is absent (CI's base.txt venv). Wrapped the `test_vaedeode_real_path_returns_pil_image()` function body in a try/except ImportError that calls `pytest.skip()` when torch is unavailable. Guarded `_MockVaeWithDecode.decode()`'s internal `import torch` with its own try/except that raises ImportError, providing defense-in-depth. The fix ensures pytest reports the real-path test as "skipped" during collection rather than "error" when torch is not installed, unblocking CI for all subsequent Group A tasks in this phase.

## Resolved Dependencies

None. This task uses only `pytest.importorskip()`, which is part of pytest's standard public API (available since pytest 2.0). The `pytest>=9.1` dependency already declared in `worker/requirements/base.txt` fully covers this usage.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/tests/test_nodes_decode.py` | Add guarded torch import at module level, `pytest.importorskip("torch")` guard, wrap `test_vaedeode_real_path_returns_pil_image()` body in try/except ImportError, guard `_MockVaeWithDecode.decode()` method's torch import |
| MODIFY | `docs/TESTS.md` | Update `test_vaedeode_real_path_returns_pil_image` entry to reflect new skip behavior when torch is absent |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |   6 +--
 .forge/state/state.json                   |  12 ++---
 docs/TESTS.md                             |   8 +--
 worker/tests/test_nodes_decode.py         | 102 ++++++++++++++++++++++++++----
 4 files changed, 83 insertions(+), 45 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 91 items

worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry PASSED [  1%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image PASSED [  2%]
worker/tests/test_nodes_decode.py::test_vaedeode_metadata_attributes PASSED [  3%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock PASSED [  4%]
worker/tests/test_nodes_decode.py::test_vaedeode_real_path_returns_pil_image PASSED [  5%]
... (86 more Python tests all PASSED)
============================= 91 passed in 19.14s ==============================

Rust tests: all 230+ tests across all crates passed with zero failures.
```

Note: `test_vaedeode_real_path_returns_pil_image` shows PASSED because torch IS installed in this local venv. In CI (without torch), the `pytest.importorskip("torch")` at module level would skip the entire file during collection, producing a "SKIPPED" status instead.

## Format Gate

```
cargo fmt --all -- --check
```
(No output — exit 0, all files formatted.)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
--- CHECK 1 OK ---

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s
--- CHECK 2 OK ---

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
--- CHECK 3 OK ---

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s
--- CHECK 4 OK ---
```

All four platform cross-checks passed.

## Project Gates

### Gate 1 — Config Surface Sync

```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. This task does not modify any config fields, so no other gates are triggered.

## Public API Delta

```
(no output from grep — no new pub items introduced)
```

No new pub items introduced. This task modifies only test code — no public API items are introduced or changed.

## Deviations from Plan

None. The implementation matches the approved plan exactly:
- Guarded module-level import added after existing imports (Step 1)
- `pytest.importorskip("torch")` added immediately after (Step 2)
- Test function body wrapped in try/except ImportError (Step 3)
- `_MockVaeWithDecode.decode()` torch import guarded (Step 4)
- No other bare `import torch` statements exist in the file (Step 5 confirmed)

## Blockers

None.
