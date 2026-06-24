# Plan Report: P904-Z1b

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-Z1b                                    |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/tests/real_fixtures.py: raw-checkpoint-format ZiT transformer and VAE fixtures (pre-remap keys) |
| Depends on  | P904-Z1                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-24T17:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add two pytest fixtures (`tiny_zit_transformer_raw` and `tiny_vae_raw`) to `worker/tests/real_fixtures.py` that produce synthetic `.safetensors` checkpoints in the raw, pre-remap key format — exactly the format that `load_transformer()` and `load_vae()` (P904-A10/A11) consume via their key-remap functions. This closes a critical detection gap: a fixture saving a model's own `state_dict()` would skip the remap/QKV-defuse path entirely, producing false test confidence. Both fixtures build a tiny model (dim=64 for the transformer, block_out_channels=(8,16) for the VAE), extract its `state_dict()`, then invert the known remap tables to produce raw-format checkpoints before saving.

## Scope

### In Scope
- Add `tiny_zit_transformer_raw(tmp_path)` fixture to `worker/tests/real_fixtures.py`:
  - Build `ZImageTransformer2DModel(dim=64, n_layers=2, n_heads=2, cap_feat_dim=64)` with zero-arg construction defaults matching the published architecture
  - Extract `state_dict()` from the constructed model
  - Invert the remap to raw-checkpoint format: fuse `to_q`/`to_k`/`to_v` into `qkv.weight` via `torch.cat`, rename `all_x_embedder.2-1.`/`all_final_layer.2-1.` back to `x_embedder.`/`final_layer.`, prepend `model.diffusion_model.` to every key
  - Save the raw-format state dict to a `.safetensors` file and return its path
- Add `tiny_vae_raw(tmp_path)` fixture to `worker/tests/real_fixtures.py`:
  - Build `AutoencoderKL(block_out_channels=(8,16), latent_channels=4)` with zero-arg construction defaults
  - Extract `state_dict()` from the constructed model
  - Invert the LDM remap to raw format: strip `down_blocks`/`resnets` structure back to `down`/`block` LDM-style keys, strip `mid_block.resnets` back to `mid.block_`, strip `up_blocks`/`resnets` back to `up`/`block`, add `vae.` prefix
  - Save the raw-format state dict to a `.safetensors` file and return its path
- Add unit tests verifying the inverse remap is correct (round-trip: raw → remap → raw) and that the fixtures produce valid safetensors files with expected key patterns

### Out of Scope
None. This task has `defers_to: []` (absent) and must implement its full scope. No deferrals.

## Existing Codebase Assessment

The `worker/tests/real_fixtures.py` file already contains three CLIP checkpoint fixtures (`tiny_qwen3_clip`, `tiny_clip_l_clip`, `tiny_t5_clip`) that build tiny models, call `state_dict()`, and save to `.safetensors`. These fixtures use native state_dict format — no inverse remapping is needed because the CLIP loaders call `load_state_dict()` directly on whatever is saved, with no key-remap step.

The `worker/nodes/arch/diffusion/zit.py` file (1040 lines) already implements the forward remap functions `_remap_z_image_keys()` and `_remap_ldm_vae_keys()` that convert raw checkpoints to diffusers convention. These functions were derived from the diffusers source and are used by `load_transformer()` and `load_vae()`. The inverse remap for this task must be the exact logical complement of these functions.

The test file `worker/tests/test_arch_zit.py` (1053 lines) already tests the forward remap functions extensively with synthetic checkpoints, using `torch.ones()` for tensor values. The test patterns (lazy torch imports, `tmp_path` fixture usage, assertion style) provide the established convention to follow.

The `worker/tests/conftest.py` sets `ANVILML_WORKER_MOCK=1` via autouse fixture. The new fixtures must preserve lazy-import isolation (torch imported only inside the fixture body, not at module level) so that mock-mode tests can still import the module without torch.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0 (minimum) | pypi-query MCP | n/a |
| python | torch   | (from cpu-linux-agent.txt) | pypi-query MCP | n/a |
| python | safetensors | (from base.txt) | pypi-query MCP | n/a |

No new dependencies introduced. All packages already declared in `worker/requirements/base.txt` or `worker/requirements/cpu-linux-agent.txt`.

**API verification for diffusers 0.38.0 remap functions:**
- `convert_z_image_transformer_checkpoint_to_diffusers(checkpoint)` — confirmed at line 3938 of `diffusers/loaders/single_file_utils.py` in the installed `.venv`. Uses `Z_IMAGE_KEYS_RENAME_DICT` (same 7-entry rename table as zit.py's `_remap_z_image_keys`) and `convert_z_image_fused_attention` for QKV defuse.
- `convert_ldm_vae_checkpoint(checkpoint, config)` — confirmed at line 1471. Uses `DIFFUSERS_TO_LDM_MAPPING["vae"]` (lines 311-327) for direct key mappings and `update_vae_resnet_ldm_to_diffusers` for block remapping.
- `ZImageTransformer2DModel()` — zero-arg construction uses registered defaults matching the published 6B ZiT config (`dim=3840, n_layers=30, n_heads=30, n_kv_heads=30, cap_feat_dim=2560`). Passing `dim=64, n_layers=2, n_heads=2, cap_feat_dim=64` produces a valid tiny model.
- `AutoencoderKL(block_out_channels=(8,16), latent_channels=4)` — valid construction with the specified parameters.

## Approach

### Step 1: Build the inverse ZiT remap function

Add `_invert_z_image_keys(state_dict: dict) -> dict` to `real_fixtures.py`. This function takes a diffusers-convention state dict (the output of `model.state_dict()`) and produces a raw-checkpoint state dict. It is the exact inverse of `_remap_z_image_keys` in `zit.py`.

The inverse mapping is derived by reading `convert_z_image_transformer_checkpoint_to_diffusers`'s own `Z_IMAGE_KEYS_RENAME_DICT` (lines 3939-3947 of diffusers source) and reversing each entry:

Forward (diffusers → raw):
```
"final_layer." → "all_final_layer.2-1."
"x_embedder." → "all_x_embedder.2-1."
".attention.out.bias" → ".attention.to_out.0.bias"
".attention.k_norm.weight" → ".attention.norm_k.weight"
".attention.q_norm.weight" → ".attention.norm_q.weight"
".attention.out.weight" → ".attention.to_out.0.weight"
"model.diffusion_model." → ""
```

Inverse (diffusers state_dict → raw checkpoint):
```
"all_final_layer.2-1." → "final_layer."
"all_x_embedder.2-1." → "x_embedder."
".attention.to_out.0.bias" → ".attention.out.bias"
".attention.norm_k.weight" → ".attention.k_norm.weight"
".attention.norm_q.weight" → ".attention.q_norm.weight"
".attention.to_out.0.weight" → ".attention.out.weight"
"" → "model.diffusion_model." (prepend to every key)
```

Additionally, the inverse must handle QKV defuse in reverse: for every key containing `.attention.to_q.weight`, `.attention.to_k.weight`, or `.attention.to_v.weight`, concatenate the three tensors along dim 0 via `torch.cat([to_q, to_k, to_v], dim=0)` into a single `.attention.qkv.weight` tensor, and remove the three individual keys.

Implementation:
1. Copy the input state dict (never mutate).
2. First, handle QKV defuse in reverse: scan for keys matching `*.attention.to_q.weight` (and corresponding to_k/to_v). For each group, `torch.cat` them into `qkv.weight` and remove the three originals. This must happen BEFORE prefix renaming so that the fused key gets the correct prefix.
3. Apply the inverse rename table (string replacement, same order as forward).
4. Prepend `model.diffusion_model.` to every key.

### Step 2: Build the inverse VAE remap function

Add `_invert_ldm_vae_keys(state_dict: dict) -> dict` to `real_fixtures.py`. This inverts `_remap_ldm_vae_keys` from `zit.py`.

The diffusers source's `DIFFUSERS_TO_LDM_MAPPING["vae"]` (lines 311-327) maps diffusers keys → LDM keys. The inverse must go the other way:

Direct key mappings (from diffusers source, reversed):
```
"encoder.conv_in.weight" → "vae.encoder.conv_in.weight"
"encoder.conv_in.bias" → "vae.encoder.conv_in.bias"
"encoder.conv_out.weight" → "vae.encoder.conv_out.weight"
"encoder.conv_out.bias" → "vae.encoder.conv_out.bias"
"encoder.conv_norm_out.weight" → "vae.encoder.norm_out.weight"
"encoder.conv_norm_out.bias" → "vae.encoder.norm_out.bias"
"decoder.conv_in.weight" → "vae.decoder.conv_in.weight"
"decoder.conv_in.bias" → "vae.decoder.conv_in.bias"
"decoder.conv_out.weight" → "vae.decoder.conv_out.weight"
"decoder.conv_out.bias" → "vae.decoder.conv_out.bias"
"decoder.conv_norm_out.weight" → "vae.decoder.norm_out.weight"
"decoder.conv_norm_out.bias" → "vae.decoder.norm_out.bias"
"quant_conv.weight" → "vae.quant_conv.weight"
"quant_conv.bias" → "vae.quant_conv.bias"
"post_quant_conv.weight" → "vae.post_quant_conv.weight"
"post_quant_conv.bias" → "vae.post_quant_conv.bias"
```

Block structure remapping (reversing `_remap_ldm_vae_keys` regex patterns):
```
"encoder.down_blocks.{N}.resnets.{M}.conv1.weight" → "vae.encoder.down.{N}.block.{M}.conv1.weight"
"encoder.down_blocks.{N}.resnets.{M}.conv2.weight" → "vae.encoder.down.{N}.block.{M}.conv2.weight"
"encoder.down_blocks.{N}.resnets.{M}.conv1.norm1.weight" → "vae.encoder.down.{N}.block.{M}.conv1.norm1.weight"
"encoder.down_blocks.{N}.resnets.{M}.conv2.norm2.weight" → "vae.encoder.down.{N}.block.{M}.conv2.norm2.weight"
"encoder.down_blocks.{N}.downsamplers.0.conv.weight" → "vae.encoder.down.{N}.downsample.conv.weight"
"encoder.down_blocks.{N}.downsamplers.0.conv.bias" → "vae.encoder.down.{N}.downsample.conv.bias"
"decoder.mid_block.resnets.{0,1}.conv1.weight" → "vae.decoder.mid.block_{1,2}.conv1.weight"
"decoder.mid_block.resnets.{0,1}.conv2.weight" → "vae.decoder.mid.block_{1,2}.conv2.weight"
"decoder.up_blocks.{N}.resnets.{M}.conv1.weight" → "vae.decoder.up.{N}.block.{M}.conv1.weight"
"decoder.up_blocks.{N}.resnets.{M}.conv2.weight" → "vae.decoder.up.{N}.block.{M}.conv2.weight"
"decoder.up_blocks.{N}.resnets.{M}.conv1.norm1.weight" → "vae.decoder.up.{N}.block.{M}.conv1.norm1.weight"
"decoder.up_blocks.{N}.resnets.{M}.conv2.norm2.weight" → "vae.decoder.up.{N}.block.{M}.conv2.norm2.weight"
"decoder.up_blocks.{N}.conv_upsample.weight" → "vae.decoder.up.{N}.block.{M}.conv_up.weight"
```

Implementation:
1. Copy the input state dict.
2. For each key, apply regex-based reverse mapping:
   - If key matches `encoder.down_blocks.{N}.resnets.{M}.conv[12].weight` → `vae.encoder.down.{N}.block.{M}.conv{1,2}.weight`
   - If key matches `encoder.down_blocks.{N}.resnets.{M}.conv[12].norm[12].weight` → `vae.encoder.down.{N}.block.{M}.conv{1,2}.norm{1,2}.weight`
   - If key matches `decoder.mid_block.resnets.{0,1}.conv[12].weight` → `vae.decoder.mid.block_{1,2}.conv{1,2}.weight`
   - If key matches `decoder.up_blocks.{N}.resnets.{M}.conv[12].weight` → `vae.decoder.up.{N}.block.{M}.conv{1,2}.weight`
   - If key matches `decoder.up_blocks.{N}.conv_upsample.weight` → `vae.decoder.up.{N}.block.{M}.conv_up.weight`
3. For keys that don't match any block pattern, prepend `vae.` prefix.

### Step 3: Implement `tiny_zit_transformer_raw` fixture

```python
def tiny_zit_transformer_raw(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny ZiT transformer checkpoint in raw (pre-remap) format.
    
    Constructs a ZImageTransformer2DModel with dim=64, n_layers=2, n_heads=2,
    cap_feat_dim=64, extracts its state_dict(), inverts the diffusers key-remap
    to produce raw ComfyUI-format keys (fused QKV, model.diffusion_model. prefix,
    x_embedder/final_layer naming), and saves to .safetensors.
    
    Args:
        tmp_path: Pytest tmp_path fixture.
    
    Returns:
        Path to the saved tiny_zit_transformer_raw.safetensors file.
    """
    import torch
    from diffusers import ZImageTransformer2DModel
    from safetensors.torch import save_file
    
    # Construct the tiny model — only dim/n_layers/n_heads/cap_feat_dim
    # are overridden; all other parameters use registered defaults.
    # This matches the pattern used by load_transformer() which
    # zero-arg-constructs and then overrides inferred values.
    model = ZImageTransformer2DModel(
        dim=64,
        n_layers=2,
        n_heads=2,
        cap_feat_dim=64,
    )
    
    # Get the diffusers-convention state dict.
    state_dict = model.state_dict()
    
    # Invert the remap to raw-checkpoint format.
    raw = _invert_z_image_keys(state_dict)
    
    # Save and return path.
    output_path = tmp_path / "tiny_zit_transformer_raw.safetensors"
    save_file(raw, str(output_path))
    return output_path
```

### Step 4: Implement `tiny_vae_raw` fixture

```python
def tiny_vae_raw(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a tiny VAE checkpoint in raw LDM format.
    
    Constructs an AutoencoderKL with block_out_channels=(8,16), latent_channels=4,
    extracts its state_dict(), inverts the diffusers key-remap to produce raw
    LDM-format keys (vae. prefix, down/up block structure), and saves to .safetensors.
    
    Args:
        tmp_path: Pytest tmp_path fixture.
    
    Returns:
        Path to the saved tiny_vae_raw.safetensors file.
    """
    import torch
    from diffusers import AutoencoderKL
    from safetensors.torch import save_file
    
    # Construct the tiny VAE with the specified config.
    # block_out_channels=(8,16) gives 2 stages; latent_channels=4 is standard.
    model = AutoencoderKL(
        block_out_channels=(8, 16),
        latent_channels=4,
    )
    
    # Get the diffusers-convention state dict.
    state_dict = model.state_dict()
    
    # Invert the remap to raw LDM format.
    raw = _invert_ldm_vae_keys(state_dict)
    
    # Save and return path.
    output_path = tmp_path / "tiny_vae_raw.safetensors"
    save_file(raw, str(output_path))
    return output_path
```

### Step 5: Add round-trip verification tests

Add two tests to `real_fixtures.py`:

1. `test_zit_transformer_raw_roundtrip`: Load the raw checkpoint file, apply `_remap_z_image_keys` (the forward remap), then apply `_invert_z_image_keys` (the inverse). Assert the final state dict keys match the original model's state dict keys exactly. This verifies the inverse is an exact complement of the forward remap.

2. `test_vae_raw_roundtrip`: Same pattern — load raw checkpoint, apply forward `_remap_ldm_vae_keys`, then inverse `_invert_ldm_vae_keys`, assert keys match the original model's state dict keys.

3. `test_zit_transformer_raw_has_raw_key_patterns`: Load the raw file with `safetensors.torch.load_file` and assert it contains expected raw-format key patterns: keys starting with `model.diffusion_model.`, at least one `.attention.qkv.weight` (fused), and `x_embedder.`/`final_layer.` naming (not `all_x_embedder.2-1.`/`all_final_layer.2-1.`).

4. `test_vae_raw_has_raw_key_patterns`: Load the raw file and assert it contains `vae.` prefix, `encoder.down.` LDM-style keys, and `decoder.up.` LDM-style keys.

### Step 6: Update module docstring

Update the module docstring to mention the two new raw-format fixtures alongside the existing CLIP fixtures.

## Public API Surface

New items in `worker/tests/real_fixtures.py`:

```python
def _invert_z_image_keys(state_dict: dict[str, Any]) -> dict[str, Any]
def _invert_ldm_vae_keys(state_dict: dict[str, Any]) -> dict[str, Any]
def tiny_zit_transformer_raw(tmp_path: pathlib.Path) -> pathlib.Path
def tiny_vae_raw(tmp_path: pathlib.Path) -> pathlib.Path
```

All four are module-level functions. The two `_invert_*` functions are private (underscore-prefixed). The two fixtures are pytest fixtures (not `pub` in the Rust sense, but callable Python functions).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/tests/real_fixtures.py` | Add `_invert_z_image_keys`, `_invert_ldm_vae_keys`, `tiny_zit_transformer_raw`, `tiny_vae_raw` fixtures and round-trip verification tests; update module docstring |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|---------------------|
| `worker/tests/real_fixtures.py` | `test_zit_transformer_raw_roundtrip` | The inverse ZiT remap is the exact complement of the forward remap: raw → forward → raw produces matching keys | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_zit_transformer_raw_roundtrip -v` exits 0 |
| `worker/tests/real_fixtures.py` | `test_vae_raw_roundtrip` | The inverse VAE remap is the exact complement of the forward remap | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_vae_raw_roundtrip -v` exits 0 |
| `worker/tests/real_fixtures.py` | `test_zit_transformer_raw_has_raw_key_patterns` | The raw checkpoint file contains `model.diffusion_model.` prefix, fused `.attention.qkv.weight`, and `x_embedder.`/`final_layer.` naming | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_zit_transformer_raw_has_raw_key_patterns -v` exits 0 |
| `worker/tests/real_fixtures.py` | `test_vae_raw_has_raw_key_patterns` | The raw VAE checkpoint file contains `vae.` prefix and LDM-style `encoder.down.`/`decoder.up.` keys | `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_vae_raw_has_raw_key_patterns -v` exits 0 |

## CI Impact

No CI changes required. The new fixtures are in `worker/tests/real_fixtures.py` which is already collected by `pytest worker/tests/`. The new tests require torch (real-mode CPU venv) and will be skipped in mock mode because torch is absent from the CI venv. No new CI jobs or gates are needed.

## Platform Considerations

None identified. The `torch.cat` and `torch.chunk` operations used in the inverse remaps are cross-platform standard PyTorch operations. Path handling uses `pathlib.Path` which is cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| The ZImageTransformer2DModel's `state_dict()` key naming may differ from what `_remap_z_image_keys` expects — the registered defaults in diffusers 0.38.0 may use different key patterns than what the remap function handles (e.g. missing `norm_final.weight`, different attention sub-key naming) | Medium | High | Build the tiny model, inspect its actual `state_dict()` keys before writing the inverse remap; verify every key that the forward remap touches is present in the model's state dict. If a key is missing, the inverse has nothing to invert for that key — that's fine, the roundtrip test will catch mismatches. |
| The VAE's `state_dict()` key naming may include keys not covered by the LDM remap (e.g. `quant_conv`, `post_quant_conv` which are in `DIFFUSERS_TO_LDM_MAPPING["vae"]` but have simple 1:1 mappings) | Low | Medium | These keys don't need block structure remapping — they just get the `vae.` prefix prepended. The inverse function handles them as a catch-all: any key not matching a block pattern gets `vae.` prepended. The roundtrip test will catch any missing keys. |
| QKV defuse reversal may produce incorrect tensor shapes if the model has asymmetric to_q/to_k/to_v dimensions (e.g. GQA with different head counts) | Low | High | The tiny model uses `n_heads=2, n_kv_heads=2` (no GQA), so all three tensors have equal dimension and `torch.cat(..., dim=0)` into 3x dimension is correct. The roundtrip test verifies the defused keys round-trip correctly. |
| The inverse VAE remap regex patterns may not cover all key variants produced by the diffusers AutoencoderKL (e.g. bias keys, norm keys in different positions) | Medium | Medium | Build the model, inspect the full `state_dict()`, and ensure every key pattern is covered by at least one regex branch. The catch-all `vae.` prefix handles any unrecognized keys. The roundtrip test is the definitive check. |

## Acceptance Criteria

- [ ] `python3 -m py_compile worker/tests/real_fixtures.py` exits 0 (syntax check before test run)
- [ ] `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py --collect-only` exits 0 (fixture collection without error)
- [ ] `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_zit_transformer_raw_roundtrip -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_vae_raw_roundtrip -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_zit_transformer_raw_has_raw_key_patterns -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=0 worker/.venv-cpu-agent/bin/python -m pytest worker/tests/real_fixtures.py::test_vae_raw_has_raw_key_patterns -v` exits 0
- [ ] `python3 -c "
import os, pathlib
os.environ['ANVILML_WORKER_MOCK'] = '0'
from worker.tests.real_fixtures import tiny_zit_transformer_raw, tiny_vae_raw, _invert_z_image_keys, _invert_ldm_vae_keys
print('All symbols importable')
"` exits 0 (all new symbols are importable without torch at import time)
- [ ] `grep -n "model.diffusion_model" worker/tests/real_fixtures.py` returns at least one match (inverse ZiT remap prepends the prefix)
- [ ] `grep -n "vae\." worker/tests/real_fixtures.py` returns at least one match (inverse VAE remap prepends the prefix)
