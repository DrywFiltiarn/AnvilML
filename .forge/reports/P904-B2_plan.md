# Plan Report: P904-B2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-B2                                           |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/arch/diffusion/zit.py: load_vae() — replace diffusers-internals reuse with shape-inferred config + manual key remap |
| Depends on  | P904-B1                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-24T15:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace `load_vae()`'s dependency on the private, unversioned `diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint` function with a fully self-contained implementation that infers the `AutoencoderKL` config from the raw checkpoint's tensor shapes and performs the key remap manually. This eliminates the diffusers-internal import while maintaining identical loading behavior.

## Scope

### In Scope
- Add `_infer_vae_config_from_checkpoint(checkpoint: dict) -> dict` helper that infers `latent_channels`, `block_out_channels`, `in_channels`, `out_channels`, and `layers_per_block` from tensor shapes present in the raw LDM-format checkpoint.
- Add `_remap_ldm_vae_keys(checkpoint: dict) -> dict` helper that strips LDM prefixes (`vae.`, `first_stage_model.`) and remaps LDM block keys to diffusers `AutoencoderKL` key format, including the encoder down-blocks, decoder up-blocks, mid block, and final conv — with correct `-1` offset for decoder `layers_per_block`.
- Rewrite `load_vae()` to use the two new helpers instead of `convert_ldm_vae_checkpoint` and hardcoded `block_out_channels=[128, 256, 512, 512]` / hardcoded `4` stages.
- Add unit tests for `_infer_vae_config_from_checkpoint` and `_remap_ldm_vae_keys` in `worker/tests/test_arch_zit.py`.
- Update `__all__` to include new private helpers (no — they remain private, not exported).

### Out of Scope
None. `defers_to (from JSON): []` — this task may not defer any scope. All described functionality is implemented in full.

## Existing Codebase Assessment

The existing `load_vae()` function (lines 534–619 of `zit.py`) constructs `AutoencoderKL(block_out_channels=[128, 256, 512, 512])` with hardcoded block channels, loads the raw checkpoint via `safetensors.torch.load_file`, and remaps keys by importing and calling `diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint(checkpoint, config)` where `config` is a dict with `"down_block_types": ["DownEncoderBlock2D"] * 4` and `"up_block_types": ["UpDecoderBlock2D"] * 4`.

The sibling `load_transformer()` (lines 435–531) already demonstrates the pattern this task follows: it has `_infer_config_from_checkpoint()` and `_remap_z_image_keys()` private helpers that perform shape inference and key remapping without any diffusers-internal imports. The `_remap_z_image_keys` function is a good reference for the style — it operates on a copy, applies sequential string replacements, handles tensor splitting via `torch.chunk` (imported locally inside the real-mode branch), and never mutates the original checkpoint.

The test file `worker/tests/test_arch_zit.py` follows a consistent pattern: synthetic checkpoints built with `torch.ones(shape)` for pure key-manipulation tests, mock-mode isolation via `ANVILML_WORKER_MOCK=1`, and `os.environ` capture-and-restore for tests that override the mock flag. The existing `test_remap_key_transformations()` test (line 695) is a direct template for how `_remap_ldm_vae_keys` should be tested.

The `conftest.py` autouse fixture ensures `ANVILML_WORKER_MOCK=1` for all tests, which means the new helper functions can be tested in pure-Python mode (no torch needed for shape inference, only for tensor creation in test fixtures). The `test_no_diffusers_internal_import()` test (line 660) confirms the pattern of reading source text to verify no private diffusers import exists — this same pattern will apply to `convert_ldm_vae_checkpoint`.

No prior source exists for `_infer_vae_config_from_checkpoint` or `_remap_ldm_vae_keys` — they are new additions to `zit.py`.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source         | Feature flags confirmed |
|--------|-------------|-----------------|--------------------|------------------------|
| python | diffusers   | 0.38.0          | pypi-query MCP     | n/a                    |
| python | torch       | (project env)   | —                  | n/a                    |
| python | safetensors | (project env)   | —                  | n/a                    |

No new external dependencies are introduced. The task removes a diffusers-internal import and replaces it with local Python logic.

## Approach

1. **Add `_infer_vae_config_from_checkpoint(checkpoint)` helper.**

   Implement a function that scans the raw LDM-format checkpoint keys for tensor shapes and derives the `AutoencoderKL` config parameters. The function receives a dict of `{key: tensor}` (the raw state dict from `safetensors.torch.load_file`).

   Shape inference rules (confirmed against real VAE checkpoint scan in task context):
   - `latent_channels`: Find the first key containing `decoder.conv_in.weight`. The shape is `[out_channels, latent_channels, kernel_h, kernel_w]` (e.g., `[512, 16, 3, 3]`). Extract `shape[1]` as `latent_channels`.
   - `block_out_channels`: Scan keys matching `decoder.up.{N}.block.{M}.conv1.weight` for unique stage indices `N`. For each `N`, read the first dimension of the conv1 weight to get the channel count for that stage. Collect all `(N, channel)` pairs, sort by `N` ascending, and extract the channel values as a list. This dynamically discovers the stage count and per-stage channels rather than hardcoding 4 stages.
   - `in_channels`: Find the first key containing `encoder.conv_in.weight`. The shape is `[out_channels, in_channels, kernel_h, kernel_w]`. Extract `shape[1]` as `in_channels`.
   - `out_channels`: Find the first key containing `decoder.conv_out.weight`. The shape is `[out_channels, in_channels, kernel_h, kernel_w]`. Extract `shape[0]` as `out_channels`.
   - `layers_per_block`: Count the unique block indices `M` in `decoder.up.{N}.block.{M}.conv1.weight` for any `N`. The observed count is the number of resnet blocks per decoder stage. Apply the `-1` offset (diffusers' `Decoder` class uses `num_layers = layers_per_block + 1` for up-blocks) to derive the actual `layers_per_block` value. For example, if 3 resnets are observed, `layers_per_block = 3 - 1 = 2`.

   Return a dict with keys: `latent_channels`, `block_out_channels`, `in_channels`, `out_channels`, `layers_per_block`.

   Raise `ValueError` if any required key is absent.

2. **Add `_remap_ldm_vae_keys(checkpoint)` helper.**

   Implement a function that remaps raw LDM-format checkpoint keys to diffusers `AutoencoderKL` key format. Operates on a shallow copy to avoid mutating the original checkpoint.

   Transformation steps:
   a. **Prefix stripping:** For each key, strip leading `vae.` and `first_stage_model.` prefixes (both are common LDM checkpoint prefix styles).
   b. **Encoder down-block remap:** Keys matching `encoder.down.{N}.block.{M}.conv{1,2}.weight` map to `encoder.down_blocks.{N}.resnets.{M}.conv{1,2}.weight`. Similarly, `encoder.down.{N}.block.{M}.conv{1,2}.norm{1,2}.weight` maps to `encoder.down_blocks.{N}.resnets.{M}.conv{1,2}.norm{1,2}.weight`.
   c. **Encoder downsample:** Keys matching `encoder.down.{N}.block.{M}.conv_down.weight` map to `encoder.down_blocks.{N}.conv_down.weight`.
   d. **Encoder down block 0:** `encoder.down.0.block.0.conv{1,2}.weight` → `encoder.down_blocks.0.resnets.0.conv{1,2}.weight` (block 0 of down stage 0).
   e. **Mid block:** Keys matching `decoder.mid.block_{1,2}.conv{1,2}.weight` map to `decoder.mid_block.resnets.{0,1}.conv{1,2}.weight`. Similarly for norm layers.
   f. **Decoder up-block remap:** Keys matching `decoder.up.{N}.block.{M}.conv{1,2}.weight` map to `decoder.up_blocks.{N}.resnets.{M}.conv{1,2}.weight`. Similarly for norm layers.
   g. **Decoder upsample:** Keys matching `decoder.up.{N}.block.{M}.conv_up.weight` map to `decoder.up_blocks.{N}.conv_upsample.weight`.
   h. **Final conv:** `decoder.conv_out.weight` stays as `decoder.conv_out.weight` (already in diffusers format). Also handle `decoder.conv_out.weight` norm if present: `decoder.conv_out.norm.weight` → `decoder.conv_out.norm.weight`.
   i. **Output norm:** `decoder.conv_out.weight` → `decoder.conv_out.weight` (no change needed for the main conv).

   Return a new dict with all remapped keys and their original tensor values.

3. **Rewrite `load_vae()` to use the new helpers.**

   Replace the current implementation:
   - Remove the import of `from diffusers.loaders.single_file_utils import convert_ldm_vae_checkpoint`.
   - Remove the hardcoded `AutoencoderKL(block_out_channels=[128, 256, 512, 512])` construction. Instead, call `_infer_vae_config_from_checkpoint(checkpoint)` first, then construct `AutoencoderKL` with the inferred `block_out_channels`, `latent_channels`, `in_channels`, `out_channels`, and `layers_per_block` (applying the `-1` offset already done in the helper).
   - Replace `convert_ldm_vae_checkpoint(checkpoint, config)` with `_remap_ldm_vae_keys(checkpoint)`.
   - Keep the `scaling_factor` as a hardcoded constant `0.18215` (the SD1.x default) — this is unconfirmable from shapes and is documented as best-effort in the task context. Note in the docstring that an incorrect value here produces visible brightness/contrast issues in decoded images, not a crash.
   - Keep the `model.vae_scale_factor = VAE_SCALE_FACTOR` assignment (currently `8`, which is the spatial compression factor, not the `scaling_factor` used in the latent formula `(latents / scaling_factor) + shift_factor`).

4. **Add unit tests.**

   In `worker/tests/test_arch_zit.py`, add:
   a. `test_infer_vae_config_from_checkpoint()` — build a synthetic checkpoint with keys matching the real VAE key format (`decoder.conv_in.weight [512, 16, 3, 3]`, `decoder.up.0.block.0.conv1.weight [128, 128, 3, 3]`, `decoder.up.1.block.0.conv1.weight [256, 128, 3, 3]`, `decoder.up.2.block.0.conv1.weight [512, 256, 3, 3]`, `decoder.up.3.block.0.conv1.weight [512, 512, 3, 3]`, `encoder.conv_in.weight [64, 3, 3, 3]`, `decoder.conv_out.weight [64, 64, 3, 3]`, and 3 resnet blocks per up-stage), call `_infer_vae_config_from_checkpoint`, and assert: `latent_channels == 16`, `block_out_channels == [128, 256, 512, 512]`, `in_channels == 3`, `out_channels == 3`, `layers_per_block == 2` (3 observed - 1 offset).
   b. `test_remap_ldm_vae_keys()` — build a synthetic checkpoint with LDM-format keys (`vae.decoder.conv_in.weight`, `vae.encoder.down.0.block.0.conv1.weight`, `vae.decoder.up.0.block.0.conv1.weight`, `vae.decoder.mid.block_1.conv1.weight`, `vae.decoder.conv_out.weight`), call `_remap_ldm_vae_keys`, and assert all keys are correctly remapped to diffusers format (no `vae.` prefix, correct block structure).

5. **Update the `test_no_diffusers_internal_import` test.**

   Extend the existing test (line 660) to also check that `convert_ldm_vae_checkpoint` does not appear in the source file. Currently it only checks for `convert_z_image_transformer_checkpoint_to_diffusers`.

## Public API Surface

No new public items. The two new helpers (`_infer_vae_config_from_checkpoint`, `_remap_ldm_vae_keys`) are private (underscore-prefixed, not in `__all__`). The `load_vae()` function signature remains unchanged: `def load_vae(model_id: str) -> Any`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/nodes/arch/diffusion/zit.py | Add `_infer_vae_config_from_checkpoint()`, `_remap_ldm_vae_keys()` helpers; rewrite `load_vae()` to use them; remove `convert_ldm_vae_checkpoint` import |
| MODIFY | worker/tests/test_arch_zit.py | Add `test_infer_vae_config_from_checkpoint()`, `test_remap_ldm_vae_keys()` tests; extend `test_no_diffusers_internal_import` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| worker/tests/test_arch_zit.py | test_infer_vae_config_from_checkpoint | Shape inference produces correct config: latent_channels=16, block_out_channels=[128,256,512,512], in_channels=3, out_channels=3, layers_per_block=2 (with -1 offset from 3 observed decoder resnets) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_infer_vae_config_from_checkpoint -v` exits 0 |
| worker/tests/test_arch_zit.py | test_remap_ldm_vae_keys | Key remap strips `vae.` prefix, converts LDM block structure to diffusers format for encoder down-blocks, decoder up-blocks, mid block, and final conv | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_remap_ldm_vae_keys -v` exits 0 |
| worker/tests/test_arch_zit.py | test_no_diffusers_internal_import (extended) | Verifies both `convert_z_image_transformer_checkpoint_to_diffusers` AND `convert_ldm_vae_checkpoint` are absent from zit.py source | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_no_diffusers_internal_import -v` exits 0 |

## CI Impact

No CI changes required. The new tests are added to the existing `test_arch_zit.py` file, which is already collected by `pytest worker/tests/ -v`. No new test files, no new CI jobs, no new dependencies.

## Platform Considerations

None identified. The task is pure Python logic — key string manipulation and tensor shape inspection — with no platform-specific code paths, no path separators, and no line-ending concerns. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The LDM key format in real VAE checkpoints may differ from the assumed `decoder.up.N.block.M.conv1.weight` pattern — the actual key structure depends on how the checkpoint was saved (ComfyUI vs. diffusers `state_dict()` vs. raw training output). | Medium | High | The task context confirms the key format from a full real VAE key scan. The implementation scans for the highest block index dynamically rather than hardcoding, and the test uses keys matching the confirmed real format. |
| The `-1` offset for `layers_per_block` applies only to decoder up-blocks, not encoder down-blocks. If the encoder also uses a different offset, the inferred config would be wrong. | Low | Medium | The task context explicitly states the offset is decoder-only (confirmed in diffusers' `vae.py` source). The implementation only applies the offset to the decoder block count. If encoder blocks are also needed for the config, they can be added later. |
| `scaling_factor` hardcoded to 0.18215 (SD1.x default) may be incorrect for Z-Image-Turbo's actual VAE. Wrong value produces visible brightness/contrast errors in decoded images, not a crash. | Low | Medium (visual only) | Documented as best-effort in the task context and in the function docstring. The value is the known SD1.x default and is the most likely candidate. Real-image visual inspection (outside this phase's CPU-only test scope) would be needed to fully validate. |
| Key remapping misses an edge-case key present in some VAE checkpoints (e.g., norm layers on the final conv, or `first_stage_model.` prefix variants). | Medium | Medium | The `_remap_ldm_vae_keys` function uses a copy of the checkpoint and only transforms known key patterns; unknown keys pass through unchanged (they won't match `load_state_dict` but also won't crash). The test covers the confirmed real key format. |

## Acceptance Criteria

- [ ] `grep -n "convert_ldm_vae_checkpoint" worker/nodes/arch/diffusion/zit.py` exits 1 (zero matches)
- [ ] `grep -n "layers_per_block.*=.*2\|layers_per_block=2" worker/nodes/arch/diffusion/zit.py` exits 0 (at least one match)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (all tests pass, including new ones)
- [ ] `python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/diffusion/zit.py').read_text(); assert '_infer_vae_config_from_checkpoint' in p"` exits 0
- [ ] `python3 -c "from pathlib import Path; p = Path('worker/nodes/arch/diffusion/zit.py').read_text(); assert '_remap_ldm_vae_keys' in p"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/arch/diffusion/zit.py` exits 0 (syntax check)
