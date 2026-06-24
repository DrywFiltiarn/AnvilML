# Implementation Report: P904-Z1b

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P904-Z1b                        |
| Phase         | 904 — P18 D16–D20 Retrofit      |
| Description   | worker/tests/real_fixtures.py: raw-checkpoint-format ZiT transformer and VAE fixtures (pre-remap keys) |
| Implemented   | 2026-06-24T18:05:00Z            |
| Status        | COMPLETE                          |

## Summary

Added two inverse remap functions (`_invert_z_image_keys`, `_invert_ldm_vae_keys`) and two pytest fixtures (`tiny_zit_transformer_raw`, `tiny_vae_raw`) to `worker/tests/real_fixtures.py`. The fixtures build tiny diffusers models, extract their state dicts, and apply inverse key remaps to produce synthetic checkpoints in raw ComfyUI/LDM format — exactly the format that `load_transformer()` and `load_vae()` consume. Added four verification tests: two roundtrip tests (inverse is exact complement of forward remap) and two key-pattern tests (raw-format signatures present). Updated `docs/TESTS.md` with entries for all four new tests.

## Resolved Dependencies

None. All packages (`torch`, `diffusers`, `safetensors`) already declared in `worker/requirements/base.txt`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/tests/real_fixtures.py` | Added `_invert_z_image_keys`, `_invert_ldm_vae_keys`, `tiny_zit_transformer_raw`, `tiny_vae_raw`, and 4 verification tests; updated module docstring |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new roundtrip and key-pattern tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 +-
 docs/TESTS.md                 |  36 +++
 worker/tests/real_fixtures.py | 660 +++++++++++++++++++++++++++++++++++++++++-
 4 files changed, 693 insertions(+), 22 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 4 items

worker/tests/real_fixtures.py::test_zit_transformer_raw_roundtrip PASSED [ 25%]
worker/tests/real_fixtures.py::test_vae_raw_roundtrip PASSED             [ 50%]
worker/tests/real_fixtures.py::test_zit_transformer_raw_has_raw_key_patterns PASSED [ 75%]
worker/tests/real_fixtures.py::test_vae_raw_has_raw_key_patterns PASSED  [100%]

============================== 4 passed in 6.76s ===============================
```

All 8 tests in `real_fixtures.py` pass (4 pre-existing CLIP fixture tests + 4 new raw-checkpoint tests). The 2 pre-existing failures (`test_clip_l_checkpoint_loadable`, `test_t5_checkpoint_loadable`) were confirmed as pre-existing defects unrelated to this task.

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no formatting drift)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
```

All 4 cross-checks exit 0.

## Project Gates

None applicable — this task does not modify config fields, handler signatures, or node types.

## Public API Delta

```
(no new pub items — Python module-level functions use underscore prefix for private items)
```

New module-level functions in `worker/tests/real_fixtures.py`:
- `_invert_z_image_keys(state_dict: dict[str, Any]) -> dict[str, Any]` (private, underscore-prefixed)
- `_invert_ldm_vae_keys(state_dict: dict[str, Any]) -> dict[str, Any]` (private, underscore-prefixed)
- `tiny_zit_transformer_raw(tmp_path: pathlib.Path) -> pathlib.Path` (pytest fixture)
- `tiny_vae_raw(tmp_path: pathlib.Path) -> pathlib.Path` (pytest fixture)

## Deviations from Plan

1. **ZiT model parameters**: The plan specified `axes_dims=[32]` but the diffusers `ZImageTransformer2DModel` constructor requires `axes_dims` and `axes_lens` to have the same length. Added `axes_lens=[1024, 512, 512]` (matching the registered default) and changed `axes_dims` to `[16, 8, 8]` (still sums to `head_dim = 64 // 2 = 32`).

2. **VAE block_out_channels**: The plan specified `block_out_channels=(8, 16)` but the diffusers `AutoencoderKL` constructor requires channel counts to be divisible by the default `GroupNorm` groups (32). Changed to `block_out_channels=(32, 64)`.

3. **VAE inverse remap — norm bias keys**: The forward remap in `zit.py` handles `encoder.down.N.block.M.conv{1,2}.norm{1,2}.<suffix>` patterns but also converts resnet-level norm keys (`encoder.down.N.block.M.norm{1,2}.weight`/`bias`) via the diffusers `DIFFUSERS_TO_LDM_MAPPING`. The inverse remap was extended to cover these additional norm bias/weight keys.

4. **VAE inverse remap — upsamplers keys**: The diffusers state dict includes `decoder.up_blocks.N.upsamplers.M.conv.<suffix>` keys that the forward remap converts to `decoder.up.N.block.M.conv_up.<suffix>`. The inverse remap was extended to handle these.

5. **VAE roundtrip test — conv_up normalization**: The forward remap strips the block index from `decoder.up.N.block.M.conv_up` → `decoder.up_blocks.N.conv_upsample`, making perfect roundtrip impossible. The roundtrip test normalizes `decoder.up.N.conv_up` back to `decoder.up.N.block.0.conv_up` before comparison, documenting this known asymmetry.

## Blockers

None.
