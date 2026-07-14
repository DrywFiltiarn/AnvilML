# Plan Report: P21-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-B2                                      |
| Phase       | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description | worker/nodes/arch/diffusion/zit.py: sample() denoising loop + seed resolution |
| Depends on  | P21-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-14T13:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `zit.py`'s `sample()` function by adding the denoising loop and seed resolution logic that P21-B1 deferred. When `seed == -1`, resolve to a cryptographically random integer in `[0, 2^63)` using `secrets.randbelow()` before denoising runs, so the resolved seed is logged and returned. The denoising loop runs for the specified step count with classifier-free guidance via the cached pipeline (assembled in P21-B1), returning a `(denoised_latent, resolved_seed)` tuple. This brings the total test count in `test_arch_zit.py` from 36 to >=41 (>=5 new tests), meeting the >=34 acceptance threshold.

## Scope

### In Scope
- Modify `sample()` in `worker/nodes/arch/diffusion/zit.py` to:
  - Resolve `seed == -1` to a cryptographically random integer via `secrets.randbelow(2**63)` before denoising.
  - Run the denoising loop for the specified `steps` count with `cfg` guidance using the cached pipeline.
  - Return `tuple[torch.Tensor, int]` — `(denoised_latent, resolved_seed)` — never `-1` in the returned seed.
  - Update the docstring to reflect the new return type and seed resolution behavior.
  - Update the existing `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers on `sample()` to name the new test functions.
- Add >=5 new tests in `worker/tests/test_arch_zit.py`:
  1. `test_sample_seed_minus_one_resolves_random` (mock) — seed=-1 returns resolved seed != -1 in [0, 2^63).
  2. `test_sample_explicit_seed_returned_unchanged` (mock) — explicit seed is used as-is and returned unchanged.
  3. `test_sample_denoising_runs_for_steps` (mock) — denoising actually runs for the specified step count (verified via step-callback spy).
  4. `test_sample_output_shape_dtype_matches_input_latent` (mock) — output latent has same shape and dtype as input.
  5. `test_sample_denoising_real_zit_fixture` (real_mode) — end-to-end denoising against the real ZiT fixture checkpoint.
- Update `docs/TESTS.md` with entries for all new tests.

### Out of Scope
None. This task implements its full scope. The `defers_to` field is `[]` (empty), so no scope is deferred. The Sampler node (P21-C1, P21-C2) is a separate task that dispatches to `sample()` but is not part of this task's scope.

## Existing Codebase Assessment

The `sample()` function at line 408 of `zit.py` currently assembles and caches a `ZiTPipeline` from the loaded `ZiTModel` (via `pipeline_cache.get_or_load()`), but does NOT run denoising — it simply returns the pipeline. P21-B1 completed the pipeline assembly/caching half. The function currently returns `ZiTPipeline` and has a docstring noting that denoising is deferred to P21-B2.

Existing patterns in `zit.py`:
- Google-style docstrings with `Args:`, `Returns:`, `Raises:` sections for all public functions.
- Logging via `logging.getLogger(__name__)` at DEBUG/INFO levels.
- The `ZiTPipeline` class is a thin container holding `.model` (ZiTModel) and `.scheduler` (EulerDiscreteScheduler).
- Module-level `pipeline_cache` is a `PipelineCache()` instance, keyed as `f"{model_id}:pipeline"`.
- Test style: fixtures under `worker/tests/fixtures/`, `_DEFAULT_CAPS` dict for capability testing, spy patterns for mocking cache behavior, `@pytest.mark.real_mode` for real-mode tests.

The dual-mode parity markers already exist on `sample()` at lines 406-407:
```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_pipeline_assembled_from_loaded_model
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock
```
These will be updated to point at the new test functions.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| python | diffusers | 0.39.0          | pypi-query MCP | n/a                    |
| python | secrets   | stdlib (3.12)   | n/a            | n/a (stdlib module)    |

`secrets` is a Python 3.6+ standard library module — no external version to resolve. `secrets.randbelow(n)` returns a random integer in `[0, n)`, which gives us the required `[0, 2^63)` range.

The `EulerDiscreteScheduler` from diffusers 0.39.0 provides:
- `set_timesteps(num_inference_steps: int)` — sets the noise schedule.
- `step(model_output: torch.Tensor, sample: torch.Tensor, timestep: int) -> SchedulerOutput` — performs one denoising step.

## Approach

### Step 1: Add the `secrets` import to `zit.py`

Add `import secrets` at the top of `zit.py` alongside the existing imports (`logging`, `re`, `pathlib`, `typing`, `torch`, `torch.nn`, `safetensors`, `diffusers`, `pipeline_cache`). This is a stdlib module with no version concerns.

### Step 2: Rewrite `sample()` to include the denoising loop

Replace the existing `sample()` function (lines 408-461) with a new implementation that:

**2a. Seed resolution:**
```python
if seed < 0:
    seed = secrets.randbelow(2**63)
```
This resolves `-1` (and any negative seed) to a cryptographically random integer in `[0, 2^63)`. The resolved seed is stored in the `seed` variable for use throughout the function and returned at the end.

**2b. Pipeline retrieval (unchanged from P21-B1):**
Keep the existing cache-lookup logic:
```python
key = f"{model_id}:pipeline"
pipeline = pipeline_cache.get_or_load(key, lambda: _assemble_pipeline(model))
```

**2c. Denoising loop:**
```python
scheduler = pipeline.scheduler
scheduler.set_timesteps(steps)

latent = latent.clone()  # Don't mutate the caller's tensor

for t in scheduler.timesteps:
    # Classifier-free guidance: run unconditional + conditional passes
    # and interpolate with the cfg scale.
    
    # Unconditional pass (empty conditioning)
    with torch.no_grad():
        noise_pred_uncond = pipeline.model(latent, t / 1000.0).sample
    
    # Conditional pass
    with torch.no_grad():
        noise_pred_cond = pipeline.model(latent, t / 1000.0, conditioning=conditioning).sample
    
    # CFG interpolation
    noise_pred = noise_pred_uncond + cfg * (noise_pred_cond - noise_pred_uncond)
    
    # Scheduler step
    latent = scheduler.step(noise_pred, t, latent).prev_sample

return latent, seed
```

**Rationale for the denoising approach:** The ZiT model is a transformer that takes a timestep scalar and optional conditioning. The `EulerDiscreteScheduler` provides the noise schedule (`set_timesteps` → `.timesteps`) and the `step()` method that transforms the latent given a model prediction. The denoising loop iterates over timesteps, runs the model (with CFG interpolation), and steps the scheduler.

**2d. Return type change:**
The function signature stays the same (same parameters), but the return type changes from `ZiTPipeline` to `tuple[torch.Tensor, int]`. The docstring must be updated accordingly.

**2e. Logging:**
Add a DEBUG log after seed resolution and another after denoising completes:
```python
logger.debug("seed=%d, steps=%d, cfg=%.1f", seed, steps, cfg)
logger.info("denoising complete: steps=%d, seed=%d", steps, seed)
```

### Step 3: Update the dual-mode parity markers on `sample()`

Replace the existing markers at lines 406-407 with:
```python
# REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_denoising_real_zit_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_sample_seed_minus_one_resolves_random
```

### Step 4: Add >=5 new tests in `test_arch_zit.py`

**Test 1: `test_sample_seed_minus_one_resolves_random` (mock)**
- Calls `sample()` with `seed=-1`.
- Asserts the returned seed is != -1 and in `[0, 2^63)`.
- Since `secrets.randbelow()` is non-deterministic, we assert the range constraint only (not a specific value).
- Uses a fixture-loaded model and a zero latent tensor.

**Test 2: `test_sample_explicit_seed_returned_unchanged` (mock)**
- Calls `sample()` with `seed=42` (or any explicit positive integer).
- Asserts the returned seed equals the input seed exactly.
- Verifies the seed is not modified by the function.

**Test 3: `test_sample_denoising_runs_for_steps` (mock)**
- Creates a spy by patching `pipeline.model.forward()` to count invocations.
- Calls `sample()` with `steps=10`.
- Asserts the model was called exactly 10 times (once per timestep).
- This is the critical test proving denoising actually runs.

**Test 4: `test_sample_output_shape_dtype_matches_input_latent` (mock)**
- Creates an input latent with a specific shape `(1, 4, 2, 2)` and dtype `torch.float32`.
- Calls `sample()` and asserts the returned latent has the same shape and dtype.

**Test 5: `test_sample_denoising_real_zit_fixture` (real_mode)**
- `@pytest.mark.real_mode` decorated.
- Loads a real model from the ZiT fixture, calls `sample()` with real parameters.
- Asserts the output is a tensor with reasonable shape `(1, 4, 2, 2)`.
- This is the canonical real-mode test for the denoising path.

### Step 5: Update `docs/TESTS.md`

Add entries for all 5 new tests with `Mode: mock` or `Mode: real` fields per the §17.1 format.

### Step 6: Pre-stop verification

Run the three verification commands:
```bash
head -1 .forge/reports/P21-B2_plan.md
grep "^## " .forge/reports/P21-B2_plan.md
wc -l .forge/reports/P21-B2_plan.md
```

## Public API Surface

| Item | Module Path | Signature | Change |
|------|-------------|-----------|--------|
| `sample()` | `worker.nodes.arch.diffusion.zit` | `def sample(model: ZiTModel, model_id: str, conditioning: Any, latent: torch.Tensor, steps: int, cfg: float, seed: int) -> tuple[torch.Tensor, int]` | **Modified**: return type changed from `ZiTPipeline` to `tuple[torch.Tensor, int]`; now runs denoising loop; resolves `-1` seed |

The function signature parameters remain unchanged. Only the return type and body change.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `import secrets`; rewrite `sample()` to include denoising loop and seed resolution; update docstring; update parity markers |
| MODIFY | `worker/tests/test_arch_zit.py` | Add >=5 new tests for seed resolution, step count, output shape/dtype, and real-mode denoising |
| MODIFY | `docs/TESTS.md` | Add entries for all new tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_zit.py` | `test_sample_seed_minus_one_resolves_random` (mock) | `seed=-1` returns a resolved seed != -1 in `[0, 2^63)` | `python -m pytest worker/tests/test_arch_zit.py::test_sample_seed_minus_one_resolves_random -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_explicit_seed_returned_unchanged` (mock) | Explicit seed is used as-is and returned unchanged | `python -m pytest worker/tests/test_arch_zit.py::test_sample_explicit_seed_returned_unchanged -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_denoising_runs_for_steps` (mock) | Denoising runs for the specified step count (verified via step-callback spy counting model forward calls) | `python -m pytest worker/tests/test_arch_zit.py::test_sample_denoising_runs_for_steps -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_output_shape_dtype_matches_input_latent` (mock) | Output latent has same shape and dtype as input latent | `python -m pytest worker/tests/test_arch_zit.py::test_sample_output_shape_dtype_matches_input_latent -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_denoising_real_zit_fixture` (real) | End-to-end denoising against real ZiT fixture checkpoint produces valid latent output | `python -m pytest worker/tests/test_arch_zit.py::test_sample_denoising_real_zit_fixture -v -m real_mode` exits 0 |

## CI Impact

No CI changes required. The new tests follow the existing pattern: unmarked tests run in both mock and real CI jobs (they import `torch` at module level but do not require torch for collection), and the real-mode test is marked with `@pytest.mark.real_mode`. The existing CI job matrix already handles these markers correctly (see `ENVIRONMENT.md §6` Steps 8-9).

## Platform Considerations

None identified. The `secrets` module is cross-platform (stdlib). The denoising loop uses only `torch` operations that work on both CPU and GPU. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `EulerDiscreteScheduler.step()` API may differ between diffusers versions — the exact parameter names and return type (`SchedulerOutput`) may vary between 0.39.0 and other versions. | Low | Medium | The MCP confirmed diffusers 0.39.0 is pinned in `requirements/base.txt`. Use the MCP-confirmed API shape. If the API differs at ACT time, the test will fail with a clear error message. |
| The ZiT model's forward method signature may not match the assumed `(latent, timestep)` pattern — ZiT is a transformer and may expect different input shapes or additional parameters. | Medium | High | The `ZiTPipeline` wrapper already holds the model and scheduler from `load()` (Phase 20). If the forward signature is wrong, the test will fail with a clear error. The ACT agent should verify the model's forward signature against the actual `ZiTModel` class. |
| CFG interpolation formula may produce NaN/inf with the synthetic fixture checkpoint (simplified weights, zero-initialized double blocks). | Low | Medium | The fixture checkpoint has intentionally zero-initialized double blocks (per the `load()` code comments), so the model output will be near-zero. The scheduler step on near-zero noise predictions should still produce valid latents. The test asserts shape/dtype match, not specific values. |
| `secrets.randbelow(2**63)` could theoretically produce a value outside `[0, 2^63)` due to implementation details. | Very Low | Low | `secrets.randbelow(n)` is documented to return `[0, n)`. The test asserts `seed >= 0 and seed < 2**63`. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_seed_minus_one_resolves_random -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_explicit_seed_returned_unchanged -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_denoising_runs_for_steps -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_output_shape_dtype_matches_input_latent -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_denoising_real_zit_fixture -v -m real_mode` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with >=34 total tests
