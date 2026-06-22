# Implementation Report: P18-D7

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P18-D7                             |
| Phase         | 018 — ZiT Generic Nodes            |
| Description   | worker/nodes/arch/: restructure into arch/diffusion/ + arch/clip/ siblings |
| Implemented   | 2026-06-22T16:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Restructured `worker/nodes/arch/` by creating a new `arch/diffusion/` subpackage containing the `pkgutil.iter_modules()` scanning logic and `get_module()`/`can_handle()` dispatch functions. Moved `zit.py` into the new subpackage. Rewrote `arch/__init__.py` as a thin re-export shim (`from worker.nodes.arch.diffusion import can_handle, get_module`) that preserves backward-compatible import paths. Updated both test files (`test_arch_init.py` and `test_arch_zit.py`) to assert and import from the new module paths. Updated `docs/TESTS.md` catalogue entries to reflect the new module paths. All 71 Python tests and all Rust tests pass.

## Resolved Dependencies

None. This task introduces no new external dependencies or packages. It is a pure Python file restructure within the existing `worker/` package.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/diffusion/__init__.py` | New diffusion subpackage init with `pkgutil.iter_modules()` scanning logic and `get_module()`/`can_handle()` dispatch functions |
| CREATE | `worker/nodes/arch/diffusion/zit.py` | Moved from `worker/nodes/arch/zit.py` (no content change) |
| MODIFY | `worker/nodes/arch/__init__.py` | Replaced 144-line scanning logic with 10-line thin re-export shim |
| MODIFY | `worker/tests/test_arch_init.py` | Updated `mod.__name__` assertion from `"worker.nodes.arch.zit"` to `"worker.nodes.arch.diffusion.zit"` |
| MODIFY | `worker/tests/test_arch_zit.py` | Updated import path from `worker.nodes.arch.zit` to `worker.nodes.arch.diffusion.zit` (5 occurrences: import, docstring, sys.modules cache clears, import statement) |
| MODIFY | `docs/TASKS_PHASE018.md` | Appended one-line note noting P18-D7 executed the restructure |
| MODIFY | `docs/TESTS.md` | Updated 12 test catalogue entries to reference `worker.nodes.arch.diffusion.zit` instead of `worker.nodes.arch.zit` |

## Commit Log

```
 .forge/reports/P18-D7_plan.md            | 148 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |   6 +-
 .forge/state/state.json                  |  13 +--
 docs/TASKS_PHASE018.md                   |   5 +-
 docs/TESTS.md                            |  28 +++---
 worker/nodes/arch/__init__.py            | 139 ++---------------------------
 worker/nodes/arch/diffusion/__init__.py  | 144 ++++++++++++++++++++++++++++++
 worker/nodes/arch/{ => diffusion}/zit.py |   0
 worker/tests/test_arch_init.py           |   4 +-
 worker/tests/test_arch_zit.py            |  10 +--
 10 files changed, 332 insertions(+), 165 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 12 items

worker/tests/test_arch_init.py::test_get_module_returns_zit_for_zit_model PASSED [  8%]
worker/tests/test_arch_init.py::test_get_module_returns_none_for_unknown_arch PASSED [ 16%]
worker/tests/test_arch_init.py::test_can_handle_still_works_after_refactor PASSED [ 25%]
worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [ 33%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [ 41%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [ 50%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [ 58%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [ 66%]
worker/tests/test_arch_zit.py::test_sample_real_path_raises_not_implemented PASSED [ 75%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [ 83%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [ 91%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [100%]

============================== 12 passed in 0.07s ==============================
```

Full Python test suite (all 71 tests):
```
============================= 71 passed in 1.96s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.76s
     Running tests/config_reference.rs (target/debug/deps/config_reference-e67db010962c3074f789)
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 (OpenAPI Drift) not triggered — task does not modify handler signatures or ToSchema derives.
Gate 3 (Node Parity) — `worker/tests/test_parity.py` does not yet exist (scheduled for P18-E1).

## Public API Delta

```
No new pub items introduced.
```

The public API surface is unchanged. `can_handle` and `get_module` are re-exported via the thin shim in `arch/__init__.py`, so all existing import paths (`from worker.nodes.arch import can_handle, get_module`) continue to work identically. The zit module's public API (`can_handle`, `sample`, `compute_latent_shape`, `MockLatent`, `VAE_SCALE_FACTOR`) is unchanged — only its module path changed from `worker.nodes.arch.zit` to `worker.nodes.arch.diffusion.zit`.

## Deviations from Plan

- The approved plan stated "No other changes needed" for `test_arch_zit.py` beyond the import path on line 19. However, the `test_sample_mock_no_torch_import` test contains 5 additional references to the old module path `worker.nodes.arch.zit` (docstring, 2 sys.modules cache clears, the import statement, and the final cache restore). These were updated to `worker.nodes.arch.diffusion.zit` to ensure the test functions correctly after the restructure. The plan's intent was correct — no logic changes were needed — but the sys.modules cache clearing and import statement in the test also needed updating because they reference the module path directly.

## Blockers

None.
