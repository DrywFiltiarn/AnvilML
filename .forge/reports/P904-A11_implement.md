# Implementation Report: P904-A11

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P904-A11                                                    |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)       |
| Description | worker/nodes/arch/diffusion/zit.py: add load_vae() — offline VAE loading, no HF network access |
| Implemented | 2026-06-24T11:45:00Z                                        |
| Status      | COMPLETE                                                    |

## Summary

Implemented `load_vae(model_id: str) -> Any` in `worker/nodes/arch/diffusion/zit.py`, mirroring the established `load_transformer()` pattern. The function loads an `AutoencoderKL` VAE from a raw `.safetensors` checkpoint file using zero-argument model construction, `safetensors.torch.load_file()`, and `diffusers.loaders.single_file_utils.convert_ldm_vae_checkpoint()` for key remapping — all local operations with no HuggingFace network access. Added `"load_vae"` to the module's `__all__` public API, and added `test_load_vae_is_callable()` to the existing test file. All 95 Python tests and all Rust tests pass.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| python | diffusers | 0.38.0        | pypi-query MCP |
| python | safetensors | ≥0.8        | project lockfile (declared in worker/requirements/base.txt) |

Verified at session start:
- `convert_ldm_vae_checkpoint(checkpoint, config)` signature confirmed: takes `(checkpoint, config)` where config is a dict with `down_block_types` and `up_block_types` keys (only their lengths are used).
- `AutoencoderKL.__init__` accepts `block_out_channels` as keyword argument; defaults are compatible with Z-Image Turbo VAE checkpoint format.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Added `load_vae()` function (90 lines); added `"load_vae"` to `__all__` |
| MODIFY | `worker/tests/test_arch_zit.py` | Added `load_vae` import; added `test_load_vae_is_callable()` (17 lines) |
| MODIFY | `docs/TESTS.md` | Added `test_load_vae_is_callable` catalogue entry |

## Commit Log

```
 .forge/reports/P904-A11_plan.md    | 159 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +--
 docs/TESTS.md                      |   9 +++
 worker/nodes/arch/diffusion/zit.py |  89 +++++++++++++++++++++
 worker/tests/test_arch_zit.py      |  22 +++++
 6 files changed, 289 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 95 items

worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [  1%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [  2%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [  3%]
worker/tests/test_arch_zit.py::test_load_transformer_is_callable PASSED  [  4%]
worker/tests/test_arch_zit.py::test_load_vae_is_callable PASSED          [  5%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [  6%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [  7%]
worker/tests/test_arch_zit.py::test_sample_real_assembles_pipeline_via_cache PASSED [  8%]
worker/tests/test_arch_zit.py::test_sample_real_invokes_pipeline_with_correct_args PASSED [  9%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [ 10%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [ 11%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [ 12%]
worker/tests/test_arch_zit.py::test_make_callback_emits_progress PASSED  [ 13%]
worker/tests/test_arch_zit.py::test_make_callback_raises_on_cancellation PASSED [ 14%]
... (all 95 tests passed) ...
============================= 95 passed in 16.85s ==============================
```

Rust tests: `cargo test --workspace --features mock-hardware` — 237 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0, no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.58s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

All four checks exited 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
  running 1 test
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:** Not triggered — no handler signatures, `#[utoipa::path]` annotations, or `ToSchema` derives modified.

**Gate 3 — Node Parity:** `worker/tests/test_parity.py` does not exist in this repository — gate N/A.

## Public API Delta

```
+    "load_vae",
+def load_vae(model_id: str) -> Any:
```

New public items:
- `fn load_vae(model_id: str) -> Any` in module `worker.nodes.arch.diffusion.zit`
- `"load_vae"` appended to `__all__` list

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- `load_vae()` placed after `load_transformer()` (line 292) and before `sample()` (line 295)
- Mock-mode check via `os.environ.get("ANVILML_WORKER_MOCK") == "1"` returns `None`
- Lazy imports inside real-mode branch: `AutoencoderKL`, `convert_ldm_vae_checkpoint`, `safetensors_load_file`
- Model construction: `AutoencoderKL(block_out_channels=[128, 256, 512, 512])` with zero other arguments
- Config dict: `{"down_block_types": ["DownEncoderBlock2D"] * 4, "up_block_types": ["UpDecoderBlock2D"] * 4}`
- Key remapping via `convert_ldm_vae_checkpoint(checkpoint, config)`
- Weights applied via `model.load_state_dict(remapped)`
- `"load_vae"` added to `__all__` after `"load_transformer"`
- Test `test_load_vae_is_callable()` added after `test_load_transformer_is_callable()`
- `load_vae` added to test file import block

## Blockers

None.
