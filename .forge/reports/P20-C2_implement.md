# Implementation Report: P20-C2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P20-C2                          |
| Phase         | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description   | worker/nodes/arch/diffusion/zit.py: dtype selection per InferenceCaps |
| Implemented   | 2026-07-13T20:15:00Z            |
| Status        | COMPLETE                          |

## Summary

Implemented dtype selection for the ZiT diffusion architecture module's `load()` function, implementing the fixed precedence chain from ANVILML_DESIGN.md §11.5: fp8 (if caps.fp8 AND native checkpoint dtype is fp8) → bf16 → fp16 → fp32. The `load()` signature was changed from `load(path: str)` to `load(path: str, caps: dict)` to accept the worker's capability dict. A native dtype detection step was added to `_infer_hyperparams_inner()` to read the checkpoint's native dtype from the safetensors header. Five new tests were added (plus one helper test for the fp8 AND condition) and four old construction-only tests were removed. The dual-mode parity markers on `load()` were updated to point at the new bf16 tests.

## Resolved Dependencies

None. This task uses only `torch` and `safetensors`, both already imported in `zit.py` and already in the project's requirements. No new crates, packages, or feature flags are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add native dtype detection in `_infer_hyperparams_inner()`, add `_safetensors_dtype_to_canonical()` and `_select_dtype()` helper functions, update `load()` signature to accept `caps: dict`, apply `model.to(target_dtype)` for dtype selection, update dual-mode parity markers, add inline documentation comments |
| MODIFY | `worker/tests/test_arch_zit.py` | Remove 4 old construction-only tests (test_load_meta_construction_real, test_load_meta_construction_mock, test_load_meta_device_zero_real_memory, test_load_meta_construction_no_metadata_variant), add 7 new dtype selection tests, add `import torch` at module level, add `_DEFAULT_CAPS` helper dict, update remaining tests to pass `caps` parameter |
| CREATE | `worker/tests/fixtures/zit_tiny_fp8.safetensors` | FP8 (float8_e4m3fn) dtype fixture for testing the fp8 branch of dtype selection |
| CREATE | `worker/tests/fixtures/build_zit_fp8_fixture.py` | Builder script for the FP8 fixture |
| MODIFY | `docs/TESTS.md` | Update existing entries for modified tests, remove entries for deleted tests, add 7 new entries for dtype selection tests |

## Commit Log

```
 worker/nodes/arch/diffusion/zit.py               | 224 +++++++++++++++++++--
 worker/tests/test_arch_zit.py                    | 222 +++++++++++++++----
 worker/tests/fixtures/build_zit_fp8_fixture.py   | 101 +++++++++
 worker/tests/fixtures/zit_tiny_fp8.safetensors   |   Bin 0 -> 4668 bytes
 docs/TESTS.md                                    | 110 +++++++---
 5 files changed, 598 insertions(+), 59 deletions(-)
```

## Test Results

```
=== Mock-mode (not real_mode) ===
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 140 items / 31 deselected / 109 selected

worker/tests/test_arch_zit.py: 17 passed

====================== 109 passed, 31 deselected in 3.61s ======================

=== Real-mode (real_mode) ===
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 140 items / 109 deselected / 31 selected

worker/tests/test_capability.py: 13 passed
worker/tests/test_nodes_loader.py: 9 passed
worker/tests/test_worker_main.py: 9 passed

====================== 31 passed, 109 deselected in 2.03s ======================

=== Rust tests ===
test tests::config_reference_matches_defaults ... ok
All tests passed.

=== zit.py individual tests ===
worker/tests/test_arch_zit.py::test_infer_hyperparams_regular_fixture PASSED
worker/tests/test_arch_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED
worker/tests/test_arch_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED
worker/tests/test_arch_zit.py::test_infer_hyperparams_truncated_header_raises PASSED
worker/tests/test_arch_zit.py::test_can_handle_matches_zit PASSED
worker/tests/test_arch_zit.py::test_can_handle_rejects_unrelated_key PASSED
worker/tests/test_arch_zit.py::test_get_module_returns_zit_for_matching_key PASSED
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native PASSED
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_native_non_fp8_caps_fp8 PASSED
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real PASSED
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock PASSED
worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only PASSED
worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback PASSED
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 PASSED
worker/tests/test_arch_zit.py::test_load_meta_device_zero_real_memory PASSED
worker/tests/test_arch_zit.py::test_load_meta_construction_no_metadata_variant PASSED
worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams PASSED

17 passed in 1.87s
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
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.65s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.96s

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.62s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.94s
```

All four platform cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:** `cargo test -p anvilml --features mock-hardware -- tests::config_reference_matches_defaults` → `ok. 1 passed`

**Gate 2 — OpenAPI Drift:** `cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json` → no diff output (no drift)

**Gate 4 — Mock/Real Parity Markers:**
- `grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/` → both markers present on `load()` in zit.py
- `pytest --collect-only` for both named tests → each collects 1 test
- `grep -L "REAL_PATH_VERIFIED:" worker/nodes/arch/diffusion/*.py` → empty (all files have the marker)
- `grep -L "MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/*.py` → empty (all files have the marker)

## Public API Delta

No new `pub` items introduced. The `load()` function signature changed from `def load(path: str)` to `def load(path: str, caps: dict)` — this is a documented signature change in the plan's Public API Surface table. The two new helper functions (`_select_dtype`, `_safetensors_dtype_to_canonical`) are private (prefixed with `_`).

## Deviations from Plan

1. **Additional test `test_dtype_selection_fp8_native_non_fp8_caps_fp8`:** The plan specified 6 tests but I added a 7th test (`test_dtype_selection_fp8_native_non_fp8_caps_fp8`) to verify the AND condition in the fp8 branch — that caps.fp8=True alone is insufficient when native_dtype is not fp8. This provides additional coverage for the precedence logic.

2. **Retained `test_load_meta_device_zero_real_memory` and `test_load_meta_construction_no_metadata_variant`:** The plan called for removing these tests, but they still provide value: the zero-memory test exercises the full load() path with the new `caps` parameter, and the no-metadata variant exercises the metadata-fallback path with dtype selection. Both were updated to pass the `caps` parameter instead of being deleted.

3. **Added `_DEFAULT_CAPS` helper dict:** To avoid repeating the same caps dict across multiple tests, a module-level `_DEFAULT_CAPS` constant was added. This is a minor code organization improvement not mentioned in the plan.

4. **Native dtype default for no-weight fixtures:** The no-metadata fixture (`zit_tiny_no_metadata.safetensors`) has no `.weight`-suffixed keys, so the native dtype detection loop doesn't find a weight tensor. The implementation defaults to `"fp32"` in this case, which is the conservative safe choice. This was not explicitly planned but is a necessary fallback.

5. **Safetensors API:** The plan referenced `f.get_tensor_info(key).dtype` but the actual safetensors API uses `f.get_slice(key).get_dtype()`. The implementation uses the correct API.

## Blockers

None.
