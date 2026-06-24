# Plan Report: P904-A11

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P904-A11                                                    |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)       |
| Description | worker/nodes/arch/diffusion/zit.py: add load_vae() — offline VAE loading, no HF network access |
| Depends on  | P904-A10                                                    |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-24T11:30:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Add `load_vae(model_id: str) -> Any` to `worker/nodes/arch/diffusion/zit.py`, mirroring the pattern established by `load_transformer()` (P904-A10). This function loads a VAE (`AutoencoderKL`) from a raw `.safetensors` checkpoint file using local-only operations — no HuggingFace network access — by constructing the model with zero-argument defaults matching the published Z-Image Turbo architecture config, loading weights via `safetensors.torch.load_file()`, and remapping keys via `diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint()`. The function is added to `__all__` so it can be consumed by `LoadVae.execute()` (P904-A12) and tested alongside the existing `load_transformer` tests.

## Scope

### In Scope
- Add `load_vae(model_id: str) -> Any` function to `worker/nodes/arch/diffusion/zit.py`
- Add `"load_vae"` to `__all__` in `worker/nodes/arch/diffusion/zit.py`
- Add `test_load_vae_is_callable()` test to `worker/tests/test_arch_zit.py` (mock-mode importability test, following the `test_load_transformer_is_callable` pattern)
- Import `load_vae` in `worker/tests/test_arch_zit.py` test file's import block

### Out of Scope
- None. The `defers_to (from JSON): absent` — this task must implement its full scope. No stubs, no deferred functionality.

## Existing Codebase Assessment

The `zit.py` module already contains `load_transformer()` (added by P904-A10), which follows a well-established pattern: mock-mode check via `os.environ.get("ANVILML_WORKER_MOCK")`, lazy imports of `diffusers`/`safetensors`/`torch` inside the real-mode branch, model construction with zero arguments using registered defaults, raw checkpoint loading via `safetensors.torch.load_file()`, key remapping via a `diffusers.loaders.single_file_utils` conversion function, and `model.load_state_dict()` to apply weights. The module uses `os.environ` for mock detection (not a module-level flag), and all heavy dependencies are imported lazily to preserve mock-mode import isolation.

The existing test file `worker/tests/test_arch_zit.py` follows a clear pattern: `test_load_transformer_is_callable` verifies that the function symbol is importable and callable in mock mode without requiring torch/diffusers. Tests that exercise real-mode paths use the `ANVILML_WORKER_MOCK=0` override-and-restore pattern with `try/finally`.

The `AutoencoderKL` class from `diffusers` is the target model class. The `VAE_SCALE_FACTOR` constant (value 8) is already documented in the module's comment block explaining the `block_out_channels=[128, 256, 512, 512]` config. The `convert_ldm_vae_checkpoint` function from `diffusers.loaders.single_file_utils` is the key-remapping utility — it takes a raw checkpoint dict and a plain config dict with `down_block_types`/`up_block_types`, and returns a remapped dict suitable for `load_state_dict()`.

No gap between design doc and current source affects this task's approach: `load_transformer()` already exists and `load_vae()` is the natural sibling. The design doc (§10.5) documents both functions as the same pattern.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0        | pypi-query MCP | n/a                    |
| python | safetensors | ≥0.8        | pypi-query MCP (declared in worker/requirements/base.txt) | n/a |

Note: `diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint` is a private module path — not part of diffusers' public API. This is the same situation as `convert_z_image_transformer_checkpoint_to_diffusers` used by `load_transformer()`. The function exists in diffusers 0.38.0 (confirmed by the project's own design doc §10.5 which explicitly names it).

## Approach

1. **Add `load_vae()` to `worker/nodes/arch/diffusion/zit.py`**

   Implement `load_vae(model_id: str) -> Any` immediately after `load_transformer()` (after line 292, before `sample()` at line 295). The function follows the identical structure:

   a. Mock-mode check: `_mock = os.environ.get("ANVILML_WORKER_MOCK") == "1"` — return `None` immediately in mock mode.

   b. Lazy imports inside the real-mode branch:
      - `from diffusers import AutoencoderKL`
      - `from diffusers.loaders.single_file_utils import convert_ldm_vae_checkpoint`
      - `from safetensors.torch import load_file as safetensors_load_file`

   c. Model construction: `model = AutoencoderKL(block_out_channels=[128, 256, 512, 512], ...)` — constructed with zero arguments using the class's registered defaults. The `block_out_channels` argument is the only non-default parameter needed; all other config values (such as `latent_channels`, `down_block_types`, `up_block_types`) use their published defaults which match the Z-Image Turbo architecture. This mirrors `load_transformer()`'s `ZImageTransformer2DModel()` with zero arguments.

   d. Raw checkpoint load: `checkpoint = safetensors_load_file(model_id)`

   e. Key remapping: `remapped = convert_ldm_vae_checkpoint(checkpoint, config)` where `config` is a plain dict with `down_block_types=["DownEncoderBlock2D"] * 4` and `up_block_types=["UpDecoderBlock2D"] * 4` — these are the standard SD-style VAE block type strings matching the 4-entry `block_out_channels` length. The function `convert_ldm_vae_checkpoint` only reads the *length* of these lists from config, not their exact content, so the standard block type strings are sufficient.

   f. Apply weights: `model.load_state_dict(remapped)`

   g. Return `model`

   The function docstring follows the same structure as `load_transformer()`'s: describes what it does, the zero-network-calls guarantee, mock-mode behavior, Args/Returns/Raises sections.

2. **Add `"load_vae"` to `__all__`**

   Append `"load_vae"` to the `__all__` list at the top of `zit.py` (after `"load_transformer"`), so the function is part of the module's public API.

3. **Add test to `worker/tests/test_arch_zit.py`**

   Add `test_load_vae_is_callable()` immediately after `test_load_transformer_is_callable()` (after line 157). The test:
   - Imports `load_vae` from the module under test
   - Asserts `callable(load_vae)` is `True`
   - Confirms the function symbol exists and is importable without torch/diffusers in mock mode

   Also add `"load_vae"` to the import block at the top of the test file (after `"load_transformer"`).

## Public API Surface

New public item added to `worker.nodes.arch.diffusion.zit`:

```python
def load_vae(model_id: str) -> Any:
    """Load a VAE from a raw ``.safetensors`` file.

    Constructs an ``AutoencoderKL`` with the published Z-Image Turbo
    VAE config (``block_out_channels=[128, 256, 512, 512]``) using
    zero-argument defaults. Weights are loaded from the provided
    ``.safetensors`` file via ``safetensors.torch.load_file``, and
    keys are remapped to the diffusers convention by reusing
    ``diffusers.loaders.single_file_utils``'s internal conversion
    function.

    This function performs **zero network calls**.

    In mock mode (``ANVILML_WORKER_MOCK=1``), returns ``None``.

    Args:
        model_id: Path to a ``.safetensors`` file containing raw VAE
            weights (the format produced by ``AutoencoderKL.state_dict()``).

    Returns:
        An ``AutoencoderKL`` instance with weights loaded and remapped,
        or ``None`` in mock mode.

    Raises:
        OSError: If the file at ``model_id`` does not exist or is
            inaccessible.
        ValueError: If the checkpoint is malformed and cannot be remapped
            or loaded into the model's state dict.
    """
```

Also added to `__all__`: `"load_vae"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `load_vae()` function; add `"load_vae"` to `__all__` |
| MODIFY | `worker/tests/test_arch_zit.py` | Add `load_vae` import; add `test_load_vae_is_callable()` test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_load_vae_is_callable` | `load_vae` is a callable function symbol importable in mock mode without torch/diffusers/safetensors | `ANVILML_WORKER_MOCK=1` set by conftest.py autouse fixture | None | `callable(load_vae) == True` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_vae_is_callable -v` exits 0 |

## CI Impact

No CI changes required. The new test follows the existing mock-mode test pattern already collected by the default `pytest worker/tests/` invocation. No new file type, gate, or test module is introduced.

## Platform Considerations

None identified. The `load_vae()` function operates entirely on local filesystem paths and uses only Python/PyTorch APIs that are platform-neutral. The `safetensors.torch.load_file()` call works identically on Linux, Windows, and macOS. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint` API shape (function name, parameter order, config dict keys) differs from what the task context describes or what exists in the actual installed diffusers version | Medium | High | The ACT agent must verify the function signature at session start using the installed diffusers version (e.g. `python -c "from diffusers.loaders.single_file_utils import convert_ldm_vae_checkpoint; import inspect; print(inspect.signature(convert_ldm_vae_checkpoint))"`). If the signature differs, adapt the config dict and call accordingly. The design doc §10.5 explicitly names this function, providing strong confidence it exists in 0.38.0. |
| `AutoencoderKL(block_out_channels=[128,256,512,512])` zero-arg construction may not produce the correct VAE architecture config — the published Z-Image Turbo VAE may have additional non-default parameters beyond `block_out_channels` | Low | Medium | If `load_state_dict()` fails due to missing/extra keys, inspect the real Z-Image Turbo VAE's config to identify all required non-default parameters and add them to the constructor. The `VAE_SCALE_FACTOR` comment in the file already documents the key config value, and `convert_ldm_vae_checkpoint` handles the key remapping regardless of which parameters the model was constructed with. |
| `convert_ldm_vae_checkpoint` expects specific `down_block_types`/`up_block_types` strings that may differ from standard SD-style names | Low | Medium | Read the actual source of `convert_ldm_vae_checkpoint` at ACT time to confirm the expected config keys and values. The function only reads list length from config per the task context, but the exact key names must match. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.arch.diffusion.zit import load_vae; assert callable(load_vae)"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_load_vae_is_callable -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (all existing tests still pass)
- [ ] `grep -n '"load_vae"' worker/nodes/arch/diffusion/zit.py` — at least one match in `__all__`
- [ ] `grep -n 'def load_vae' worker/nodes/arch/diffusion/zit.py` — at least one match for the function definition
- [ ] `grep -n 'load_vae' worker/tests/test_arch_zit.py` — at least two matches (import + test function name)
