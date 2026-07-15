# Implementation Report: P21-D1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P21-D1                                            |
| Phase         | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description   | Runnable Proof: Sampler node denoises the ZiT fixture latent for real |
| Implemented   | 2026-07-15T09:15:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Executed the phase's Runnable Proof: ran the real-mode pytest suites for `test_arch_zit.py` and `test_nodes_sampler.py` against the ZiT fixture checkpoint, confirming the full real-mode chain succeeds end-to-end with zero skips and zero xfails. Fixed stale dual-mode parity markers on `compute_latent_shape()` in `zit.py` that referenced non-existent test names. Also fixed a pre-existing test fixture bug in `test_nodes_sampler.py` where `_make_ctx()` passed a string `job_id` instead of raw 16-byte UUID bytes, which caused all 6 real-mode Sampler tests to fail with a `ValueError` when accessing `ctx.job_id_str`.

## Resolved Dependencies

None. This task does not introduce or reference any external crates or Python packages.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/diffusion/zit.py` | Fixed stale `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers on `compute_latent_shape()` (lines 400-401) to point to existing test names. |
| Modify | `worker/tests/test_nodes_sampler.py` | Added `import uuid`; fixed `_make_ctx()` to use `uuid.uuid4().bytes` instead of string `"test-job"` for `job_id`, resolving `ValueError: bytes is not a 16-char string` in all 6 real-mode Sampler tests. |

## Commit Log

```
 .forge/state/CURRENT_TASK.md               |  6 +++---
 .forge/state/state.json                    | 17 +++++++++--------
 worker/nodes/arch/diffusion/zit.py         |  4 ++--
 worker/tests/test_nodes_sampler.py         |  7 ++++++-
 4 files changed, 20 insertions(+), 14 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 51 items / 17 deselected / 34 selected

worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native PASSED [  2%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_native_non_fp8_caps_fp8 PASSED [  5%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real PASSED     [  8%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock PASSED     [ 11%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only PASSED     [ 14%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback PASSED [ 17%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 PASSED [ 20%]
worker/tests/test_arch_zit.py::test_load_meta_construction_then_materialize PASSED [ 23%]
worker/tests/test_arch_zit.py::test_load_no_metadata_construction_then_materialize PASSED [ 26%]
worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams PASSED [ 29%]
worker/tests/test_arch_zit.py::test_load_real_zit_fixture PASSED         [ 32%]
worker/tests/test_arch_zit.py::test_load_mock_zit_fixture PASSED         [ 35%]
worker/tests/test_arch_zit.py::test_load_no_metadata_real PASSED         [ 38%]
worker/tests/test_arch_zit.py::test_load_no_metadata_mock PASSED         [ 41%]
worker/tests/test_arch_zit.py::test_load_tensors_materialized_on_device PASSED [ 44%]
worker/tests/test_arch_zit.py::test_load_raises_on_invalid_path PASSED   [ 47%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load PASSED [ 50%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_non_multiple_after_load PASSED [ 52%]
worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock PASSED [ 55%]
worker/tests/test_arch_zit.py::test_sample_second_call_reuses_cached_pipeline_mock PASSED [ 58%]
worker/tests/test_arch_zit.py::test_sample_different_model_id_gets_separate_pipeline PASSED [ 61%]
worker/tests/test_arch_zit.py::test_sample_returns_tuple_with_tensor_and_seed PASSED [ 64%]
worker/tests/test_arch_zit.py::test_sample_denoising_real_zit_fixture PASSED [ 67%]
worker/tests/test_arch_zit.py::test_sample_seed_minus_one_resolves_random PASSED [ 70%]
worker/tests/test_arch_zit.py::test_sample_explicit_seed_returned_unchanged PASSED [ 73%]
worker/tests/test_arch_zit.py::test_sample_denoising_runs_for_steps PASSED [ 76%]
worker/tests/test_arch_zit.py::test_sample_output_shape_dtype_matches_input_latent PASSED [ 79%]
worker/tests/test_arch_zit.py::test_sample_different_step_count_changes_iterations PASSED [ 82%]
worker/tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture PASSED [ 85%]
worker/tests/test_nodes_sampler.py::test_sampler_real_seed_minus_one_resolves PASSED [ 88%]
worker/tests/test_nodes_sampler.py::test_sampler_real_explicit_seed_unchanged PASSED [ 91%]
worker/tests/test_nodes_sampler.py::test_sampler_real_multiple_steps PASSED [ 94%]
worker/tests/test_nodes_sampler.py::test_sampler_real_cfg_one_is_conditional_only PASSED [ 97%]
worker/tests/test_nodes_sampler.py::test_sampler_real_latent_shape_preserved PASSED [100%]

=============================== warnings summary ===============================
tests/test_arch_zit.py: 12 warnings
tests/test_nodes_sampler.py: 6 warnings
  /home/dryw/AnvilML/worker/.venv/lib/python3.12/site-packages/diffusers/schedulers/scheduling_euler_discrete.py:436: DeprecationWarning: __array__ implementation doesn't accept a copy keyword, so passing copy=False failed. __array__ must implement 'dtype' and 'copy' keyword arguments. To learn more, see the migration guide https://numpy.org/devdocs/numpy_2_0_migration_guide.html#adapting-to-changes-in-the-copy-keyword
    sigmas = np.array(((1 - self.alphas_cumprod) / self.alphas_cumprod) ** 0.5)

-- Docs: https://pytest.org/en/stable/how-to/capture.html
================ 34 passed, 17 deselected, 18 warnings in 6.29s ================
```

## Runnable Proof Transcript

The following is the verbatim terminal output of the Runnable Proof command — the literal acceptance criterion for this task:

```bash
$ worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode
```

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 51 items / 17 deselected / 34 selected

worker/tests/test_arch_zit.py::test_dtype_selection_fp8_caps_and_native PASSED [  2%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_native_non_fp8_caps_fp8 PASSED [  5%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real PASSED     [  8%]
worker/tests/test_arch_zit.py::test_dtype_selection_bf16_mock PASSED     [ 11%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp16_only PASSED     [ 14%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp32_fallback PASSED [ 17%]
worker/tests/test_arch_zit.py::test_dtype_selection_fp8_beats_bf16 PASSED [ 20%]
worker/tests/test_arch_zit.py::test_load_meta_construction_then_materialize PASSED [ 23%]
worker/tests/test_arch_zit.py::test_load_no_metadata_construction_then_materialize PASSED [ 26%]
worker/tests/test_arch_zit.py::test_load_raises_invalid_hyperparams PASSED [ 29%]
worker/tests/test_arch_zit.py::test_load_real_zit_fixture PASSED         [ 32%]
worker/tests/test_arch_zit.py::test_load_mock_zit_fixture PASSED         [ 35%]
worker/tests/test_arch_zit.py::test_load_no_metadata_real PASSED         [ 38%]
worker/tests/test_arch_zit.py::test_load_no_metadata_mock PASSED         [ 41%]
worker/tests/test_arch_zit.py::test_load_tensors_materialized_on_device PASSED [ 44%]
worker/tests/test_arch_zit.py::test_load_raises_on_invalid_path PASSED   [ 47%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load PASSED [ 50%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_real_non_multiple_after_load PASSED [ 52%]
worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock PASSED [ 55%]
worker/tests/test_arch_zit.py::test_sample_second_call_reuses_cached_pipeline_mock PASSED [ 58%]
worker/tests/test_arch_zit.py::test_sample_different_model_id_gets_separate_pipeline PASSED [ 61%]
worker/tests/test_arch_zit.py::test_sample_returns_tuple_with_tensor_and_seed PASSED [ 64%]
worker/tests/test_arch_zit.py::test_sample_denoising_real_zit_fixture PASSED [ 67%]
worker/tests/test_arch_zit.py::test_sample_seed_minus_one_resolves_random PASSED [ 70%]
worker/tests/test_arch_zit.py::test_sample_explicit_seed_returned_unchanged PASSED [ 73%]
worker/tests/test_arch_zit.py::test_sample_denoising_runs_for_steps PASSED [ 76%]
worker/tests/test_arch_zit.py::test_sample_output_shape_dtype_matches_input_latent PASSED [ 79%]
worker/tests/test_arch_zit.py::test_sample_different_step_count_changes_iterations PASSED [ 82%]
worker/tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture PASSED [ 85%]
worker/tests/test_nodes_sampler.py::test_sampler_real_seed_minus_one_resolves PASSED [ 88%]
worker/tests/test_nodes_sampler.py::test_sampler_real_explicit_seed_unchanged PASSED [ 91%]
worker/tests/test_nodes_sampler.py::test_sampler_real_multiple_steps PASSED [ 94%]
worker/tests/test_nodes_sampler.py::test_sampler_real_cfg_one_is_conditional_only PASSED [ 97%]
worker/tests/test_nodes_sampler.py::test_sampler_real_latent_shape_preserved PASSED [100%]

=============================== warnings summary ===============================
tests/test_arch_zit.py: 12 warnings
tests/test_nodes_sampler.py: 6 warnings
  /home/dryw/AnvilML/worker/.venv/lib/python3.12/site-packages/diffusers/schedulers/scheduling_euler_discrete.py:436: DeprecationWarning: __array__ implementation doesn't accept a copy keyword, so passing copy=False failed. __array__ must implement 'dtype' and 'copy' keyword arguments. To learn more, see the migration guide https://numpy.org/devdocs/numpy_2_0_migration_guide.html#adapting-to-changes-in-the-copy-keyword
    sigmas = np.array(((1 - self.alphas_cumprod) / self.alphas_cumprod) ** 0.5)

-- Docs: https://pytest.org/en/stable/how-to/capture.html
================ 34 passed, 17 deselected, 18 warnings in 6.29s ================
```

Exit code: 0. Zero skips. Zero xfails. All 34 real-mode tests (28 from test_arch_zit.py + 6 from test_nodes_sampler.py) passed, confirming the full real-mode chain: load (Phase 20) → pipeline assembly (P21-B1) → denoising + seed resolution (P21-B2) → Sampler node's real branch dispatch (P21-C2) succeeds end-to-end with zero NotImplementedError anywhere.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, all files formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.27s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.57s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.76s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; filtered out

# Gate 3 — Node Parity
test_parity.py not present (not applicable — file does not exist in this repo)

# Gate 4 — Mock/Real Parity Markers
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py (grep -v __init__ | grep -v base.py) → empty
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py (grep -v __init__ | grep -v base.py) → empty
All files with execute()/load()/sample()/decode()/compute_latent_shape() have both markers.
```

## Public API Delta

```
(no new pub items introduced)
```

## Deviations from Plan

- **Fixed pre-existing test fixture bug in `worker/tests/test_nodes_sampler.py`**: The `_make_ctx()` function passed `job_id="test-job"` (a string) to `NodeContext`, but `NodeContext.job_id_str` constructs a UUID via `uuid.UUID(bytes=self.job_id)`, which requires raw 16-byte UUID bytes. This caused all 6 real-mode Sampler tests to fail with `ValueError: bytes is not a 16-char string`. Fixed by importing `uuid` and using `uuid.uuid4().bytes` as the job_id. This was a pre-existing defect discovered during the Runnable Proof execution — it is a test-fix, not a scope deviation.
- **No version bump performed**: The task only modified Python files (`zit.py` and `test_nodes_sampler.py`), not any Rust crate source files. Per ENVIRONMENT.md §12, version bumps apply only to crates whose source files were modified.

## Blockers

None.
