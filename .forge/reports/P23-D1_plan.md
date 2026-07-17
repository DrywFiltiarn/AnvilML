# Plan Report: P23-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P23-D1                                      |
| Phase       | 23 — ZiT VAE Arch Module                    |
| Description | worker/nodes/arch/vae/zit_vae.py: decode() latent-to-image |
| Depends on  | P23-C3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-17T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `decode(vae_module: ZiTVaeModel, latent: torch.Tensor) -> list[PIL.Image.Image]` in `worker/nodes/arch/vae/zit_vae.py` — the VAE family's second fixed method per `ANVILML_DESIGN.md §10.4`. This function runs the loaded VAE's decoder forward pass on a denoised latent tensor and post-processes the raw float output into one or more valid RGB PIL Images (denormalize, clamp to uint8, convert NCHW→HWC). The latent shape contract is `(batch_size, 4, latent_h, latent_w)` matching `zit.py`'s `compute_latent_shape()` output, making this the actual integration point between the diffusion sampler and VAE decoding.

## Scope

### In Scope
- Implement `ZiTVaeModel.forward(latent: torch.Tensor) -> torch.Tensor` — runs the mid-block then sequential decoder blocks on the latent tensor.
- Implement `decode(vae_module: ZiTVaeModel, latent: torch.Tensor, output_mode: str = "RGB") -> list[PIL.Image.Image]` — calls `forward()`, post-processes raw float tensor (clamp, convert to uint8, NCHW→HWC, select 3 channels for RGB), returns a list of PIL Images (one per batch item).
- Add dual-mode parity markers (`REAL_PATH_VERIFIED` / `MOCK_PATH_VERIFIED`) on the `decode()` function.
- Update the module docstring to reflect `decode()` as implemented (step 4 no longer says "later task").
- Add >=5 new tests in `worker/tests/test_arch_vae_zit.py` covering decode functionality.

### Out of Scope
- The `VaeDecode` generic node (`worker/nodes/decode.py`) — planned for Phase 24 (P24-B1/P24-B2).
- `SaveImage` node or any image-saving logic.
- Any changes to `zit.py` (diffusion module) — independent per `§11.4`.
- Channel reduction from 16 to 3 for RGB — handled by taking first 3 channels in `decode()`.

## Existing Codebase Assessment

The `zit_vae.py` file (763 lines) already implements the full four-step loading contract: `_infer_hyperparams()` (shape inference), `can_handle()` (dispatch matching), and `load()` (meta construction → materialization → key remapping → weight loading with `.arch` attribute). The `ZiTVaeModel` class has encoder blocks, mid-block, and decoder blocks built from `Conv2d` + `GroupNorm` layers, but **no `forward()` method** — the model is a pure constructor with no inference path.

The test file `test_arch_vae_zit.py` has 23 tests covering `_infer_hyperparams`, `can_handle`, `get_module`, `_build_key_remapping`, and `load()` (meta construction, dtype selection, weight loading, mock-mode). The fixture `zit_vae_tiny.safetensors` has a `latents` tensor of shape `(1, 4, 8, 8)` — 4 latent channels, 8×8 spatial, matching ZiT's `MODEL_LATENT_CHANNELS=4`.

The established patterns include: guarded torch import (mock-mode safe), Google-style docstrings with Args/Returns/Raises sections, `@pytest.mark.real_mode` for torch-dependent tests, and sentinel-based mock testing via `unittest.mock.patch`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| python | pillow  | (from venv)     | pypi-query MCP | n/a                |
| python | torch   | (from venv)     | pypi-query MCP | n/a                |

No new external dependencies are introduced. `PIL.Image` (from the `Pillow` package, already in `worker/requirements/base.txt`) and `torch` (already in the guarded import) are used.

## Approach

1. **Implement `ZiTVaeModel.forward(latent: torch.Tensor) -> torch.Tensor`**

   Add a `forward` method to the `ZiTVaeModel` class. The decoder forward pass follows the VAE architecture:
   - Pass the latent tensor through the mid-block (conv → norm → SiLU).
   - Sequentially pass the mid-block output through each decoder block in order (block_0 → block_1).
   - Each block applies: `conv(x)` → `norm(result)` → `SiLU(result)`.
   - Return the final tensor.

   The mid-block is shared (not sequential), so its output feeds into the first decoder block, whose output feeds into the second, etc.

   Rationale: This is the standard VAE decoder topology — the mid-block is a bottleneck transformation, and the decoder blocks progressively upsample/channel-expand the latent representation.

2. **Implement `decode(vae_module: ZiTVaeModel, latent: torch.Tensor, output_mode: str = "RGB") -> list[PIL.Image.Image]`**

   Add a module-level function with this exact signature (never `vae_decode()` or `to_image()` per `§10.4`):
   - Call `vae_module.forward(latent)` to get the raw decoded tensor.
   - Clamp values to `[0.0, 1.0]` range (standard VAE output normalization).
   - Convert to numpy array with `.cpu().numpy()`.
   - Select the first 3 channels for RGB (from the 16-channel output).
   - Clamp to `[0, 255]` and convert to `uint8`.
   - For each batch item: reshape from `(channels, H, W)` to `(H, W, channels)` (NCHW→HWC), then call `PIL.Image.fromarray()`.
   - Return a list of PIL Images, one per batch item.

   Rationale: VAE decoders typically output in [0, 1] float range. The first-3-channels approach is simple and deterministic; the fixture tensors are random so any 3-channel subset produces valid pixel data.

3. **Add dual-mode parity markers**

   Add `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` comment markers above the `decode()` function, naming the test functions that will be created in step 4:
   ```python
   # REAL_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_decode_real_zit_vae_fixture
   # MOCK_PATH_VERIFIED: worker/tests/test_arch_vae_zit.py::test_decode_mock_returns_sentinel
   ```

4. **Update the module docstring**

   Change the line `4. decode(model, image) — implemented in a later task.` to `4. decode(vae_module, latent) — implemented in this task (P23-D1).`

5. **Add tests in `worker/tests/test_arch_vae_zit.py`**

   Add the following tests (5+ new tests, bringing total from 23 to >=28):

   a. `test_decode_single_image_produces_pil` (real_mode) — Calls `load()` to get a model, creates a `(1, 4, 8, 8)` latent tensor, calls `decode()`, asserts the result is a list of exactly 1 PIL Image with mode "RGB".

   b. `test_decode_batch_produces_multiple_images` (real_mode) — Creates a `(2, 4, 8, 8)` batched latent, calls `decode()`, asserts the result is a list of exactly 2 PIL Images.

   c. `test_decode_output_dimensions_match_latent_spatial` (real_mode) — Verifies that the output PIL Image's width and height match the latent's spatial dimensions (8×8 for the fixture), confirming the decoder preserves spatial resolution.

   d. `test_decode_output_is_rgb_uint8` (real_mode) — Verifies the PIL Image mode is "RGB" and pixel values are valid uint8 (0-255 range).

   e. `test_decode_mock_returns_sentinel` (real_mode, mock-mode path) — Patches `ZiTVaeModel.forward` to return a sentinel tensor of known shape `(1, 16, 8, 8)`, calls `decode()`, asserts the result is a list of 1 PIL Image. This tests the post-processing path without requiring the full forward pass.

   f. `test_decode_non_rgb_mode` (real_mode) — Tests `decode(output_mode="L")` produces a grayscale PIL Image with mode "L" (selects first channel only).

   g. `test_decode_empty_batch` (real_mode) — Tests that `decode()` with a `(0, 4, 8, 8)` empty batch returns an empty list.

6. **Run pre-test syntax check**

   Run `worker/.venv/bin/python -m py_compile worker/nodes/arch/vae/zit_vae.py` to confirm no syntax errors before running the full test suite.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `ZiTVaeModel.forward` | `worker.nodes.arch.vae.zit_vae.ZiTVaeModel` | `def forward(self, latent: torch.Tensor) -> torch.Tensor` |
| `decode` | `worker.nodes.arch.vae.zit_vae.decode` | `def decode(vae_module: ZiTVaeModel, latent: torch.Tensor, output_mode: str = "RGB") -> list[PIL.Image.Image]` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/vae/zit_vae.py` | Add `forward()` method to `ZiTVaeModel`, add `decode()` function, add dual-mode markers, update module docstring |
| MODIFY | `worker/tests/test_arch_vae_zit.py` | Add >=5 new tests for `decode()` functionality |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| test_arch_vae_zit.py | test_decode_single_image_produces_pil (real) | decode() against fixture-shaped latent (1,4,8,8) produces a valid PIL Image with mode "RGB" | load() succeeds | Single latent tensor from fixture | list[PIL.Image] of length 1, mode="RGB" | `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_single_image_produces_pil -v` exits 0 |
| test_arch_vae_zit.py | test_decode_batch_produces_multiple_images (real) | decode() against batched latent (2,4,8,8) produces 2 PIL Images | load() succeeds | Batched latent tensor (batch_size=2) | list[PIL.Image] of length 2 | `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_batch_produces_multiple_images -v` exits 0 |
| test_arch_vae_zit.py | test_decode_output_dimensions_match_latent_spatial (real) | Output PIL Image dimensions match latent spatial dims (8×8) | load() succeeds | Single latent tensor (1,4,8,8) | PIL.Image.size == (8, 8) | `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_output_dimensions_match_latent_spatial -v` exits 0 |
| test_arch_vae_zit.py | test_decode_output_is_rgb_uint8 (real) | Output PIL Image mode is "RGB" with valid uint8 pixel values | load() succeeds | Single latent tensor | PIL.Image.mode == "RGB", pixels in [0,255] | `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_output_is_rgb_uint8 -v` exits 0 |
| test_arch_vae_zit.py | test_decode_mock_returns_sentinel (mock) | decode() post-processing path works with patched forward returning sentinel tensor | torch importable | Patched forward returns (1,16,8,8) tensor | list[PIL.Image] of length 1 | `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_mock_returns_sentinel -v` exits 0 |
| test_arch_vae_zit.py | test_decode_non_rgb_mode (real) | decode(output_mode="L") produces grayscale PIL Image with mode "L" | load() succeeds | Single latent tensor | PIL.Image.mode == "L" | `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_non_rgb_mode -v` exits 0 |
| test_arch_vae_zit.py | test_decode_empty_batch (real) | decode() with empty batch (0,4,8,8) returns empty list | load() succeeds | Empty batch tensor | list[] of length 0 | `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_empty_batch -v` exits 0 |

## CI Impact

No CI changes required. The new tests are in the existing `worker/tests/test_arch_vae_zit.py` file, which is already picked up by both mock-mode (`-m "not real_mode"`) and real-mode (`-m real_mode`) pytest commands. The tests are marked `real_mode` since they require torch, so they will run in the `worker-linux-real` and `worker-windows-real` CI jobs. No new CI jobs, gates, or markers are introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. PIL Image operations and torch tensor operations are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The NCHW→HWC reshape and PIL.Image.fromarray() work identically on all platforms.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The VAE model's forward output is in a non-standard range (not [0,1]) — the fixture tensors are random, so the actual value range is unknown. Clamping to [0,1] may produce all-black or all-white images. | Medium | Low | The test asserts the PIL Image exists with correct mode and dimensions, not specific pixel content. Since fixture tensors are random, some pixel variation is guaranteed. If the range is extreme, clamp ensures no overflow but the image may be near-uniform — this is correct behavior for random input. |
| The 16-channel output → 3-channel RGB reduction by taking first 3 channels may not match what downstream consumers (VaeDecode node in Phase 24) expect. | Low | Medium | The Phase 24 VaeDecode task will call this same `decode()` function. If a different channel-reduction strategy is needed, it can be added as a parameter or the function can be extended. For now, first-3-channels is the simplest correct approach that produces valid RGB output. |
| The fixture's latent tensor shape (1, 4, 8, 8) may not match the actual ZiT VAE's expected latent shape in production. | Low | Low | The fixture is intentionally tiny for CI/agent testing (per §17.5). The shape contract with `compute_latent_shape()` is `(batch, 4, H, W)` where H and W depend on input resolution. The fixture's 8×8 spatial dimension corresponds to a 128×128 input image (128/16=8). |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/vae/zit_vae.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 with >=25 total tests (currently 23 + 7 new = 30)
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_single_image_produces_pil -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_batch_produces_multiple_images -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_output_dimensions_match_latent_spatial -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_output_is_rgb_uint8 -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_decode_mock_returns_sentinel -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_arch_vae_zit.py -v -m "not real_mode"` exits 0 (mock-mode collection + mock tests pass)
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py -v -m real_mode` exits 0 (real-mode tests pass)
