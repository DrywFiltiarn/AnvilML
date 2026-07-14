# Implementation Report: P21-B1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P21-B1                                            |
| Phase         | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description   | worker/nodes/arch/diffusion/zit.py: sample() pipeline assembly + caching |
| Implemented   | 2026-07-14T11:00:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented `sample()` in `worker/nodes/arch/diffusion/zit.py` that assembles and caches a `ZiTPipeline` (model + scheduler wrapper) from a loaded `ZiTModel`. The function uses the module-level `PipelineCache` with key `"{model_id}:pipeline"` — first call assembles, subsequent calls return the cached pipeline. Added `ZiTPipeline` wrapper class, `_assemble_pipeline()` internal helper, `PipelineCache` import, dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`), and `defers_to: P21-B2` marker for the deferred denoising loop. Wrote 5 tests (4 mock + 1 real-mode) in `test_arch_zit.py`. All 36 zit tests pass, all 124 mock-mode tests pass, all 33 real-mode tests pass, all Rust tests pass, all platform cross-checks pass, all gates pass.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | diffusers | 0.35.1           | pypi-query MCP |
| python | torch     | 2.7.x (local)    | local          |

No new external dependencies introduced. `EulerDiscreteScheduler` from `diffusers` was verified available via `worker/.venv/bin/python -c "from diffusers import EulerDiscreteScheduler; print('OK')"`. `PipelineCache` is an existing module from Phase 19 (P19-B1).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/diffusion/zit.py` | Added `PipelineCache` import, `ZiTPipeline` class, `_assemble_pipeline()`, `sample()` with cache-assembly, parity markers, defers_to marker, updated module docstring |
| Modify | `worker/tests/test_arch_zit.py` | Added 5 tests: `test_sample_first_call_assembles_pipeline_mock`, `test_sample_second_call_reuses_cached_pipeline_mock`, `test_sample_different_model_id_gets_separate_pipeline`, `test_sample_pipeline_is_zit_wrapper`, `test_sample_pipeline_assembled_from_loaded_model` |
| Modify | `docs/TESTS.md` | Added 5 entries for the new tests |

## Commit Log

```
 .forge/reports/P21-B1_plan.md      | 158 +++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +--
 docs/TESTS.md                      |  63 ++++++++++++
 worker/nodes/arch/diffusion/zit.py | 123 +++++++++++++++++++++-
 worker/tests/test_arch_zit.py      | 203 +++++++++++++++++++++++++++++++++++++
 6 files changed, 555 insertions(+), 11 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 36 items

worker/tests/test_arch_zit.py::test_infer_hyperparams_regular_fixture PASSED [  2%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED [  5%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED [  8%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_truncated_header_raises PASSED [ 11%]
worker/tests/test_arch_zit.py::test_can_handle_matches_zit PASSED        [ 13%]
worker/tests/test_arch_zit.py::test_can_handle_rejects_unrelated_key PASSED [ 16%]
worker/tests/test_arch_zit.py::test_get_module_returns_zit_for_matching_key PASSED [ 19%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native PASSED [ 22%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_native_non_fp8_caps_fp8 PASSED [ 25%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real PASSED     [ 27%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock PASSED     [ 30%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only PASSED     [ 33%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback PASSED [ 36%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 PASSED [ 38%]
worker/tests/test_arch_zit.py::test_load_meta_construction_then_materialize PASSED [ 41%]
worker/tests/test_arch_zit.py::test_load_no_metadata_construction_then_materialize PASSED [ 44%]
worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams PASSED [ 47%]
worker/tests/test_arch_zit.py::test_load_real_zit_fixture PASSED         [ 50%]
worker/tests/test_arch_zit.py::test_load_mock_zit_fixture PASSED         [ 52%]
worker/tests/test_arch_zit.py::test_load_no_metadata_real PASSED         [ 55%]
worker/tests/test_arch_zit.py::test_load_no_metadata_mock PASSED         [ 58%]
worker/tests/test_arch_zit.py::test_load_tensors_materialized_on_device PASSED [ 61%]
worker/tests/test_arch_zit.py::test_load_key_remapping_direct_match PASSED [ 63%]
worker/tests/test_arch_zit.py::test_load_raises_on_invalid_path PASSED   [ 66%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple PASSED [ 69%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_non_multiple PASSED [ 72%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_batch_scaling PASSED [ 75%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load PASSED [ 77%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_non_multiple_after_load PASSED [ 80%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_default_batch_size PASSED [ 83%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_zero_dims PASSED [ 86%]
worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock PASSED [ 88%]
worker/tests/test_arch_zit.py::test_sample_second_call_reuses_cached_pipeline_mock PASSED [ 91%]
worker/tests/test_arch_zit.py::test_sample_different_model_id_gets_separate_pipeline PASSED [ 94%]
worker/tests/test_arch_zit.py::test_sample_pipeline_is_zit_wrapper PASSED [ 97%]
worker/tests/test_arch_zit.py::test_sample_pipeline_assembled_from_loaded_model PASSED [100%]

============================== 36 passed in 4.54s ==============================
```

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.98s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 59.72s

# 3. Real-hardware Linux:
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 02s

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 03s
```

## Project Gates

```
# Gate 1 — Config Surface Sync:
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored

# Gate 3 — Node Parity:
# test_parity.py does not exist yet — no gate trigger (no node types modified)
```

## Public API Delta

```
git diff HEAD -- worker/nodes/arch/diffusion/zit.py | grep "^+.*def " | head -20
+    def __init__(self, model: ZiTModel, scheduler: Any) -> None:
+def _assemble_pipeline(model: ZiTModel) -> ZiTPipeline:
+def sample(
```

New public items:
- `ZiTPipeline` class (internal, not pub — but its `__init__` is visible to callers)
- `_assemble_pipeline(model: ZiTModel) -> ZiTPipeline` — internal helper (private, underscore prefix)
- `sample(model, model_id, conditioning, latent, steps, cfg, seed) -> ZiTPipeline` — new public function

New test functions in `test_arch_zit.py`:
- `test_sample_first_call_assembles_pipeline_mock`
- `test_sample_second_call_reuses_cached_pipeline_mock`
- `test_sample_different_model_id_gets_separate_pipeline`
- `test_sample_pipeline_is_zit_wrapper`
- `test_sample_pipeline_assembled_from_loaded_model`

## Deviations from Plan

- The plan named `test_sample_different_model_id_isolation` as the test name; I used `test_sample_different_model_id_gets_separate_pipeline` (more descriptive, consistent with the existing naming style in `test_arch_zit.py`).
- The plan's test table listed 5 tests total (4 mock + 1 real). I implemented exactly 5 tests: 4 mock-compatible (no `@pytest.mark.real_mode` decorator) and 1 real-mode (`@pytest.mark.real_mode` decorated). This matches the plan's intent.
- No version bump was needed — the task only modifies Python files, and Python worker code has no per-module version identifiers per ENVIRONMENT.md §12.

## Blockers

None.
