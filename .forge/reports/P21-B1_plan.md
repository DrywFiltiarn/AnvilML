# Plan Report: P21-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P21-B1                                            |
| Phase       | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description | worker/nodes/arch/diffusion/zit.py: sample() pipeline assembly + caching |
| Depends on  | P21-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-14T09:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add `sample()` to `worker/nodes/arch/diffusion/zit.py` that assembles a runnable pipeline from the already-loaded `ZiTModel` (produced by `load()`, Phase 20) and caches it under `f"{model_id}:pipeline"` in the per-process `PipelineCache`. The first call for a given `model_id` assembles and caches; subsequent calls reuse the cached pipeline. This is cache-assembly only — the denoising loop call itself is deferred to P21-B2. Acceptance: >=4 new tests in `test_arch_zit.py` verifying assembly, reuse, and per-model isolation; `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (>=35 total, up from current 31).

## Scope

### In Scope
- Add a module-level `PipelineCache()` instance to `zit.py` (imported from `pipeline_cache`).
- Implement `sample(model, model_id, conditioning, latent, steps, cfg, seed)` in `zit.py`:
  - On first call for a given `model_id`, assemble a runnable pipeline from the provided `model` (an already-loaded `ZiTModel` instance) using `pipeline_cache.get_or_load(f"{model_id}:pipeline", lambda: _assemble_pipeline(model))`.
  - Cache the assembled pipeline under `f"{model_id}:pipeline"` — distinct from the raw component's cache key used by `load()`.
  - Return the cached pipeline object.
  - Do NOT run the denoising loop (deferred to P21-B2).
- Implement `_assemble_pipeline(model: ZiTModel)` internal helper that constructs a runnable pipeline wrapper from the `ZiTModel`:
  - Creates a minimal pipeline-like object that wraps the `ZiTModel` and provides the interface expected by the denoising loop (to be wired in P21-B2).
  - Uses `diffusers` scheduler classes as building blocks per §11.2's library boundary (allowed: layer/block classes; not allowed: `DiffusionPipeline.from_pretrained()`).
  - The pipeline object is a simple wrapper class (`ZiTPipeline`) that holds the model and a scheduler instance.
- Add `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` marker comments next to `sample()` per §10.6's dual-mode parity marker convention.
- Add Google-style docstring to `sample()` and `_assemble_pipeline()`.
- Add DEBUG log call when a pipeline is assembled and cached.
- Write >=4 tests in `test_arch_zit.py` covering: first-call assembly with call-count spy, second-call reuse, different model_id isolation, and pipeline object type verification.

### Out of Scope
- The denoising loop call inside `sample()` — deferred to P21-B2 (see `defers_to: ["P21-B2"]`).
- Seed `-1` resolution logic — deferred to P21-B2.
- Any changes to `pipeline_cache.py` — already implemented in Phase 19 (P19-B1).
- The `Sampler` generic node — separate task group (P21-C1, P21-C2).
- VAE decoding — out of scope for this phase.

## Existing Codebase Assessment

`zit.py` (777 lines) currently implements: `ZiTModel` class, `compute_latent_shape()`, `load()`, `can_handle()`, and helper functions (`_infer_hyperparams()`, `_select_dtype()`, `_build_key_remapping()`, `_safetensors_dtype_to_canonical()`). The `load()` function is fully implemented per Phase 20 — it constructs the model on meta-device, materializes to the target device, remaps checkpoint keys, and loads weights via `load_state_dict(assign=True)`.

The `PipelineCache` class exists in `pipeline_cache.py` (102 lines) as a single-threaded LRU cache with `get_or_load(key, loader_fn)` — returns cached value if present, calls `loader_fn` exactly once on miss, caches the result, evicts LRU on capacity overflow. Failed loader calls do not populate the cache.

The dispatcher in `worker/nodes/arch/diffusion/__init__.py` already imports and registers `zit`. Test file `test_arch_zit.py` has 31 tests covering `_infer_hyperparams()`, `can_handle()`, `get_module()`, `_select_dtype()`, `load()`, key remapping, and `compute_latent_shape()`.

Established patterns: Google-style docstrings with Args/Returns/Raises sections; `# REAL_PATH_VERIFIED:` / `# MOCK_PATH_VERIFIED:` marker comments next to public functions; `logger.debug()` for internal state; `logger.info()` for operational events; ceiling division with `(x + n - 1) // n`; `global` state for module-level constants (`MODEL_PATCH_SIZE`, `MODEL_LATENT_CHANNELS`); test fixtures under `worker/tests/fixtures/`.

No gap between the design doc and current source that affects this task's approach. The design doc's §11.6 boundary (assemble from loaded nn.Module, not from_pretrained) is consistent with the existing raw-construction pattern in `load()`.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source | Feature flags confirmed |
|--------|-----------|-----------------|------------|------------------------|
| python | diffusers | 0.35.1          | pypi-query | n/a                    |
| python | torch     | 2.7.x (local)   | local      | n/a                    |

No new external dependencies are introduced. `diffusers` is already in `worker/requirements/base.txt` and used for scheduler classes (allowed per §11.2). `torch` and `torch.nn` are already imported in `zit.py`. The `PipelineCache` is an existing module in the worker package.

## Approach

**Step 1 — Add PipelineCache import and module-level cache to zit.py.**

Import `PipelineCache` from `pipeline_cache` at the top of `zit.py` alongside the existing imports. Create a module-level cache instance: `pipeline_cache = PipelineCache()`. This provides the per-process LRU cache for pipeline objects, keyed by `f"{model_id}:pipeline"`.

**Step 2 — Implement _assemble_pipeline(model) internal helper.**

Add `_assemble_pipeline(model: ZiTModel) -> Any` at module level (after `ZiTModel` class, before `compute_latent_shape()`). This function:
- Accepts a `ZiTModel` instance.
- Creates a minimal `ZiTPipeline` wrapper class instance that holds the model and a scheduler.
- The `ZiTPipeline` class is defined inside or alongside `_assemble_pipeline` — it stores the `ZiTModel` instance and a `diffusers` scheduler (e.g., `EulerDiscreteScheduler` or a simple no-op scheduler since the actual denoising loop isn't implemented yet).
- Returns the `ZiTPipeline` instance.

The wrapper class stores: `self.model = model`, `self.scheduler = scheduler`. This provides the interface that the denoising loop (P21-B2) will call. The scheduler is a simple placeholder since the full denoising step function isn't wired yet.

**Step 3 — Implement sample() function with cache-assembly.**

Add `sample(model, model_id, conditioning, latent, steps, cfg, seed)` after `load()` in `zit.py`. The function:
1. Constructs the cache key: `key = f"{model_id}:pipeline"`.
2. Calls `pipeline_cache.get_or_load(key, lambda: _assemble_pipeline(model))` to get or assemble the pipeline.
3. Logs at DEBUG level: `"assembled pipeline for model_id=%s"`.
4. Returns the pipeline object.

The function does NOT run denoising. It returns the cached/just-assembled pipeline so P21-B2 can extend it to also run the denoising loop.

Add Google-style docstring describing: what it does, parameters, return value, and noting that denoising is deferred to a later task.

**Step 4 — Add dual-mode parity markers.**

Add `# REAL_PATH_VERIFIED:` and `# MOCK_PATH_VERIFIED:` comment markers next to `sample()`, naming the test functions defined in Step 5.

**Step 5 — Write tests in test_arch_zit.py.**

Add >=4 tests:
1. `test_sample_first_call_assembles_pipeline` — spy on `pipeline_cache.get_or_load` call count; first call for `model_id="test1"` invokes the loader once; verifies a `ZiTPipeline` is returned.
2. `test_sample_second_call_reuses_cached_pipeline` — call `sample()` twice with same `model_id`; verify the second call returns the same pipeline object without re-assembly (spy on call count stays at 1).
3. `test_sample_different_model_id_gets_separate_pipeline` — call `sample()` with two different `model_id` values; verify two separate pipeline objects are cached and returned.
4. `test_sample_pipeline_is_zit_wrapper` — verify the returned pipeline object has a `.model` attribute that is the same `ZiTModel` instance passed in.

All tests use a mock `ZiTModel` (or the fixture-loaded model) and a spy on the cache's `get_or_load` method. The spy tracks how many times the loader function is invoked.

Mock-mode test: `test_sample_first_call_assembles_pipeline_mock` and `test_sample_second_call_reuses_cached_pipeline_mock` (with `MOCK_PATH_VERIFIED` marker).
Real-mode test: `test_sample_pipeline_assembled_from_loaded_model` (with `REAL_PATH_VERIFIED` marker, using fixture-loaded model).

## Public API Surface

| Item | Path | Signature | Description |
|------|------|-----------|-------------|
| function | `worker.nodes.arch.diffusion.zit.sample` | `def sample(model: ZiTModel, model_id: str, conditioning: Any, latent: torch.Tensor, steps: int, cfg: float, seed: int) -> Any` | Assembles and caches a runnable pipeline from the provided model; returns the cached pipeline. |
| class | `worker.nodes.arch.diffusion.zit.ZiTPipeline` | `class ZiTPipeline` | Internal wrapper holding a `ZiTModel` and scheduler, returned by `sample()`. |
| function | `worker.nodes.arch.diffusion.zit._assemble_pipeline` | `def _assemble_pipeline(model: ZiTModel) -> Any` | Internal helper that constructs a `ZiTPipeline` from a `ZiTModel`. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/diffusion/zit.py` | Add `sample()`, `_assemble_pipeline()`, `ZiTPipeline` class, `PipelineCache` import, parity markers |
| Modify | `worker/tests/test_arch_zit.py` | Add >=4 tests for pipeline assembly and caching behavior |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| test_arch_zit.py | test_sample_first_call_assembles_pipeline_mock (mock) | First call to sample() with model_id="test1" assembles and caches pipeline; get_or_load loader called exactly once | `python -m pytest worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock -v` |
| test_arch_zit.py | test_sample_second_call_reuses_cached_pipeline_mock (mock) | Second call with same model_id returns cached pipeline without re-assembly; call count stays at 1 | `python -m pytest worker/tests/test_arch_zit.py::test_sample_second_call_reuses_cached_pipeline_mock -v` |
| test_arch_zit.py | test_sample_different_model_id_isolation (mock) | Two different model_ids produce separate pipeline objects in the cache | `python -m pytest worker/tests/test_arch_zit.py::test_sample_different_model_id_isolation -v` |
| test_arch_zit.py | test_sample_pipeline_is_zit_wrapper (mock) | Returned pipeline has .model attribute matching the input ZiTModel instance | `python -m pytest worker/tests/test_arch_zit.py::test_sample_pipeline_is_zit_wrapper -v` |
| test_arch_zit.py | test_sample_pipeline_assembled_from_loaded_model (real) | sample() with fixture-loaded model produces a pipeline with the correct model; REAL_PATH_VERIFIED | `python -m pytest worker/tests/test_arch_zit.py::test_sample_pipeline_assembled_from_loaded_model -v -m real_mode` |

## CI Impact

No CI changes required. The tests are added to the existing `test_arch_zit.py` file, which is already collected by the mock-mode pytest run (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v -m "not real_mode"`) and the real-mode pytest run (`python -m pytest worker/tests/ -v -m real_mode`). No new test markers or file patterns are introduced.

## Platform Considerations

None identified. The `PipelineCache` is a pure Python in-memory LRU cache with no platform-specific behavior. `ZiTPipeline` holds a `torch.nn.Module` and a `diffusers` scheduler — both are cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `ZiTPipeline` wrapper's interface may not match what P21-B2's denoising loop expects, requiring a restructure. | Medium | Medium | Design `ZiTPipeline` with the minimal attributes the denoising loop will need (`.model`, `.scheduler`). P21-B2's plan will confirm the exact interface; if it differs, the wrapper is adjusted then. For this task, a simple `.model` + `.scheduler` wrapper is sufficient. |
| Test fixture `ZiTModel` construction may be slow or memory-intensive for the pipeline assembly tests. | Low | Low | Use a small fixture checkpoint (the existing `zit_tiny.safetensors`) which already loads quickly on CPU. The `_assemble_pipeline` function only wraps the model, it doesn't run inference. |
| The `diffusers` scheduler class API may differ from what's available in the installed version. | Low | Medium | Verify the exact scheduler class name and constructor at ACT time via MCP (pypi-query). Use a simple no-op scheduler if the full `EulerDiscreteScheduler` API has changed. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/diffusion/zit.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_zit.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_first_call_assembles_pipeline_mock -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_second_call_reuses_cached_pipeline_mock -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_different_model_id_isolation -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_pipeline_is_zit_wrapper -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_sample_pipeline_assembled_from_loaded_model -v -m real_mode` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with >=35 tests total (31 existing + >=4 new)
