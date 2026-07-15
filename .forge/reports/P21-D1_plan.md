# Plan Report: P21-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-D1                                      |
| Phase       | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description | Runnable Proof: Sampler node denoises the ZiT fixture latent for real |
| Depends on  | P21-C2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-15T08:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Execute the phase's Runnable Proof: run the real-mode pytest suites for `test_arch_zit.py` and `test_nodes_sampler.py` against the ZiT fixture checkpoint, confirming the full real-mode chain succeeds end-to-end — load (Phase 20) → pipeline assembly (P21-B1) → denoising + seed resolution (P21-B2) → Sampler node's real branch dispatch (P21-C2) — with zero `NotImplementedError`, zero skips, and zero xfails. Additionally, fix stale dual-mode parity markers on `compute_latent_shape()` in `zit.py` that reference non-existent test names (a phase-closing audit finding from §9a.2).

## Scope

### In Scope
- Run `python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode` and record the literal output.
- Fix stale `REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED` markers on `compute_latent_shape()` in `worker/nodes/arch/diffusion/zit.py` to point to the actual existing test names.
- Verify the pytest command exits 0 with zero skips and zero xfails.

### Out of Scope
None. `defers_to (from JSON): []` — this task may not defer any scope.

## Existing Codebase Assessment

The codebase at the start of this task has all preceding phase 21 tasks completed (P21-A1 through P21-C2). The relevant source files are fully implemented:

- **`worker/nodes/arch/diffusion/zit.py`** contains all four fixed-contract methods: `load()` (Phase 20), `compute_latent_shape()` (P21-A1), `sample()` (P21-B1 + P21-B2), and `can_handle()` (P20-B2). The `sample()` function implements the full denoising loop with classifier-free guidance, pipeline caching under `f"{model_id}:pipeline"`, and seed resolution via `secrets.randbelow(2**63)` for `seed < 0`.
- **`worker/nodes/sampler.py`** contains the `Sampler` node with both mock and real branches. The real branch dispatches via `arch.diffusion.get_module(inputs["model"].arch).sample(...)`. Dual-mode parity markers (`REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED`) are present on `execute()`.
- **`worker/tests/test_arch_zit.py`** has 34+ tests covering `_infer_hyperparams`, `can_handle`, `load` (with dtype selection), `compute_latent_shape` (with ceiling division), and `sample` (pipeline caching, denoising loop step count, seed resolution). Tests marked `@pytest.mark.real_mode` exercise real torch-level code.
- **`worker/tests/test_nodes_sampler.py`** has 9 tests: class attributes, mock seed resolution, and 6 real-mode tests covering denoising, seed resolution, step count, cfg=1.0, and shape preservation.
- **`worker/tests/fixtures/zit_tiny.safetensors`** is a 99KB synthetic checkpoint with `hidden_dim=64`, `double_block_count=1`, `single_block_count=1`, `latent_channels=4`, `latent_height=4`, `latent_width=4`, `patch_size=4`, `arch="zit"`. This is the fixture used by all real-mode tests.

Established patterns:
- Tests import `torch` conditionally (guarded import) and use `@pytest.mark.real_mode` for torch-dependent tests.
- Pipeline cache cleanup is done in test `finally` blocks to avoid cross-test pollution.
- Real-mode tests load the fixture via `load(path, caps, device="cpu")` and pass the model to `sample()` or `Sampler.execute()`.
- Seed resolution: real mode uses `secrets.randbelow(2**63)` for `seed < 0`; mock mode uses deterministic `-1 → 0`.

Gap between design doc and current source: The `compute_latent_shape()` function in `zit.py` has stale `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers that reference non-existent test names (`test_compute_latent_shape_real_formula` and `test_compute_latent_shape_mock_formula`). The actual test names are `test_compute_latent_shape_real_after_load` / `test_compute_latent_shape_real_non_multiple_after_load` (real mode) and `test_compute_latent_shape_mock_exact_multiple` / `test_compute_latent_shape_mock_non_multiple` / `test_compute_latent_shape_mock_batch_scaling` (mock mode). This is a documentation/marker drift issue from P21-A1.

## Resolved Dependencies

None. This task does not introduce or reference any external crates or Python packages. It only runs existing tests against the already-installed `torch` (CPU wheel) and `diffusers` dependencies.

## Approach

### Step 1 — Fix stale dual-mode parity markers on `compute_latent_shape()`

The `compute_latent_shape()` function in `worker/nodes/arch/diffusion/zit.py` has two stale markers:

```
# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_real_formula
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_formula
```

Neither test name exists. Per `ANVILML_DESIGN.md §10.6` and `FORGE_AGENT_RULES.md §9a.2`, every marker must name a real, collectible test. Fix by updating to:

```
# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple
```

Rationale: `test_compute_latent_shape_real_after_load` is the primary real-mode test for `compute_latent_shape()` that exercises the post-load path (where `load()` has updated the module-level hyperparameters). `test_compute_latent_shape_mock_exact_multiple` is the primary mock-mode test that exercises the exact-multiple ceiling division path. These are the most representative tests for each mode.

### Step 2 — Run the real-mode pytest invocation

Execute the acceptance criterion command:

```bash
python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode
```

This exercises:
- **test_arch_zit.py real-mode tests** (all tests marked `@pytest.mark.real_mode`):
  - `test_dtype_selection_fp8_caps_and_native` — `_select_dtype` fp8 branch
  - `test_dtype_selection_fp8_native_non_fp8_caps_fp8` — fp8 AND condition
  - `test_dtype_selection_bf16_real` — bf16 selection via `load()`
  - `test_dtype_selection_fp16_only` — fp16 fallback
  - `test_dtype_selection_fp32_fallback` — fp32 universal fallback
  - `test_dtype_selection_fp8_beats_bf16` — fp8 priority
  - `test_load_meta_construction_then_materialize` — meta → real device
  - `test_load_no_metadata_construction_then_materialize` — no-metadata fixture
  - `test_load_raises_invalid_hyperparams` — error propagation
  - `test_load_real_zit_fixture` — full load end-to-end (REAL marker)
  - `test_load_mock_zit_fixture` — full load in mock-mode context (MOCK marker)
  - `test_load_no_metadata_real` — no-metadata load (REAL marker)
  - `test_load_no_metadata_mock` — no-metadata load in mock-mode (MOCK marker)
  - `test_load_tensors_materialized_on_device` — materialization verification
  - `test_compute_latent_shape_real_after_load` — post-load shape formula (REAL marker, fixed in Step 1)
  - `test_compute_latent_shape_real_non_multiple_after_load` — ceiling division after load (REAL marker, fixed in Step 1)
  - `test_sample_first_call_assembles_pipeline_mock` — pipeline cache assembly
  - `test_sample_second_call_reuses_cached_pipeline_mock` — pipeline cache reuse
  - `test_sample_different_model_id_gets_separate_pipeline` — separate pipelines
  - `test_sample_returns_tuple_with_tensor_and_seed` — return type verification
  - `test_sample_denoising_real_zit_fixture` — end-to-end denoising (REAL marker)
  - `test_sample_seed_minus_one_resolves_random` — seed resolution (MOCK marker)
  - `test_sample_explicit_seed_returned_unchanged` — seed passthrough (MOCK marker)
  - `test_sample_denoising_runs_for_steps` — forward call count
  - `test_sample_output_shape_dtype_matches_input_latent` — shape/dtype preservation
  - `test_sample_different_step_count_changes_iterations` — step scaling

- **test_nodes_sampler.py real-mode tests** (all tests marked `@pytest.mark.real_mode`):
  - `test_sampler_real_denoises_zit_fixture` — end-to-end Sampler real branch (REAL marker)
  - `test_sampler_real_seed_minus_one_resolves` — seed resolution via Sampler
  - `test_sampler_real_explicit_seed_unchanged` — explicit seed passthrough
  - `test_sampler_real_multiple_steps` — step count verification
  - `test_sampler_real_cfg_one_is_conditional_only` — cfg=1.0 path
  - `test_sampler_real_latent_shape_preserved` — shape preservation

### Step 3 — Record literal pytest output

Paste the full pytest terminal output into the implementation report's `## Runnable Proof Transcript` section per `FORGE_AGENT_RULES.md §5.14`.

### Step 4 — Verify acceptance

Confirm the command exits 0, with zero skips and zero xfails in this specific invocation.

## Public API Surface

None. This task does not introduce any new public items. It only fixes comment markers and runs tests.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/diffusion/zit.py` | Fix stale `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers on `compute_latent_shape()` (lines 400-401) to point to existing test names. |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_zit.py` | All `@pytest.mark.real_mode` tests (26 tests) | Full real-mode chain: load → pipeline assembly → denoising → seed resolution against ZiT fixture | `python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode` exits 0 |
| `worker/tests/test_nodes_sampler.py` | All `@pytest.mark.real_mode` tests (6 tests) | Sampler node real branch dispatches to zit.py's sample() with denoising, seed resolution, cfg handling, shape preservation | Same command above |

No new test files or test functions are created. The task runs the existing real-mode suites.

## CI Impact

No CI changes required. The task does not modify any CI workflow files, test markers, or test configuration. The existing `worker-linux-real` and `worker-windows-real` CI jobs already run `python -m pytest worker/tests -v -m real_mode`, which includes these same test files.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient. All code paths exercised by the real-mode tests are platform-neutral: torch CPU operations, safetensors header parsing, and diffusers scheduler step logic do not depend on OS-specific APIs.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch` CPU wheel is not installed in the venv, causing `ImportError` on collection of real-mode tests. | Medium | High | The venv should have `cpu-linux-agent.txt` or `cpu-runner-reqs.txt` installed. If not, install it before running the pytest command. The error message will be explicit (`ImportError: No module named 'torch'`). |
| `diffusers` `EulerDiscreteScheduler` default constructor has changed API in the installed version, causing `TypeError` during pipeline assembly in `sample()`. | Low | Medium | The `_assemble_pipeline()` function creates `EulerDiscreteScheduler()` with no arguments — this is the standard diffusers API that has been stable. If the installed version differs, the error will surface immediately in the first sample test. |
| Stale markers cause `pytest --collect-only` to fail for the named tests, but the actual tests still run fine. | N/A (already verified) | Low | The markers are comments only; they do not affect pytest collection or execution. The fix updates them to point to real tests for phase-closing compliance. |
| Test fixture `zit_tiny.safetensors` is corrupted or missing, causing all real-mode tests to fail with `FileNotFoundError`. | Low | High | Verify the fixture exists at `worker/tests/fixtures/zit_tiny.safetensors` (99KB) before running. The file was built by `build_zit_fixture.py` in Phase 20. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py -v -m real_mode` exits 0 with zero skips and zero xfails
- [ ] `grep -n "test_compute_latent_shape_real_formula\|test_compute_latent_shape_mock_formula" worker/nodes/arch/diffusion/zit.py` returns no output (stale markers removed)
- [ ] `grep -n "REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load" worker/nodes/arch/diffusion/zit.py` returns a match (correct REAL marker present)
- [ ] `grep -n "MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple" worker/nodes/arch/diffusion/zit.py` returns a match (correct MOCK marker present)
