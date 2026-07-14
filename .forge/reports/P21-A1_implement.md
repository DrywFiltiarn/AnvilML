# Implementation Report: P21-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P21-A1                          |
| Phase         | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description   | worker/nodes/arch/diffusion/zit.py: compute_latent_shape() formula |
| Implemented   | 2026-07-14T07:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Implemented `compute_latent_shape(width: int, height: int, batch_size: int = 1) -> tuple` in `worker/nodes/arch/diffusion/zit.py` using ZiT's patch-packing formula. Added module-level `MODEL_PATCH_SIZE` and `MODEL_LATENT_CHANNELS` constants (defaulting to 16 and 4) that are updated by `load()` from the checkpoint's actual hyperparameters. The ceiling-division formula `(x + patch_size - 1) // patch_size` correctly handles exact multiples, non-multiples, and zero-dimension edge cases. Added 7 tests covering exact multiples, non-multiples, batch scaling, post-load integration, default batch_size, and zero dimensions. All 152 Python tests (120 mock + 32 real) and all Rust tests pass.

## Resolved Dependencies

None. This task only adds a pure Python function and module-level constants to an existing module. No new external packages or crates are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Added `MODEL_PATCH_SIZE` and `MODEL_LATENT_CHANNELS` constants; added `compute_latent_shape()` function with docstring and dual-mode parity markers; updated `load()` to cache hyperparameters with debug log |
| MODIFY | `worker/tests/test_arch_zit.py` | Added 7 new tests for `compute_latent_shape()` (exact multiple, non-multiple, batch scaling, post-load real, post-load non-multiple real, default batch_size, zero dims); updated import to include `compute_latent_shape` |
| MODIFY | `docs/TESTS.md` | Added 7 new test catalogue entries for the new `compute_latent_shape` tests |
| MODIFY | `.forge/reports/P21-A1_plan.md` | Plan report (pre-existing, not modified by this session) |
| MODIFY | `.forge/state/CURRENT_TASK.md` | State file (updated by this session) |
| MODIFY | `.forge/state/state.json` | State file (updated by orchestrator) |

## Commit Log

```
 .forge/reports/P21-A1_plan.md      | 154 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 ++--
 docs/TESTS.md                      |  86 +++++++++++++++++++++
 worker/nodes/arch/diffusion/zit.py |  59 ++++++++++++++
 worker/tests/test_arch_zit.py      | 132 ++++++++++++++++++++++++++++++-
 6 files changed, 440 insertions(+), 10 deletions(-)
```

## Test Results

```
=== Mock-mode (worker/tests/test_arch_zit.py) ===
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 31 items

worker/tests/test_arch_zit.py::test_infer_hyperparams_regular_fixture PASSED [  3%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED [  6%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED [  9%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_truncated_header_raises PASSED [ 12%]
worker/tests/test_arch_zit.py::test_can_handle_matches_zit PASSED        [ 16%]
worker/tests/test_arch_zit.py::test_can_handle_rejects_unrelated_key PASSED [ 19%]
worker/tests/test_arch_zit.py::test_get_module_returns_zit_for_matching_key PASSED [ 22%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native PASSED [ 25%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_native_non_fp8_caps_fp8 PASSED [ 29%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real PASSED     [ 32%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock PASSED     [ 35%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only PASSED     [ 38%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback PASSED [ 41%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 PASSED [ 45%]
worker/tests/test_arch_zit.py::test_load_meta_construction_then_materialize PASSED [ 48%]
worker/tests/test_arch_zit.py::test_load_no_metadata_construction_then_materialize PASSED [ 51%]
worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams PASSED [ 54%]
worker/tests/test_arch_zit.py::test_load_real_zit_fixture PASSED         [ 58%]
worker/tests/test_arch_zit.py::test_load_mock_zit_fixture PASSED         [ 61%]
worker/tests/test_arch_zit.py::test_load_no_metadata_real PASSED         [ 64%]
worker/tests/test_arch_zit.py::test_load_no_metadata_mock PASSED         [ 67%]
worker/tests/test_arch_zit.py::test_load_tensors_materialized_on_device PASSED [ 70%]
worker/tests/test_arch_zit.py::test_load_key_remapping_direct_match PASSED [ 74%]
worker/tests/test_arch_zit.py::test_load_raises_on_invalid_path PASSED   [ 77%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple PASSED [ 80%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_non_multiple PASSED [ 83%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_batch_scaling PASSED [ 87%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load PASSED [ 90%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_non_multiple_after_load PASSED [ 93%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_default_batch_size PASSED [ 96%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_zero_dims PASSED [100%]

============================== 31 passed in 8.86s ==============================

=== Full mock-mode suite ===
====================== 120 passed, 32 deselected in 4.52s ======================

=== Real-mode suite ===
====================== 32 passed, 120 deselected in 2.62s ======================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.93s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 65s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 69s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 69s
```

## Project Gates

```
Gate 1 (config_reference): 1 passed; 0 failed
Gate 4 (Mock/Real Parity Markers): All files in worker/nodes/ contain both REAL_PATH_VERIFIED and MOCK_PATH_VERIFIED markers. No files missing either marker.
```

## Public API Delta

```
worker/nodes/arch/diffusion/zit.py:
+def compute_latent_shape(width: int, height: int, batch_size: int = 1) -> tuple[int, int, int, int]:
```

New module-level constants (not `pub` in the Rust sense, but module-level state):
- `MODEL_PATCH_SIZE: int = 16`
- `MODEL_LATENT_CHANNELS: int = 4`

## Deviations from Plan

None. All implementation follows the approved plan exactly:
- Module-level constants added after `ARCH` as specified
- `compute_latent_shape()` signature and formula match the plan
- Ceiling division formula with documented rounding rule
- Dual-mode parity markers at the function definition
- `load()` updates module-level state with debug log
- All 7 tests implemented as specified in the plan's Tests table
- `docs/TESTS.md` updated with entries for all 7 new tests

## Blockers

None.
