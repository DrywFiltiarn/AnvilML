# Plan Report: P904-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-B1                                       |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/arch/diffusion/zit.py: load_transformer() — replace diffusers-internals reuse with shape-inferred config + manual key remap |
| Depends on  | P904-A14                                      |
| Project     | anvilml                                       |
| Planned at  | 2026-06-24T14:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Rewrite `load_transformer()` in `worker/nodes/arch/diffusion/zit.py` to remove its dependency on the private, unversioned `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers` function. Instead, infer the `ZImageTransformer2DModel` configuration directly from the raw checkpoint's tensor shapes (the ComfyUI pattern), construct the model with explicit shape-derived parameters, and perform the key remap and QKV-defuse logic manually. The function must continue to perform zero network calls and behave identically in mock mode.

## Scope

### In Scope
- Rewrite `load_transformer()` in `worker/nodes/arch/diffusion/zit.py` to:
  - Infer `dim`, `head_dim`, `n_heads`, `n_kv_heads`, `n_layers`, `n_refiner_layers`, `cap_feat_dim`, `in_channels`, `all_patch_size`, and `all_f_patch_size` directly from raw checkpoint tensor shapes.
  - Construct `ZImageTransformer2DModel(dim=..., in_channels=..., n_layers=..., n_refiner_layers=..., n_heads=..., n_kv_heads=..., cap_feat_dim=..., all_patch_size=..., all_f_patch_size=...)` with all six inferred parameters explicitly passed.
  - Keep `norm_eps=1e-5`, `rope_theta=256.0`, `t_scale=1000.0`, `axes_dims=[32,48,48]`, `axes_lens=[1024,512,512]`, and `qk_norm=True` as hardcoded constants (these are never stored as weights).
  - Remap keys from the raw checkpoint format to diffusers convention using a local key-remap dictionary and QKV-defuse logic, replacing `convert_z_image_transformer_checkpoint_to_diffusers`.
  - Remove the import of `convert_z_image_transformer_checkpoint_to_diffusers` from `diffusers.loaders.single_file_utils`.
- Preserve mock-mode behavior: `ANVILML_WORKER_MOCK=1` returns `None` immediately, no imports of torch/diffusers/safetensors.
- Preserve all existing docstrings, logging, and error handling patterns in `load_transformer()`.

### Out of Scope
None. This task implements its full scope. `defers_to (from JSON): absent`.

## Existing Codebase Assessment

The `load_transformer()` function in `zit.py` (lines 214–293) currently follows this pattern:
1. Checks `ANVILML_WORKER_MOCK` env var; returns `None` in mock mode.
2. Lazy-imports `ZImageTransformer2DModel`, `convert_z_image_transformer_checkpoint_to_diffusers` (from `diffusers.loaders.single_file_utils`), and `safetensors.torch.load_file`.
3. Constructs `ZImageTransformer2DModel()` with zero arguments, relying on the class's `@register_to_config` registered defaults (`dim=3840`, `n_layers=30`, `n_heads=30`, `cap_feat_dim=2560`) to match the published 6B ZiT architecture.
4. Loads the raw checkpoint via `safetensors_load_file(model_id)`.
5. Remaps keys via `convert_z_image_transformer_checkpoint_to_diffusers(checkpoint)`.
6. Calls `model.load_state_dict(remapped)` and returns the model.

The existing test file (`worker/tests/test_arch_zit.py`) has two tests for `load_transformer`: `test_load_transformer_is_callable` (mock mode, checks `callable(load_transformer)`) and `test_sample_real_assembles_pipeline_via_cache` / `test_sample_real_invokes_pipeline_with_correct_args` (real mode, but mock the pipeline cache so they never reach `load_transformer`). There are no tests that exercise the real loading path — the task context notes that real-mode testing is covered by the Group Z real-mode test suite (P904-Z1b, P904-Z3).

The diffusers source for `convert_z_image_transformer_checkpoint_to_diffusers` (diffusers 0.38.0, `single_file_utils.py` line 3938) performs three operations: (a) a key-remap dictionary replacing `final_layer.` → `all_final_layer.2-1.`, `x_embedder.` → `all_x_embedder.2-1.`, `.attention.out.*` → `.attention.to_out.0.*`, `.attention.k_norm.*` → `.attention.norm_k.*`, `.attention.q_norm.*` → `.attention.norm_q.*`, and stripping `model.diffusion_model.` prefix; (b) removal of `norm_final.weight` if present; (c) a special handler that fuses `qkv.weight` into separate `to_q.weight`, `to_k.weight`, `to_v.weight` via `torch.chunk(..., 3, dim=0)`.

The `ZImageTransformer2DModel.__init__` signature (diffusers 0.38.0, `transformer_z_image.py` line 366) accepts 15 parameters, of which 6 are shape-inferable from checkpoint tensors and 9 are either defaults or scalar hyperparameters not stored as weights.

No gap exists between the design doc and current source that affects this task — the task context's shape-inference values have been independently verified against real checkpoint data and confirmed correct.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0        | pypi-query MCP | n/a                    |

The task removes a dependency on `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers` — a private internal function, not a public API surface. No new external dependencies are introduced. The diffusers version is confirmed at 0.38.0 via the MCP tool, matching the project's `worker/requirements/base.txt` pin of `diffusers>=0.38.0`.

The `ZImageTransformer2DModel` class and its `@register_to_config`-decorated `__init__` were verified against the live diffusers 0.38.0 source on GitHub (path: `src/diffusers/models/transformers/transformer_z_image.py`, line 359). The full 15-parameter signature was confirmed.

The key-remap logic was verified against the live diffusers 0.38.0 source (`src/diffusers/loaders/single_file_utils.py`, line 3938), confirming the exact rename dictionary and QKV-defuse handler that must be replicated manually.

## Approach

**Step 1 — Understand the remap contract from diffusers source.**

Read the `convert_z_image_transformer_checkpoint_to_diffusers` function from the diffusers 0.38.0 source (confirmed at `single_file_utils.py` line 3938). The function performs three operations:
- **Key remap dictionary** (`Z_IMAGE_KEYS_RENAME_DICT`): sequential string replacement per key. Five entries: `final_layer.` → `all_final_layer.2-1.`, `x_embedder.` → `all_x_embedder.2-1.`, `.attention.out.bias` → `.attention.to_out.0.bias`, `.attention.k_norm.weight` → `.attention.norm_k.weight`, `.attention.q_norm.weight` → `.attention.norm_q.weight`, `.attention.out.weight` → `.attention.to_out.0.weight`, and `model.diffusion_model.` → `` (empty string).
- **`norm_final.weight` removal**: if present, pop it from the state dict.
- **QKV-defuse handler** (`TRANSFORMER_SPECIAL_KEYS_REMAP`): for any key containing `.attention.qkv.weight`, pop the fused tensor and split it via `torch.chunk(fused_qkv_weight, 3, dim=0)` into three separate tensors named `to_q.weight`, `to_k.weight`, `to_v.weight`.

This logic must be replicated exactly in the new implementation.

**Step 2 — Implement shape inference for config parameters.**

For each parameter, derive from checkpoint tensor shapes:

- `dim`: Read `attention.out.weight` shape, take index `[0]` (first dimension). Example: `[3840, 3840]` → `dim = 3840`.
- `head_dim`: Read `attention.q_norm.weight` shape, take index `[0]`. Example: `[128]` → `head_dim = 128`.
- `n_heads`: Compute `dim // head_dim`. Example: `3840 // 128 = 30`.
- `n_kv_heads`: Compute `dim // head_dim` (same as `n_heads` for ZiT — no GQA). The task context confirms both are 30.
- `n_layers`: Count the number of `layers.N.` key prefixes present in the checkpoint. Scan all keys, collect unique `N` values from keys matching `layers.\d+\.` pattern, take the maximum. Example: `layers.0` through `layers.29` → `n_layers = 30`.
- `n_refiner_layers`: Count the number of `context_refiner.N.` key prefixes (or equivalently `noise_refiner.N.`). Take the maximum `N` and add 1. Example: `context_refiner.0` and `context_refiner.1` → `n_refiner_layers = 2`.
- `cap_feat_dim`: Read `cap_embedder.0.weight` shape, take index `[0]`. Example: `[2560]` → `cap_feat_dim = 2560`.
- `in_channels`: Read `final_layer.linear.weight` shape, take index `[0]`, then divide by `patch_size**2 * f_patch_size`. The registered defaults are `all_patch_size=(2,)` and `all_f_patch_size=(1,)`, so `in_channels = final_layer.linear.weight.shape[0] // (2**2 * 1)`. Example: `64 // 4 = 16`. **This is the corrected derivation** — the earlier draft claimed `final_layer.linear.weight.shape[0]` equals `in_channels` directly, which is wrong.
- `all_patch_size` and `all_f_patch_size`: Use the registered defaults `(2,)` and `(1,)` respectively. These are tuple parameters that cannot be reliably derived from a single checkpoint scan (they describe a multi-scale architecture), and the default matches the ZiT architecture.

**Step 3 — Implement the manual key remap function.**

Create a local function `_remap_z_image_keys(checkpoint: dict) -> dict` that:
1. Copies all keys from `checkpoint` into a new dict.
2. Applies the key-remap dictionary (same 7 replacements as diffusers' `Z_IMAGE_KEYS_RENAME_DICT`, applied sequentially per key).
3. Removes `norm_final.weight` if present.
4. Applies the QKV-defuse handler: for any key containing `.attention.qkv.weight`, pop the fused tensor and split it into three via `torch.chunk(..., 3, dim=0)`, naming them `to_q.weight`, `to_k.weight`, `to_v.weight` (same replacement pattern as diffusers).

The function must return the remapped state dict. It operates on a copy, not in-place on the original checkpoint (to match the existing `load_transformer` behavior where `checkpoint` is not mutated by the remap function — the current `convert_z_image_transformer_checkpoint_to_diffusers` pops from its input, but since we load via `safetensors_load_file` which returns a fresh dict, mutation is acceptable; however, for safety and clarity, we operate on a copy).

**Step 4 — Rewrite `load_transformer()` body.**

Replace the function body (lines 253–293) with:
1. Mock-mode check (unchanged).
2. Lazy imports of `torch` (needed for `torch.chunk` in the remap function), `ZImageTransformer2DModel`, and `safetensors_load_file`.
3. Load checkpoint via `safetensors_load_file(model_id)`.
4. Call the new shape-inference logic to derive config parameters.
5. Construct `ZImageTransformer2DModel` with the six inferred parameters explicitly passed, plus the hardcoded scalar constants for the remaining parameters.
6. Call the new `_remap_z_image_keys(checkpoint)` to remap keys.
7. Call `model.load_state_dict(remapped)`.
8. Return the model.

**Step 5 — Verify the import of `convert_z_image_transformer_checkpoint_to_diffusers` is removed.**

The import at line 264-266 must be deleted. No other file in the codebase imports this function — it is only used in `load_transformer()`.

**Step 6 — Preserve docstrings and error handling.**

The existing docstring for `load_transformer()` describes the function's behavior, parameters, return value, and exceptions. Update it to reflect that the function now uses shape-inferred config rather than registered defaults, and that no diffusers internal functions are called. Keep the "zero network calls" and "mock mode" descriptions unchanged.

## Public API Surface

No new public items are introduced. The public API surface of `load_transformer()` remains identical:

```python
def load_transformer(model_id: str) -> Any:
    """Load a Z-Image Turbo (ZiT) transformer from a raw .safetensors file."""
```

Return type: `ZImageTransformer2DModel` instance (or `None` in mock mode). The `Any` return type in the current signature is preserved.

The private helper `_remap_z_image_keys()` is module-private (underscore-prefixed, not in `__all__`) and is not a public API item.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Rewrite `load_transformer()`: replace diffusers-internal key remap with shape-inferred config + manual key remap; remove `convert_z_image_transformer_checkpoint_to_diffusers` import; add `_remap_z_image_keys()` helper; update docstring |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_load_transformer_is_callable` | `load_transformer` is callable in mock mode (unchanged behavior) | `ANVILML_WORKER_MOCK=1` (conftest.py autouse fixture) | Import `load_transformer` from module | `callable(load_transformer) == True` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_transformer_is_callable -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_no_diffusers_internal_import` | The import of `convert_z_image_transformer_checkpoint_to_diffusers` is removed from zit.py | Source file inspection | Read `zit.py` source | `grep "convert_z_image_transformer_checkpoint_to_diffusers" zit.py` returns zero matches | `grep -c "convert_z_image_transformer_checkpoint_to_diffusers" worker/nodes/arch/diffusion/zit.py; test $? -ne 0` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_remap_key_transformations` | The manual key remap produces correct diffusers-convention keys for a synthetic checkpoint | `ANVILML_WORKER_MOCK=1` | Call `_remap_z_image_keys()` with a dict containing raw-format keys | Remapped keys match diffusers convention (e.g., `model.diffusion_model.layers.0.attn.qkv.weight` → `layers.0.attn.to_q.weight` with fused QKV split) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_remap_key_transformations -v` exits 0 |

## CI Impact

No CI changes required. The task modifies only a Python source file in the worker module. The existing mock-mode test suite (`worker/tests/`) continues to exercise the mock path of `load_transformer()`, which is unaffected by this change. No new test files are created, no CI configuration is modified, and no Rust-side changes are introduced.

## Platform Considerations

None identified. The task is a pure Python implementation with no platform-specific code paths. `torch.chunk` operates identically on CPU tensors across Linux and Windows. The shape inference logic reads tensor shapes (integers) and performs arithmetic — fully platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The manual key remap may produce a different key set than `convert_z_image_transformer_checkpoint_to_diffusers` for edge cases (e.g., keys with multiple prefix matches that interact, or keys not present in the Z_IMAGE_KEYS_RENAME_DICT but affected by the QKV-defuse handler). This would cause `load_state_dict` to fail with unrecognised key errors. | Medium | High | Write a unit test (`test_remap_key_transformations`) that constructs a synthetic checkpoint with every key type present in the ZiT architecture (x_embedder, layers with qkv/out/q_norm/k_norm, final_layer, cap_embedder, noise_refiner, context_refiner, t_embedder, norm_final) and asserts the remapped keys match the expected diffusers convention exactly. The test uses the same key-remap dictionary from the diffusers source, applied to a known set of raw keys. |
| Shape inference for `n_layers` and `n_refiner_layers` may fail if the checkpoint has non-standard layer naming (e.g., `blocks.0` instead of `layers.0`). This would produce incorrect model dimensions and cause `load_state_dict` to fail with shape mismatch errors. | Low | High | The task context confirms the real checkpoint uses `layers.N.` and `context_refiner.N.`/`noise_refiner.N.` prefixes. The shape inference uses these exact patterns. If a future checkpoint uses different prefixes, the error would be a `load_state_dict` shape mismatch (not a silent failure), making it immediately diagnosable. |
| The `norm_final.weight` removal may be too aggressive — if a future ZiT variant stores this key for a valid purpose, removing it would silently discard weights. | Low | Medium | The diffusers 0.38.0 source removes this key unconditionally (line 3981-3982 of `single_file_utils.py`). We replicate this exact behavior. If a future variant needs it, the fix is a one-line conditional guard. |
| The QKV-defuse split using `torch.chunk(fused_qkv_weight, 3, dim=0)` may produce tensors of slightly different shapes if the fused weight's first dimension is not divisible by 3. This would cause `load_state_dict` to fail. | Low | High | The task context confirms `*.attention.qkv.weight` is `[11520, 3840]` and `11520 / 3 = 3840 = dim` — cleanly divisible. The shape inference for `n_heads` uses the same division (`dim // head_dim`), so if it fails, the model construction would fail first with a different error, making this risk secondary. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (no regressions in existing mock-mode tests)
- [ ] `grep -c "convert_z_image_transformer_checkpoint_to_diffusers" worker/nodes/arch/diffusion/zit.py` returns 0 (private diffusers internal no longer imported)
- [ ] `grep -c "in_channels.*=.*16\|in_channels=16" worker/nodes/arch/diffusion/zit.py` returns > 0 (corrected in_channels value is used)
- [ ] `python3 -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion.zit import load_transformer; assert callable(load_transformer)"` exits 0 (function is still callable in mock mode)
- [ ] `python3 -c "
import ast, sys
src = open('worker/nodes/arch/diffusion/zit.py').read()
tree = ast.parse(src)
# Check that load_transformer body does not contain 'convert_z_image_transformer_checkpoint_to_diffusers'
assert 'convert_z_image_transformer_checkpoint_to_diffusers' not in src
print('OK')
"` exits 0 (no residual reference to the removed function)
