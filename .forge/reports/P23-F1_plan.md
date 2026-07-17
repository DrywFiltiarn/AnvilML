# Plan Report: P23-F1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P23-F1                                            |
| Phase       | 023 — ZiT VAE Arch Module                         |
| Description | Runnable Proof: full load+sample+decode chain produces a real PIL image |
| Depends on  | P23-E1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-17T14:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/tests/test_e2e_zit_pipeline.py` — the first genuinely complete real-mode generation chain in the AnvilML project. This integration test chains the underlying arch modules directly: `LoadModel` (Phase 20) → `Sampler` (Phase 21) → `zit_vae.py`'s `decode()` (Phase 23), all against the respective fixture checkpoints (`zit_tiny.safetensors` and `zit_vae_tiny.safetensors`), producing a real `PIL.Image` object as the final output. The produced image's dimensions are verified against the requested width/height, confirming the shape contract between `compute_latent_shape()` and `decode()` holds end-to-end.

## Scope

### In Scope
- Create `worker/tests/test_e2e_zit_pipeline.py` with real-mode integration tests that chain LoadModel → Sampler → decode() directly against fixture checkpoints.
- The test imports torch unconditionally at module level (requires `real_mode` marker per §11.2 of ENVIRONMENT.md).
- Tests verify: (a) the full chain produces a `PIL.Image`, (b) the image dimensions match the latent spatial dimensions, (c) batch processing works correctly.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No stubs, no deferrals.

## Existing Codebase Assessment

The codebase has all the arch modules and fixtures needed for this integration test:

1. **Diffusion model loading**: `worker/nodes/arch/diffusion/zit.py` implements `load()` (meta construction + weight loading) and `sample()` (denoising pipeline). It is registered in `arch/diffusion/__init__.py` under the key `"zit"`. The `ZiTModel` class has `.arch == "zit"`.

2. **Sampler node**: `worker/nodes/sampler.py` — `Sampler.execute()` dispatches to `arch.diffusion.get_module(model.arch).sample()`, returning `(denoised_latent, resolved_seed)`. The real branch (line 93-123) calls `module.sample(model, job_id, conditioning, latent, steps, cfg, seed)`.

3. **VAE model loading**: `worker/nodes/arch/vae/zit_vae.py` implements `load()` (meta construction + weight loading) and `decode()` (latent-to-PIL-image). It is registered in `arch/vae/__init__.py` under the key `"zit_vae"`. The `ZiTVaeModel` class has `.arch == "zit_vae"`.

4. **Fixtures**: `worker/tests/fixtures/zit_tiny.safetensors` (diffusion, 8×8 latent) and `worker/tests/fixtures/zit_vae_tiny.safetensors` (VAE) are both present and have been exercised by existing unit tests.

5. **Existing test patterns**: `test_nodes_loader.py` and `test_nodes_sampler.py` demonstrate the established patterns: `_make_ctx()` helper for `NodeContext`, `@pytest.mark.real_mode` for real-mode tests, subprocess isolation for registry tests, and `pipeline_cache` cleanup between tests.

6. **Dual-mode parity markers**: `load()` and `decode()` in `zit_vae.py` already carry `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers pointing to existing tests in `test_arch_vae_zit.py`. This e2e test is a separate integration test — it does not replace those markers.

The only gap: `test_e2e_zit_pipeline.py` does not yet exist. This task fills that gap.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | torch   | (project venv)  | N/A — existing | n/a                    |
| python | pillow  | (project venv)  | N/A — existing | n/a                    |
| python | pytest  | (project venv)  | N/A — existing | n/a                    |

No new external dependencies. This task only creates a test file that imports existing packages (torch, PIL, pytest) already available in the worker venv.

## Approach

1. **Create `worker/tests/test_e2e_zit_pipeline.py`** — a new test file with real-mode integration tests chaining the arch modules directly.

2. **Module-level guarded torch import** — mirror the pattern from `test_arch_vae_zit.py`: import torch under a `try/except` guard at module level so the file stays importable in mock-mode CI collection (the worker-linux-mock job installs `base.txt` only, no torch). The `real_mode` marker ensures torch is actually available when these tests run.

3. **Helper function `_make_ctx(mock=False)`** — mirror the pattern from `test_nodes_sampler.py`: construct a `NodeContext` with `mock=False`, `device="cpu"`, bf16 caps, a random job_id (uuid4 bytes), empty pipeline cache, and a no-op emit lambda. This is needed for the Sampler node's real-mode execution.

4. **Test: `test_e2e_full_chain_produces_pil_image`** (real_mode) — the primary Runnable Proof:
   - Load the ZiT diffusion model from `fixtures/zit_tiny.safetensors` using `zit.load()` directly (not through the LoadModel node).
   - Load the ZiT VAE from `fixtures/zit_vae_tiny.safetensors` using `zit_vae.load()` directly (not through the LoadVae node).
   - Create an empty latent tensor of shape `(1, 4, 8, 8)` matching the fixture's latent dimensions, cast to the model's dtype.
   - Call `Sampler.execute(ctx, model=model, conditioning=None, clip={}, latent=latent, steps=20, cfg=7.5, seed=42)` to get `(denoised_latent, seed)`.
   - Call `zit_vae.decode(denoised_latent)` to produce a list of PIL Images.
   - Assert the result is a non-empty list of `PIL.Image.Image` objects (not mock sentinel).
   - Assert `images[0].size == (8, 8)` — dimensions match the latent spatial size.
   - Assert `images[0].mode == "RGB"`.
   - Clean up pipeline cache entry to avoid leaking state.

5. **Test: `test_e2e_batch_produces_multiple_images`** (real_mode) — verify batch processing:
   - Same setup as above, but create a `(2, 4, 8, 8)` latent tensor.
   - Assert `len(images) == 2` and all images have mode "RGB".

6. **Test: `test_e2e_image_is_real_pil_not_mock`** (real_mode) — assert the output is genuinely a PIL Image, not a mock sentinel:
   - Import `PIL.Image` directly.
   - Assert `isinstance(images[0], PIL.Image.Image)` — confirms this is a real image, not a dict with `"mock": True`.

7. **Update `docs/TESTS.md`** — add entries for the new tests per §11.4 of ENVIRONMENT.md and §5.10 of FORGE_AGENT_RULES.md. Each entry includes the `Mode: real` field.

## Public API Surface

None. This task only creates a test file — no public API changes.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/test_e2e_zit_pipeline.py` | New e2e integration test: LoadModel → Sampler → decode() chain |
| MODIFY | `docs/TESTS.md` | Add test catalogue entries for the new e2e tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_e2e_zit_pipeline.py` | `test_e2e_full_chain_produces_pil_image` (real) | Full load+sample+decode chain produces a real PIL Image with correct dimensions (8×8 RGB) | torch installed, fixtures present, bf16 caps | zit_tiny.safetensors, zit_vae_tiny.safetensors, latent (1,4,8,8), steps=20, cfg=7.5, seed=42 | `PIL.Image.Image` of size (8,8), mode "RGB" | `worker/.venv/bin/python -m pytest worker/tests/test_e2e_zit_pipeline.py::test_e2e_full_chain_produces_pil_image -v -m real_mode` exits 0 |
| `worker/tests/test_e2e_zit_pipeline.py` | `test_e2e_batch_produces_multiple_images` (real) | Batched latent (2 items) produces 2 PIL Images | torch installed, fixtures present | Same as above but latent (2,4,8,8) | List of 2 `PIL.Image.Image` objects, all mode "RGB" | `worker/.venv/bin/python -m pytest worker/tests/test_e2e_zit_pipeline.py::test_e2e_batch_produces_multiple_images -v -m real_mode` exits 0 |
| `worker/tests/test_e2e_zit_pipeline.py` | `test_e2e_image_is_real_pil_not_mock` (real) | Output is genuinely a PIL Image, not a mock sentinel dict | torch installed, fixtures present | Same as above | `isinstance(images[0], PIL.Image.Image)` is True | `worker/.venv/bin/python -m pytest worker/tests/test_e2e_zit_pipeline.py::test_e2e_image_is_real_pil_not_mock -v -m real_mode` exits 0 |

## CI Impact

The new test file is automatically picked up by:
- **`worker-linux-real`** CI job: `python -m pytest worker/tests -v -m real_mode` — will collect and run the 3 new tests.
- **`worker-windows-real`** CI job: same, Windows paths.
- **`worker-linux-mock`** and **`worker-windows-mock`** CI jobs: these install `base.txt` only (no torch) and run `pytest -m "not real_mode"`. The guarded torch import at module level prevents collection errors, and the `real_mode` marker excludes these tests from the mock suite.

No CI workflow file changes required.

## Platform Considerations

None identified. The test runs entirely on CPU with torch CPU wheel. No `#[cfg(unix)]` / `#[cfg(windows)]` guards needed. The fixture paths use `pathlib.Path` which handles cross-platform path separators automatically. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `zit.sample()` may require a conditioning tensor (not `None`) — the existing real-mode sampler test passes `conditioning=None`, but the actual `sample()` implementation may reject `None`. | Medium | High | Read `zit.py`'s `sample()` function body to confirm whether `conditioning=None` is accepted. If not, create a minimal conditioning tensor (e.g. `torch.zeros(1, 768)` for Qwen3-style text embeddings). |
| Pipeline cache state from prior tests may interfere with the e2e chain. | Low | Medium | Clean up pipeline cache entries after the test (as done in `test_sampler_real_denoises_zit_fixture`). Use unique model IDs to avoid collisions. |
| The fixture's latent spatial dimensions (8×8) may not match the sampler's expected output shape, causing a shape mismatch between sampler output and VAE decode input. | Low | High | Both fixtures are tiny synthetic checkpoints designed for the same latent space. The existing `test_arch_vae_zit.py` tests use `(1, 4, 8, 8)` latents with the VAE fixture, and `test_nodes_sampler.py` uses the same shape with the diffusion fixture. This shape contract is already verified by existing tests. |
| Torch CPU inference may be too slow for CI timeout. | Low | Medium | The fixtures are tiny (8×8 latent, 2 blocks), so the denoising loop and VAE forward pass complete in seconds, not minutes. The existing real-mode tests in `test_nodes_sampler.py` and `test_arch_vae_zit.py` already exercise these same models on CPU. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_e2e_zit_pipeline.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_e2e_zit_pipeline.py -v -m real_mode` exits 0, with all 3 tests passing
- [ ] The produced image is a real `PIL.Image.Image` (not a mock sentinel dict) with dimensions matching the requested width/height (8×8 for the tiny fixture)
- [ ] `docs/TESTS.md` is updated with entries for the new tests, including `Mode: real`
