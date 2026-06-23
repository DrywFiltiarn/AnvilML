# Plan Report: P18-D20

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D20                                     |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/decode.py: VaeDecode real decoding path |
| Depends on  | P18-D19                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-23T14:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace `VaeDecode.execute()`'s `NotImplementedError` stub with a real VAE decode path that takes the raw denoised latent tensor from `Sampler` (output_type="latent"), applies the inverse-of-encode scaling using the VAE's `scaling_factor` and `shift_factor`, calls `vae.decode()`, and postprocesses the result via `diffusers.VaeImageProcessor` into a real `PIL.Image.Image`. This completes the decode step of the ZiT FP8 generation pipeline so that `SaveImage` can receive a real image and encode it to PNG.

## Scope

### In Scope
- Modify `worker/nodes/decode.py`: replace the `NotImplementedError` real-path with working VAE decode logic
- Import `diffusers.VaeImageProcessor` lazily inside the real-mode code path (never at module top level)
- Apply inverse-of-encode scaling: `latents = (latents / vae.config.scaling_factor) + vae.config.shift_factor`
- Call `vae.decode(latents, return_dict=False)[0]` to get the raw decoded tensor
- Postprocess via `VaeImageProcessor(vae_scale_factor=16).postprocess(tensor, output_type="pil")` and return the first PIL Image
- Add inline comment explaining the inverse-of-encode scaling math
- Ensure existing mock-mode tests in `worker/tests/test_nodes_decode.py` continue to pass unchanged
- Add a real-path test that verifies the decode logic when `ANVILML_WORKER_MOCK` is not "1"

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No deferrals.

## Existing Codebase Assessment

The `worker/nodes/decode.py` module defines `VaeDecode` with a complete mock-mode path (returns `MockImage()` when `ANVILML_WORKER_MOCK=1`) and a stub real-mode path that raises `NotImplementedError`. The `MockImage` sentinel class is lightweight and serves as a placeholder for testing. The module follows the established pattern: lazy imports guarded by `os.environ.get("ANVILML_WORKER_MOCK")`, Google-style docstrings, and `@register` decorator registration.

The test file `worker/tests/test_nodes_decode.py` has four tests: registry registration, mock-mode execution, metadata attribute verification, and missing-input handling. All run under mock mode and use `importlib.reload()` to re-register the node against a cleared `NODE_REGISTRY`.

The `SaveImage` node in `image.py` already accepts an image input (though in mock mode it ignores it and generates a black PNG). In real mode, it will receive a `PIL.Image.Image` from `VaeDecode` and encode it to PNG via the existing `_generate_black_png`-style path — but since `SaveImage` currently always generates its own PNG regardless of input, the transition from `MockImage` to `PIL.Image.Image` will be transparent.

The project's Python worker uses `diffusers>=0.38.0` (confirmed via `worker/requirements/base.txt`), which includes `VaeImageProcessor` with a `postprocess()` method that accepts a `torch.Tensor` and returns `list[PIL.Image.Image]` when `output_type="pil"`.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source        | Feature flags confirmed |
|--------|-------------------|-----------------|-------------------|------------------------|
| python | diffusers         | 0.38.0          | pypi-query MCP    | n/a                    |
| python | diffusers.VaeImageProcessor | 0.38.0 (same package) | pypi-query MCP + GitHub source | n/a |

The `VaeImageProcessor` API was confirmed against the diffusers 0.38.0 source on GitHub:
- Constructor: `VaeImageProcessor(vae_scale_factor=8, ...)` — `vae_scale_factor` defaults to 8, but ZiT uses 16 (see `ZImagePipeline.__init__`: `self.image_processor = VaeImageProcessor(vae_scale_factor=self.vae_scale_factor * 2)` where `self.vae_scale_factor = 8`)
- `postprocess(image: torch.Tensor, output_type: str = "pil", do_denormalize: list[bool] | None = None) -> PIL.Image.Image | np.ndarray | torch.Tensor` — returns `list[PIL.Image.Image]` when `output_type="pil"`
- Static methods: `pt_to_numpy()`, `numpy_to_pil()`, `denormalize()`, `_denormalize_conditionally()`

## Approach

1. **Read the current `decode.py` source** to confirm the exact location of the `NotImplementedError` stub and the mock-mode guard.

2. **Implement the real decode path** inside `VaeDecode.execute()`, replacing the `raise NotImplementedError(...)` block. The real-mode code must be placed after the mock-mode `if` guard and must lazily import all heavy dependencies:

   ```python
   # Real mode: decode latent tensor using the loaded VAE.
   # Inverse of the encode-time scaling: during encoding, latents were
   # scaled as z = z * scaling_factor + shift_factor (conceptually);
   # the decoder expects the original scale, so we undo it here:
   #   latents = (latents / vae.config.scaling_factor) + vae.config.shift_factor
   # This reverses the normalization that compresses the latent space
   # to unit variance during VAE training (see Kingma & Welling 2013,
   # and the diffusers AutoencoderKL config default scaling_factor=0.18215).
   latents = inputs.get("latent")
   vae = inputs.get("vae")

   # Lazy imports — torch/diffusers must never be imported at module
   # top level, or CI tests without GPU hardware will fail on import.
   import torch
   from diffusers.image_processor import VaeImageProcessor

   # Apply the inverse-of-encode scaling.
   # The VAE was trained with latents normalised to unit variance using
   # scaling_factor (default 0.18215 for SD-style VAEs). To decode, we
   # undo this normalisation before passing to the decoder.
   latents = (latents / vae.config.scaling_factor) + vae.config.shift_factor

   # Decode the latent to a raw image tensor.
   # return_dict=False returns a plain tuple; [0] extracts the tensor.
   # The tensor is in the VAE's output space (typically [-1, 1] range).
   decoded = vae.decode(latents, return_dict=False)[0]

   # Postprocess the raw decoded tensor to a PIL Image.
   # VaeImageProcessor handles denormalization ([-1,1] -> [0,1]),
   # conversion to numpy, and conversion to PIL. The vae_scale_factor
   # of 16 matches ZImagePipeline's own image_processor construction
   # (self.vae_scale_factor=8 * 2 = 16 for ZiT's 4-block VAE).
   processor = VaeImageProcessor(vae_scale_factor=16)
   pil_images = processor.postprocess(decoded, output_type="pil")

   # Return the first (and typically only) PIL Image.
   return {"image": pil_images[0]}
   ```

3. **Update the `execute()` docstring**: Remove the `NotImplementedError` from the `Raises` section since it no longer applies. The docstring should state that real mode now returns a `PIL.Image.Image`.

4. **Verify existing tests still pass**: Run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v`. All four existing tests must pass (they exercise mock mode only).

5. **Add a real-path test** in `worker/tests/test_nodes_decode.py` that verifies the decode logic works when `ANVILML_WORKER_MOCK` is not "1". This test should:
   - Use a mock VAE object (from `worker.nodes.loader.MockVae`) that has `.config.scaling_factor`, `.config.shift_factor`, and a `.decode()` method returning a tensor
   - Use a mock latent tensor
   - Verify the returned image is a `PIL.Image.Image`
   - Mark with `@pytest.mark.skipif` to only run when not in mock mode, OR guard the test with `os.environ.get("ANVILML_WORKER_MOCK") != "1"`

## Public API Surface

No new public items. The task modifies an existing `execute()` method in-place. The public interface (`VaeDecode` class with its `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`) is unchanged.

| Item | Path | Change |
|------|------|--------|
| `VaeDecode.execute()` | `worker.nodes.decode.VaeDecode.execute` | Modified: replaces `NotImplementedError` with real decode; docstring updated |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/decode.py` | Replace `NotImplementedError` with real VAE decode path; update docstring |
| Modify | `worker/tests/test_nodes_decode.py` | Add test for real decode path (non-mock mode) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_registered_in_registry` | VaeDecode is registered in NODE_REGISTRY | NODE_REGISTRY cleared; decode module reloaded | None | `"VaeDecode" in NODE_REGISTRY`; `NODE_TYPE == "VaeDecode"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_execute_returns_mock_image` | execute() returns MockImage in mock mode | `ANVILML_WORKER_MOCK=1` set | `vae=MockVae(), latent=MockLatent()` | `isinstance(result["image"], MockImage)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image -v` exits 0 |
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_metadata_attributes` | All 6 metadata attrs correct | VaeDecode class importable | None | All assertions pass (NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_metadata_attributes -v` exits 0 |
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_execute_missing_inputs_returns_mock` | execute() handles missing inputs in mock mode | `ANVILML_WORKER_MOCK=1` set | No inputs | `isinstance(result["image"], MockImage)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock -v` exits 0 |
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_real_path_returns_pil_image` | execute() returns PIL.Image in real mode (non-mock) | `ANVILML_WORKER_MOCK` not "1" | `vae=MockVaeWithDecode(), latent=mock_tensor` | `isinstance(result["image"], PIL.Image.Image)` | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_real_path_returns_pil_image -v` exits 0 |

## CI Impact

No CI changes required. The modified file (`worker/nodes/decode.py`) is a Python source file already covered by the existing `worker-linux` and `worker-windows` CI jobs. The new test will be picked up by the same pytest invocation (`worker/.venv/bin/python -m pytest worker/tests/ -v`). The real-path test is guarded to only run when `ANVILML_WORKER_MOCK` is not set, which means in CI (where `ANVILML_WORKER_MOCK=1` is set by conftest.py), it will be skipped — the mock tests are the primary CI coverage.

## Platform Considerations

None identified. The VAE decode path uses `torch` and `diffusers`, which are platform-agnostic Python packages. The `VaeImageProcessor.postprocess()` method is pure PyTorch/PIL code with no platform-specific behavior. The `SaveImage` node (consumer of VaeDecode's output) generates PNGs using stdlib on all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `vae.config.shift_factor` may be `None` for some VAE configs (it defaults to `None` in `AutoencoderKL.__init__`). If `None`, the expression `(latents / vae.config.scaling_factor) + vae.config.shift_factor` will raise `TypeError`. | Medium | High | Check `vae.config.shift_factor` at runtime; if `None`, use `0.0` as the shift value (matching the behavior of older diffusers versions where shift was absent). Add inline comment explaining this guard. |
| The VAE's `decode()` returns a tensor in `[-1, 1]` range, and `VaeImageProcessor.postprocess()` with default `do_normalize=True` expects `[0, 1]` range. The denormalization step (`2*images - 1`) would then produce values outside `[0, 1]`, causing invalid PIL images. | Low | Medium | The `ZImagePipeline.__call__` non-latent branch uses the same `VaeImageProcessor` without any special handling — it relies on the VAE's `force_upcast` and the decoder's output range being compatible. If PIL images are corrupted, the test will fail and the ACT agent can set `do_denormalize=[False]` on the processor. |
| `ANVILML_WORKER_MOCK=1` is set by `conftest.py` for all tests. The real-path test must explicitly unset it to exercise the real decode code path. | Low | Low | Use `os.environ.pop("ANVILML_WORKER_MOCK", None)` inside the test, with capture-and-restore for env isolation per FORGE_AGENT_RULES §6. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v` exits 0 (all existing mock tests pass)
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/decode.py` exits 0 (syntax check passes)
- [ ] `worker/.venv/bin/python -c "from worker.nodes.decode import VaeDecode; print(VaeDecode.NODE_TYPE)"` prints `VaeDecode` (module imports without error, node registered)
- [ ] The `NotImplementedError` is absent from `worker/nodes/decode.py` (grep confirms no stub remains)
- [ ] `grep -n "TODO" worker/nodes/decode.py` returns no matches (no TODO comments remain)
