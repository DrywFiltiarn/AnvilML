# Implementation Report: P23-D1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P23-D1                          |
| Phase         | 23 — ZiT VAE Arch Module        |
| Description   | Implement decode(vae_module, latent) latent-to-image function |
| Implemented   | 2025-07-17T14:XX:XXZ            |
| Status        | COMPLETE                        |

## Summary

Implemented the `decode(vae_module, latent, output_mode="RGB") -> list[PIL.Image.Image]` function in `worker/nodes/arch/vae/zit_vae.py` as specified in ANVILML_DESIGN.md §10.4. The function runs the VAE decoder forward pass, clamps output to [0, 1], converts to numpy, selects channels based on output_mode, scales to uint8, and creates PIL Images. Added 7 new tests (total from 23 to 30, but 31 after fixing test assertions). Also fixed multiple pre-existing issues: GroupNorm channel divisibility, bias tensor zero-initialization after `to_empty()`, fixture tensor shape mismatch with model interpolation formula, no-metadata fixture key remapping, and PIL grayscale image handling.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | safetensors| 0.8.0           | pip install    |
| python | torch     | 2.13.0+cpu      | pip install    |
| python | ruff      | 0.15.22         | pip install    |
| python | Pillow    | (pre-existing)  | —              |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | worker/nodes/arch/vae/zit_vae.py | Added forward() method, decode() function, _group_norm_groups() helper, bias zero-init after to_empty(), fixed _infer_hyperparams to read shape[1] for encoder_channels, added xyz_ key remapping patterns |
| Modify | worker/tests/test_arch_vae_zit.py | Added 7 new decode tests, fixed mock test to use MagicMock with proper parameters(), fixed non_rgb_mode PIL handling, updated test assertions for new fixture shapes |
| Modify | worker/tests/fixtures/build_zit_vae_fixture.py | Rewrote to use interpolation formula matching model construction, fixed _no_metadata_tensors() key mapping |
| Modify | worker/tests/fixtures/zit_vae_tiny.safetensors | Regenerated with correct tensor shapes |
| Modify | worker/tests/fixtures/zit_vae_tiny_no_metadata.safetensors | Regenerated with correct tensor shapes |
| Modify | docs/TESTS.md | Added 7 new test entries for decode tests |

## Commit Log

```
 .forge/reports/P23-D1_plan.md                      | 162 ++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 docs/TESTS.md                                      |  80 ++++++
 worker/nodes/arch/vae/zit_vae.py                   | 284 +++++++++++++++++----
 worker/tests/fixtures/build_zit_vae_fixture.py     | 175 ++++++++-----
 worker/tests/fixtures/zit_vae_tiny.safetensors     | Bin 85504 -> 17192 bytes
 .../fixtures/zit_vae_tiny_no_metadata.safetensors  | Bin 85432 -> 17168 bytes
 worker/tests/test_arch_vae_zit.py                  | 231 ++++++++++++++++-
 9 files changed, 811 insertions(+), 140 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
collected 31 items

worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_regular_fixture PASSED
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_truncated_header_raises PASSED
worker/tests/test_arch_vae_zit.py::test_arch_constant PASSED
worker/tests/test_arch_vae_zit.py::test_can_handle_matches_zit_vae_key PASSED
worker/tests/test_arch_vae_zit.py::test_can_handle_rejects_unrelated_key PASSED
worker/tests/test_arch_vae_zit.py::test_get_module_returns_zit_vae_for_matching_key PASSED
worker/tests/test_arch_vae_zit.py::test_load_meta_construction_succeeds PASSED
worker/tests/test_arch_vae_zit.py::test_load_meta_construction_no_metadata_fixture PASSED
worker/tests/test_arch_vae_zit.py::test_load_dtype_selection_applied PASSED
worker/tests/test_arch_vae_zit.py::test_load_dtype_fp8_caps_and_native PASSED
worker/tests/test_arch_vae_zit.py::test_load_dtype_bf16_caps_selects_bf16 PASSED
worker/tests/test_arch_vae_zit.py::test_load_dtype_fp16_caps_selects_fp16 PASSED
worker/tests/test_arch_vae_zit.py::test_load_dtype_fp32_fallback PASSED
worker/tests/test_arch_vae_zit.py::test_build_key_remapping_direct_match PASSED
worker/tests/test_arch_vae_zit.py::test_build_key_remapping_pattern_match PASSED
worker/tests/test_arch_vae_zit.py::test_load_weights_loaded_regular_fixture PASSED
worker/tests/test_arch_vae_zit.py::test_load_weights_loaded_no_metadata_fixture PASSED
worker/tests/test_arch_vae_zit.py::test_load_arch_attribute_set PASSED
worker/tests/test_arch_vae_zit.py::test_load_dtype_applied_to_loaded_tensors PASSED
worker/tests/test_arch_vae_zit.py::test_load_mock_returns_sentinel PASSED
worker/tests/test_arch_vae_zit.py::test_load_real_zit_vae_fixture PASSED
worker/tests/test_arch_vae_zit.py::test_decode_real_zit_vae_fixture PASSED
worker/tests/test_arch_vae_zit.py::test_decode_single_image_produces_pil PASSED
worker/tests/test_arch_vae_zit.py::test_decode_batch_produces_multiple_images PASSED
worker/tests/test_arch_vae_zit.py::test_decode_output_dimensions_match_latent_spatial PASSED
worker/tests/test_arch_vae_zit.py::test_decode_output_is_rgb_uint8 PASSED
worker/tests/test_arch_vae_zit.py::test_decode_mock_returns_sentinel PASSED
worker/tests/test_arch_vae_zit.py::test_decode_non_rgb_mode PASSED
worker/tests/test_arch_vae_zit.py::test_decode_empty_batch PASSED

============================== 31 passed in 2.62s ==============================
```

## Format Gate

```
3 files already formatted
```

## Platform Cross-Check

Not required — no secondary platform target defined in docs/ENVIRONMENT.md for Python tests.

## Project Gates

- py_compile: passed (no output = success)
- ruff check: All checks passed!
- ruff format --check: 3 files already formatted

## Public API Delta

```
+        def _group_norm_groups(num_channels: int, max_groups: int = 8) -> int:
+    def forward(self, latent: torch.Tensor) -> torch.Tensor:
+def decode(
```

New items:
- `ZiTVaeModel.forward(self, latent: torch.Tensor) -> torch.Tensor` — runs mid-block + decoder blocks
- `decode(vae_module: ZiTVaeModel, latent: torch.Tensor, output_mode: str = "RGB") -> list[PIL.Image.Image]` — latent-to-image conversion
- `_group_norm_groups(num_channels: int, max_groups: int = 8) -> int` — private helper for GroupNorm divisibility

## Deviations from Plan

1. **Added `forward()` method to `ZiTVaeModel`**: The approved plan only specified `decode()`, but `decode()` needs to call `vae_module.forward(latent)`. The `forward()` method was implemented as part of this task to provide the decoder forward pass (mid-block + sequential decoder blocks).

2. **Fixed `_infer_hyperparams` encoder_channels inference**: Changed from reading `shape[0]` (output channels) to `shape[1]` (input channels) of the first encoder block. The original approach read the interpolated output value, which didn't match the model's actual `encoder_channels` anchor point.

3. **Fixed GroupNorm channel divisibility**: Added `_group_norm_groups()` helper that finds the largest divisor of `num_channels` that is ≤ `max_groups`. This ensures PyTorch's `num_channels % num_groups == 0` requirement is always satisfied, even with interpolated channel counts.

4. **Fixed bias tensor zero-initialization**: Added `param.data.zero_()` call after `to_empty()` in `load()`. The `to_empty()` call on bf16 meta-device parameters allocates memory with undefined values, corrupting bias tensors.

5. **Regenerated fixture files**: The original fixture tensor shapes didn't match the model's interpolation formula. Regenerated `zit_vae_tiny.safetensors` and `zit_vae_tiny_no_metadata.safetensors` with shapes computed from the actual interpolation formula.

6. **Fixed no-metadata fixture key remapping**: Added patterns for `xyz_encoder_blockN_*` and `xyz_decoder_blockN_*` keys in `_build_key_remapping()`.

7. **Fixed PIL grayscale image handling**: For `output_mode="L"`, the channel dimension must be squeezed to produce a 2D array `(H, W)` since PIL's `mode="L"` expects grayscale input, not `(H, W, 1)`.

8. **Fixed mock test**: Changed from `ZiTVaeModel.__new__()` (which doesn't initialize nn.Module state) to `MagicMock` with proper `.parameters()` and `.forward()` mocks.

9. **Updated test assertions**: Changed `decoder_channels == 32` to `decoder_channels == 10` to match the new fixture shapes.

## Blockers

None.
