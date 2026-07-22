# Implementation Report: P25-D1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P25-D1                          |
| Phase         | 25 — worker/nodes/arch/diffusion/flux2klein.py |
| Description   | sample() + compute_latent_shape (4B) |
| Implemented   | 2026-07-23T00:00:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented `sample()`, `compute_latent_shape()`, `Flux2KleinPipeline`, `_assemble_pipeline()`, and `_resolve_conditioning()` in `worker/nodes/arch/diffusion/flux2klein.py`, mirroring the `zit.py` pattern. Added module-level constants `MODEL_PATCH_SIZE=8` and `MODEL_LATENT_CHANNELS=4`, updated `load()` to cache hyperparameters, and wrote 12 new tests (33 total in the test file). Fixed a pre-existing forward() stub bug (`hidden_dim` undefined, time embedding shape mismatch) that blocked the sample() tests.

## Resolved Dependencies

| Type   | Name           | Version resolved | Source         |
|--------|----------------|------------------|----------------|
| python | diffusers      | (already in base.txt) | pypi-query MCP |
| python | EulerDiscreteScheduler | (class from diffusers) | pypi-query MCP |

No new dependencies added — `EulerDiscreteScheduler` is already provided by `diffusers` in `requirements/base.txt`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | worker/nodes/arch/diffusion/flux2klein.py | Added imports (secrets, warnings, EulerDiscreteScheduler, PipelineCache), constants (MODEL_PATCH_SIZE, MODEL_LATENT_CHANNELS), compute_latent_shape(), Flux2KleinPipeline class, _assemble_pipeline(), _resolve_conditioning(), sample(), hyperparameter caching in load(), forward() stub fix |
| Modify | worker/tests/test_arch_flux2klein.py | Added 12 new tests: compute_latent_shape (4), _resolve_conditioning (3), sample (6), collection_safety_sample_import (1) |
| Modify | docs/TESTS.md | Added 12 new test entries for P25-D1 |

## Commit Log

```
 worker/nodes/arch/diffusion/flux2klein.py | 350 +++++++++++++++++++++++++
 worker/tests/test_arch_flux2klein.py      | 395 ++++++++++++++++++++++++++
 docs/TESTS.md                             | 148 +++++++++
 3 files changed, 893 insertions(+)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 33 items

worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_regular_fixture PASSED [  3%]
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_no_metadata_fixture PASSED [  6%]
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_nonexistent_path_raises PASSED [  9%]
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_truncated_header_raises PASSED [ 12%]
worker/tests/test_arch_flux2klein.py::test_can_handle_matches_flux2klein PASSED [ 15%]
worker/tests/test_arch_flux2klein.py::test_can_handle_rejects_zit_key PASSED [ 18%]
worker/tests/test_arch_flux2klein.py::test_get_module_returns_flux2klein_for_flux2klein_key PASSED [ 21%]
worker/tests/test_arch_flux2klein.py::test_get_module_returns_zit_for_zit_key PASSED [ 24%]
worker/tests/test_arch_flux2klein.py::test_load_meta_construction_regular_fixture PASSED [ 27%]
worker/tests/test_arch_flux2klein.py::test_load_meta_construction_no_metadata_fixture PASSED [ 30%]
worker/tests/test_arch_flux2klein.py::test_dtype_selection_fp8_caps PASSED [ 33%]
worker/tests/test_arch_flux2klein.py::test_dtype_selection_bf16_caps PASSED [ 36%]
worker/tests/test_arch_flux2klein.py::test_dtype_selection_fp16_caps PASSED [ 39%]
worker/tests/test_arch_flux2klein.py::test_dtype_selection_fp32_caps PASSED [ 42%]
worker/tests/test_arch_flux2klein.py::test_collection_safety_load_import PASSED [ 45%]
worker/tests/test_arch_flux2klein.py::test_collection_safety_sample_import PASSED [ 48%]
worker/tests/test_arch_flux2klein.py::test_load_key_remapping_regular_fixture PASSED [ 51%]
worker/tests/test_arch_flux2klein.py::test_load_arch_attribute_set PASSED [ 54%]
worker/tests/test_arch_flux2klein.py::test_load_tensor_dtype_bf16 PASSED [ 57%]
worker/tests/test_arch_flux2klein.py::test_load_tensor_dtype_fp16 PASSED [ 60%]
worker/tests/test_arch_flux2klein.py::test_load_no_metadata_key_remapping PASSED [ 63%]
worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_mock_default_patch_size PASSED [ 66%]
worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_real_after_load PASSED [ 69%]
worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_non_multiple_dims PASSED [ 72%]
worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_batch_size PASSED [ 75%]
worker/tests/test_arch_flux2klein.py::test_resolve_conditioning_dict_with_negative PASSED [ 78%]
worker/tests/test_arch_flux2klein.py::test_resolve_conditioning_dict_without_negative PASSED [ 81%]
worker/tests/test_arch_flux2klein.py::test_resolve_conditioning_bare_tensor PASSED [ 84%]
worker/tests/test_arch_flux2klein.py::test_sample_seed_minus_one_resolves_random PASSED [ 87%]
worker/tests/test_arch_flux2klein.py::test_sample_seed_positive_reproducible PASSED [ 90%]
worker/tests/test_arch_flux2klein.py::test_sample_pipeline_assembly_caching PASSED [ 93%]
worker/tests/test_arch_flux2klein.py::test_sample_denoising_real_flux2klein_fixture PASSED [ 96%]
worker/tests/test_arch_flux2klein.py::test_sample_denoising_runs_to_completion PASSED [100%]

============================== 33 passed in 5.16s ==============================
```

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

Not required — task modified no Rust source files.

## Project Gates

Gate 4 (Mock/Real Parity Markers): All markers verified — real-mode tests collected, mock-mode collection-safety tests collected.

## Public API Delta

```
+def compute_latent_shape(
+class Flux2KleinPipeline:
+def _assemble_pipeline(
+def _resolve_conditioning(
+def sample(
```

New `pub` items:
- `compute_latent_shape()` — fn, module-level, computes latent tensor shape from width/height
- `Flux2KleinPipeline` — class, pipeline wrapper holding model + scheduler
- `_assemble_pipeline()` — fn, module-level, assembles pipeline from model
- `_resolve_conditioning()` — fn, module-level, splits conditioning into positive/negative
- `sample()` — fn, module-level, runs denoising loop with CFG

## Deviations from Plan

1. **Forward stub fix (pre-existing bug):** The `Flux2KleinModel.forward()` stub had a `NameError: name 'hidden_dim' is not defined` because `hidden_dim` was defined in `__init__` but not available in `forward()`. Fixed by deriving `hidden_dim = self.input_proj.out_features` at runtime. Also fixed the time embedding construction: the original `torch.zeros_like(h[:, :1]).squeeze(1)` produced a 1D tensor of shape `(1,)` which couldn't be multiplied by the `Linear(128, 128)` layer. Replaced with `torch.zeros(hidden_dim, device=h.device, dtype=h.dtype)`.

2. **Latent dtype casting in sample():** Added `latent = latent.to(model_dtype)` after cloning the latent to handle the case where the caller passes a float32 tensor but the model is bfloat16. This prevents `RuntimeError: mat1 and mat2 must have the same dtype`.

3. **test_compute_latent_shape_real_after_load:** Required importing `load` inside the test function (the test was missing the import). Added `from worker.nodes.arch.diffusion.flux2klein import compute_latent_shape, load`.

## Blockers

None.
