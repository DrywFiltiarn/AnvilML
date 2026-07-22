# Plan Report: P25-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P25-D1                                      |
| Phase       | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE |
| Description | worker/nodes/arch/diffusion/flux2klein.py: sample() + compute_latent_shape (4B) |
| Depends on  | P25-C2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-22T23:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `worker/nodes/arch/diffusion/flux2klein.py` by implementing its two remaining fixed-name methods: `compute_latent_shape()` (Flux 2 Klein's architecture-specific patch-packing formula) and `sample()` (pipeline assembly/caching via `pipeline_cache.get_or_load()`, seed resolution via `secrets`, and the denoising loop with classifier-free guidance). This brings flux2klein.py to feature parity with zit.py's sampling contract. The acceptance criterion is `>=8` new tests in `test_arch_flux2klein.py` (total `>=25`), all passing.

## Scope

### In Scope
- `worker/nodes/arch/diffusion/flux2klein.py`: add `compute_latent_shape(width, height, batch_size=1)` and `sample(model, model_id, conditioning, latent, steps, cfg, seed)`, plus the internal helper `_assemble_pipeline()` and `Flux2KleinPipeline` class.
- `worker/tests/test_arch_flux2klein.py`: add `>=8` new tests covering `compute_latent_shape` (exact outputs for multiple inputs), `sample()` pipeline assembly/caching, seed=-1 resolution, and denoising against the 4B fixture.
- Module-level parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) on `compute_latent_shape()` and `sample()`.
- Mandatory logging: INFO for denoising complete, DEBUG for pipeline assembly and seed resolution.
- Import guard update: add `EulerDiscreteScheduler` to the torch-import block (currently missing from flux2klein.py but present in zit.py).

### Out of Scope
None. `defers_to (from JSON): []` — this task has an empty defers_to array and must implement its full scope.

## Existing Codebase Assessment

**What already exists:** `flux2klein.py` (1067 lines) already implements steps 1–4 of the four-step loading contract: `_infer_hyperparams()`, `can_handle()`, `Flux2KleinModel` meta-construction, `_select_dtype()`, `_build_key_remapping()`, and `load()` with full weight loading. All imports are guarded with `try/except ImportError`. Parity markers exist on `load()`. The fixture files (`flux2klein4b_tiny.safetensors`, `flux2klein4b_tiny_no_metadata.safetensors`) are already built by P25-A1, with `patch_size=8`, `latent_channels=4`, `hidden_dim=128`, `double_block_count=1`, `single_block_count=1`. The existing test file has 17 tests across `_infer_hyperparams`, `can_handle`, dispatch, dtype selection, and weight loading. `pipeline_cache.py` provides `PipelineCache` with `get_or_load(key, loader_fn)` — an LRU cache keyed by string. `zit.py` (1300 lines) provides the established reference implementation for the same contract shape.

**Established patterns:** (1) Module-level constants for patch_size/latent_channels updated by `load()`; (2) `compute_latent_shape()` uses ceiling division `(x + patch_size - 1) // patch_size`; (3) `sample()` uses `pipeline_cache.get_or_load()` keyed as `"{model_id}:pipeline"`; (4) seed=-1 resolved via `secrets.randbelow(2**63)`; (5) `_resolve_conditioning()` helper splits conditioning into (positive, negative) tuple; (6) Denoising loop: `scheduler.scale_model_input(latent, t)` → unconditional pass → conditional pass → CFG interpolation → `scheduler.step()`. (7) Dual-mode parity markers on every arch-module function. (8) Google-style docstrings on all public functions. (9) Import guard with `try/except ImportError` for torch-dependent imports.

**Gap between design doc and source:** `flux2klein.py` currently does NOT import `EulerDiscreteScheduler` from diffusers (zit.py does — the import guard block in flux2klein.py only guards `torch`, `torch.nn`, and `load_file`). This must be added for `sample()` to work. The `import secrets` module is also absent (present in zit.py but not flux2klein.py).

## Resolved Dependencies

| Type   | Name          | Version verified | MCP source  | Feature flags confirmed |
|--------|---------------|-----------------|-------------|------------------------|
| python | diffusers     | 0.39.0 (project's existing) | N/A — already in worker/requirements/base.txt | EulerDiscreteScheduler exists in this version (confirmed by zit.py usage) |
| python | safetensors   | N/A — already in base.txt | N/A | load_file confirmed in flux2klein.py |

No new external dependencies are introduced. `EulerDiscreteScheduler` is already used by zit.py and available in `diffusers` per the existing requirements files. `secrets` is a Python stdlib module. `pipeline_cache` is a local module.

## Approach

**Step 1 — Update the import guard and add missing imports.**
In `flux2klein.py`, update the `try/except ImportError` block (lines 53–60) to also import `EulerDiscreteScheduler` from `diffusers` and add `import secrets` at the module level (after `import logging`, `import math`, `import re`). The `EulerDiscreteScheduler` must be guarded because mock-mode CI jobs install `base.txt` only and diffusers is in `base.txt` — wait, diffusers IS in `base.txt` per ARCHITECTURE.md §2. So `EulerDiscreteScheduler` import does NOT need the torch guard; it can be an unconditional import alongside `from safetensors import safe_open`. However, to be consistent with zit.py's pattern (which guards it inside the torch block), I'll follow zit.py's approach: add it to the existing try/except block so it becomes `None` on import failure. This is the safer pattern since diffusers depends on torch internally in some configurations.

**Step 2 — Add module-level constants for Flux 2 Klein's latent shape.**
Add two module-level constants (mirroring zit.py's `MODEL_PATCH_SIZE` and `MODEL_LATENT_CHANNELS`):
```python
MODEL_PATCH_SIZE: int = 8   # Flux 2 Klein default; updated in-place by load()
MODEL_LATENT_CHANNELS: int = 4
```
The default of 8 comes from the fixture's `latents` tensor shape (8×8). These will be set to actual checkpoint values by `load()` after `_infer_hyperparams()` extracts them.

**Step 3 — Implement `compute_latent_shape()`.**
Add the function with the same signature as zit.py:
```python
def compute_latent_shape(
    width: int, height: int, batch_size: int = 1
) -> tuple[int, int, int, int]:
```
The formula is identical to zit.py's: ceiling division `(width + MODEL_PATCH_SIZE - 1) // MODEL_PATCH_SIZE` for latent_height and `(height + MODEL_PATCH_SIZE - 1) // MODEL_PATCH_SIZE` for latent_width. The Flux 2 Klein-specific aspect is that `MODEL_PATCH_SIZE` defaults to 8 (Flux 2 Klein's actual patch size) rather than 2 (ZiT's). Returns `(batch_size, MODEL_LATENT_CHANNELS, latent_height, latent_width)`.

Add `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` markers next to the function definition. The real-mode test will be `test_compute_latent_shape_real_after_load` (called after `load()` sets the module constants). The mock-mode test will be `test_compute_latent_shape_mock_default_patch_size` (called before any `load()` — uses the default constants).

Add a Google-style docstring with `Args:` and `Returns:` sections.

**Step 4 — Add `Flux2KleinPipeline` class and `_assemble_pipeline()` helper.**
Mirror zit.py's `ZiTPipeline` class and `_assemble_pipeline()` function:
```python
class Flux2KleinPipeline:
    """Minimal pipeline wrapper that holds a ``Flux2KleinModel`` and a ``diffusers`` scheduler."""
    def __init__(self, model: Flux2KleinModel, scheduler: Any) -> None:
        self.model = model
        self.scheduler = scheduler
```

```python
def _assemble_pipeline(model: Flux2KleinModel) -> Flux2KleinPipeline:
    """Assemble a ``Flux2KleinPipeline`` from a loaded ``Flux2KleinModel``."""
    scheduler = EulerDiscreteScheduler()
    return Flux2KleinPipeline(model, scheduler)
```

Add a module-level `pipeline_cache` instance (same pattern as zit.py):
```python
pipeline_cache = PipelineCache()
```

Import `PipelineCache` from `worker.pipeline_cache` at the top of the file.

**Step 5 — Implement `_resolve_conditioning()` helper.**
Mirror zit.py's `_resolve_conditioning()` function:
```python
def _resolve_conditioning(conditioning: Any) -> tuple[Any, Any]:
    """Split conditioning into (positive, negative) embedding tensors."""
    if isinstance(conditioning, dict):
        return conditioning.get("text_embeds"), conditioning.get("negative_text_embeds")
    return conditioning, None
```

**Step 6 — Implement `sample()` with denoising loop.**
Add the `sample()` function with the same signature as zit.py:
```python
def sample(
    model: Flux2KleinModel,
    model_id: str,
    conditioning: Any,
    latent: torch.Tensor,
    steps: int,
    cfg: float,
    seed: int,
) -> tuple[torch.Tensor, int]:
```

Implementation steps (mirroring zit.py's exact sequence):
1. Guard: check `if torch is None` → raise `RuntimeError`.
2. Resolve seed: `if seed < 0: seed = secrets.randbelow(2**63)`.
3. Cache key: `key = f"{model_id}:pipeline"`.
4. Get pipeline from cache: `pipeline = pipeline_cache.get_or_load(key, lambda: _assemble_pipeline(model))`.
5. DEBUG log pipeline assembly.
6. Set timesteps: `scheduler.set_timesteps(steps)` (with the same `warnings.catch_warnings()` filter for the numpy compatibility deprecation that zit.py uses).
7. Clone latent: `latent = latent.clone()`.
8. Resolve conditioning: `cond_embeds, uncond_embeds = _resolve_conditioning(conditioning)`.
9. Denoising loop over `scheduler.timesteps`:
   a. Scale latent: `scaled_latent = scheduler.scale_model_input(latent, t)`.
   b. Unconditional pass: `noise_pred_uncond = pipeline.model(scaled_latent, t / 1000.0, conditioning=uncond_embeds)`.
   c. Conditional pass: `noise_pred_cond = pipeline.model(scaled_latent, t / 1000.0, conditioning=cond_embeds)`.
   d. CFG interpolation: `noise_pred = noise_pred_uncond + cfg * (noise_pred_cond - noise_pred_uncond)`.
   e. Step: `latent = scheduler.step(noise_pred, t, latent).prev_sample`.
10. INFO log denoising complete: `logger.info("denoising complete: steps=%d, seed=%d", steps, seed)`.
11. Return `(latent, seed)`.

Add `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` markers. The real-mode test will be `test_sample_denoising_real_flux2klein_fixture`. The mock-mode test will be `test_sample_seed_minus_one_resolves_random` (mock-mode collection safety — since `sample()` has no mock branch, the MOCK marker follows the same exception as `load()` per ANVILML_DESIGN.md §10.6).

Add a comprehensive Google-style docstring.

**Step 7 — Update `load()` to cache hyperparameters.**
Add the module-level constant update in `load()` (after `_infer_hyperparams()` returns), mirroring zit.py's pattern:
```python
global MODEL_PATCH_SIZE, MODEL_LATENT_CHANNELS
MODEL_PATCH_SIZE = hyperparams["patch_size"]
MODEL_LATENT_CHANNELS = hyperparams["latent_channels"]
```
This is necessary so that `compute_latent_shape()` uses the checkpoint's actual values when called after `load()`.

**Step 8 — Write tests in `test_arch_flux2klein.py`.**
Add `>=8` new tests (total file will have `>=25`):

1. `test_compute_latent_shape_mock_default_patch_size` (mock, no torch needed) — calls `compute_latent_shape()` with the default constants (patch_size=8), verifies exact outputs for 64×64 → (1, 4, 8, 8), 128×128 → (1, 4, 16, 16), 65×65 → (1, 4, 9, 9) (ceiling division).

2. `test_compute_latent_shape_real_after_load` (real_mode) — calls `load()` first to set module constants, then calls `compute_latent_shape()`, verifies outputs match expected values.

3. `test_compute_latent_shape_non_multiple_dims` (mock) — tests ceiling division for non-multiples: 100×80 → (1, 4, 13, 10).

4. `test_compute_latent_shape_batch_size` (mock) — tests batch_size parameter: `compute_latent_shape(64, 64, batch_size=4)` returns `(4, 4, 8, 8)`.

5. `test_sample_seed_minus_one_resolves_random` (real_mode) — calls `sample()` with seed=-1 twice, verifies both return positive seeds and that they differ (randomness).

6. `test_sample_seed_positive_reproducible` (real_mode) — calls `sample()` twice with the same seed, verifies identical output tensors.

7. `test_sample_pipeline_assembly_caching` (real_mode) — patches `pipeline_cache.get_or_load` to verify it's called exactly once (subsequent calls return cached value without re-assembling).

8. `test_sample_denoising_runs_to_completion` (real_mode) — calls `sample()` with a loaded model and small steps (4), verifies it returns a tensor of the same shape as the input latent and a positive seed.

9. `test_collection_safety_sample_import` (mock, collection safety) — subprocess test importing flux2klein without torch, confirming the module still loads. This serves as MOCK_PATH_VERIFIED for sample().

## Public API Surface

| Module Path | Item | Signature | Description |
|-------------|------|-----------|-------------|
| `worker.nodes.arch.diffusion.flux2klein` | `MODEL_PATCH_SIZE` | `int` (module-level constant, default 8) | Flux 2 Klein's default patch size; updated by `load()` |
| `worker.nodes.arch.diffusion.flux2klein` | `MODEL_LATENT_CHANNELS` | `int` (module-level constant, default 4) | Flux 2 Klein's default latent channels; updated by `load()` |
| `worker.nodes.arch.diffusion.flux2klein` | `pipeline_cache` | `PipelineCache` (module-level instance) | LRU cache for pipeline objects, keyed as `"{model_id}:pipeline"` |
| `worker.nodes.arch.diffusion.flux2klein` | `compute_latent_shape()` | `def compute_latent_shape(width: int, height: int, batch_size: int = 1) -> tuple[int, int, int, int]` | Compute latent tensor shape for a given resolution |
| `worker.nodes.arch.diffusion.flux2klein` | `Flux2KleinPipeline` | `class Flux2KleinPipeline(model: Flux2KleinModel, scheduler: Any)` | Pipeline wrapper holding model + scheduler |
| `worker.nodes.arch.diffusion.flux2klein` | `_assemble_pipeline()` | `def _assemble_pipeline(model: Flux2KleinModel) -> Flux2KleinPipeline` | Assemble pipeline from loaded model |
| `worker.nodes.arch.diffusion.flux2klein` | `_resolve_conditioning()` | `def _resolve_conditioning(conditioning: Any) -> tuple[Any, Any]` | Split conditioning into (positive, negative) tuple |
| `worker.nodes.arch.diffusion.flux2klein` | `sample()` | `def sample(model: Flux2KleinModel, model_id: str, conditioning: Any, latent: torch.Tensor, steps: int, cfg: float, seed: int) -> tuple[torch.Tensor, int]` | Run denoising loop and return denoised latent |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/diffusion/flux2klein.py` | Add `compute_latent_shape()`, `Flux2KleinPipeline`, `_assemble_pipeline()`, `_resolve_conditioning()`, `sample()`, module-level constants, import guard update, parity markers, logging |
| Modify | `worker/tests/test_arch_flux2klein.py` | Add `>=8` new tests for `compute_latent_shape` and `sample()` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `test_arch_flux2klein.py` | `test_compute_latent_shape_mock_default_patch_size` | `compute_latent_shape()` produces correct tuples for 64×64, 128×128, 65×65 using default patch_size=8 | Module imported, constants at defaults | 64×64 → (1,4,8,8); 128×128 → (1,4,16,16); 65×65 → (1,4,9,9) | Exact tuple match | `python -m pytest worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_mock_default_patch_size -v` exits 0 |
| `test_arch_flux2klein.py` | `test_compute_latent_shape_real_after_load` | After `load()` sets constants, `compute_latent_shape()` uses checkpoint's actual patch_size | `load()` called first (real_mode) | 64×64 with loaded fixture | Exact tuple match using loaded patch_size | `python -m pytest worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_real_after_load -v -m real_mode` exits 0 |
| `test_arch_flux2klein.py` | `test_compute_latent_shape_non_multiple_dims` | Ceiling division for non-multiples: 100×80 → (1,4,13,10) | Module imported | 100×80 | (1, 4, 13, 10) | `python -m pytest worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_non_multiple_dims -v` exits 0 |
| `test_arch_flux2klein.py` | `test_compute_latent_shape_batch_size` | batch_size parameter propagated correctly | Module imported | 64×64, batch_size=4 | (4, 4, 8, 8) | `python -m pytest worker/tests/test_arch_flux2klein.py::test_compute_latent_shape_batch_size -v` exits 0 |
| `test_arch_flux2klein.py` | `test_sample_seed_minus_one_resolves_random` | seed=-1 resolves to positive random int; two calls produce different seeds | torch installed (real_mode) | seed=-1, loaded model | Two distinct positive seeds | `python -m pytest worker/tests/test_arch_flux2klein.py::test_sample_seed_minus_one_resolves_random -v -m real_mode` exits 0 |
| `test_arch_flux2klein.py` | `test_sample_seed_positive_reproducible` | Same seed produces identical latent tensors | torch installed (real_mode) | seed=42, loaded model | Identical output tensors | `python -m pytest worker/tests/test_arch_flux2klein.py::test_sample_seed_positive_reproducible -v -m real_mode` exits 0 |
| `test_arch_flux2klein.py` | `test_sample_pipeline_assembly_caching` | `pipeline_cache.get_or_load` called once; subsequent calls return cached value without re-assembling | torch installed (real_mode) | Same model_id, two calls | First call loads, second returns cached | `python -m pytest worker/tests/test_arch_flux2klein.py::test_sample_pipeline_assembly_caching -v -m real_mode` exits 0 |
| `test_arch_flux2klein.py` | `test_sample_denoising_runs_to_completion` | Full denoising loop completes, returns tensor with same shape as input | torch installed (real_mode) | 4 steps, loaded model, 64×64 latent | Tensor shape matches input, positive seed | `python -m pytest worker/tests/test_arch_flux2klein.py::test_sample_denoising_runs_to_completion -v -m real_mode` exits 0 |
| `test_arch_flux2klein.py` | `test_collection_safety_sample_import` | Module imports without torch (mock-mode collection safety) | No subprocess, torch absent | Import flux2klein | No exception | `python -m pytest worker/tests/test_arch_flux2klein.py::test_collection_safety_sample_import -v` exits 0 |

## CI Impact

No CI job changes. The new tests use the existing `real_mode` marker and mock-mode collection patterns already established in this project. The `worker-linux-mock` CI job will collect the new mock-compatible tests (no torch import). The `worker-linux-real` CI job will run the `real_mode` tests. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. All new code uses Python stdlib (`secrets`, `math`) and `diffusers`/`torch` APIs that are platform-agnostic. No `#[cfg]` or path-separator handling needed — this is a pure Python module.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `EulerDiscreteScheduler` API shape may differ between diffusers versions — the constructor parameters, `set_timesteps()`, `scale_model_input()`, and `step()` method signatures could change between minor releases. | Medium | High | zit.py already uses these exact APIs and passes CI. Copy zit.py's usage verbatim. If the MCP lookup at ACT time reveals a different API, use zit.py as the authoritative reference since it's the established implementation. |
| Flux 2 Klein's `patch_size=8` (from fixture) means `compute_latent_shape()` will produce much larger latent grids than ZiT's `patch_size=2` — e.g. 512×512 input → (1,4,64,64) latent vs ZiT's (1,4,256,256). This is correct per the architecture but could cause OOM on the 10GB agent VM during real-mode denoising tests. | Low | Medium | Use the tiny fixture's actual dimensions (64×64 → (1,4,8,8) latent) for all real-mode tests. The denoising loop with 4 steps on an 8×8 latent tensor is trivially fast on CPU. |
| `sample()`'s denoising loop involves multiple `torch.no_grad()` forward passes through the model — if the model has many parameters, this could be slow on CPU. | Low | Low | The fixture has only 1 double block and 1 single block with hidden_dim=128. This is tiny and will complete in seconds on CPU. |
| `sample()` requires `diffusers` to be importable, but the import guard currently doesn't include `EulerDiscreteScheduler`. Adding it without the torch guard could break mock-mode collection if diffusers itself requires torch. | Low | High | Follow zit.py's pattern: add `EulerDiscreteScheduler` to the existing `try/except ImportError` block so it becomes `None` on import failure. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/diffusion/flux2klein.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_flux2klein.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_flux2klein.py -v` exits 0 (total `>=25` tests)
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_arch_flux2klein.py -v -m "not real_mode"` exits 0
- [ ] `python -m pytest worker/tests/test_arch_flux2klein.py -v -m real_mode` exits 0
- [ ] `grep -n "REAL_PATH_VERIFIED:" worker/nodes/arch/diffusion/flux2klein.py | grep -c "compute_latent_shape" | grep -q "^1$"` — compute_latent_shape has REAL_PATH_VERIFIED marker
- [ ] `grep -n "MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/flux2klein.py | grep -c "compute_latent_shape" | grep -q "^1$"` — compute_latent_shape has MOCK_PATH_VERIFIED marker
- [ ] `grep -n "REAL_PATH_VERIFIED:" worker/nodes/arch/diffusion/flux2klein.py | grep -c "def sample" | grep -q "^1$"` — sample has REAL_PATH_VERIFIED marker
- [ ] `grep -n "MOCK_PATH_VERIFIED:" worker/nodes/arch/diffusion/flux2klein.py | grep -c "def sample" | grep -q "^1$"` — sample has MOCK_PATH_VERIFIED marker
