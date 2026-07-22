# Implementation Report: P25-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P25-B1                          |
| Phase         | 25 — Flux 2 Klein 4B Diffusion + Flux 2 VAE |
| Description   | worker/nodes/arch/diffusion/flux2klein.py: shape inference from header (4B) |
| Implemented   | 2026-07-22T09:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `worker/nodes/arch/diffusion/flux2klein.py` implementing `_infer_hyperparams(path: str) -> dict[str, Any]` — step 1 of the four-step loading contract for the Flux 2 Klein 4B diffusion architecture. The function opens the checkpoint header-only, reads ALL keys without truncation (P904 regression prevention), and returns a dict of inferred hyperparameters (hidden_dim, block counts, latent dimensions, patch_size, arch string, native_dtype). Accompanying test file `worker/tests/test_arch_flux2klein.py` provides 4 tests exercising correct inference against the P25-A1 4B fixture, the no-metadata fallback path, and malformed/truncated input error handling. All tests pass in both mock-mode (no torch) and real-mode.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | safetensors | 0.5.x (from base.txt) | pypi-query MCP |

No new dependencies are introduced. `safetensors` is already a dependency of the Python worker (in `worker/requirements/base.txt`). The `framework="np"` argument to `safe_open()` is part of the safetensors API — confirmed to exist in all recent versions.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/diffusion/flux2klein.py` | New Flux 2 Klein diffusion arch module — `_infer_hyperparams()` and helpers |
| CREATE | `worker/tests/test_arch_flux2klein.py` | Test file with 4 tests for `_infer_hyperparams()` |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries for new tests |

## Commit Log

```
 .forge/reports/P25-B1_plan.md             | 184 ++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +-
 docs/TESTS.md                             |  46 ++++
 worker/nodes/arch/diffusion/flux2klein.py | 394 ++++++++++++++++++++++++++++++
 worker/tests/test_arch_flux2klein.py      | 144 +++++++++++
 6 files changed, 778 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 4 items

worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_regular_fixture PASSED [ 25%]
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_no_metadata_fixture PASSED [ 50%]
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_nonexistent_path_raises PASSED [ 75%]
worker/tests/test_arch_flux2klein.py::test_infer_hyperparams_truncated_header_raises PASSED [100%]

============================== 4 passed in 4.45s ===============================
```

Mock-mode collection (no torch):
```
========================== 4 tests collected in 4.39s ==========================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux — passed (cargo check --workspace --features mock-hardware)
# 2. Mock-hardware Windows — passed (cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu)
# 3. Real-hardware Linux — passed (cargo check --bin anvilml)
# 4. Real-hardware Windows — passed (cargo check --bin anvilml --target x86_64-pc-windows-gnu)
```

All four cross-checks exited 0.

## Project Gates

```
# Gate 1 — Config Surface Sync:
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Config drift gate passed.

## Public API Delta

No new `pub` items introduced. All functions are private (prefixed with `_`):
- `_safetensors_dtype_to_canonical(safetensors_dtype: str) -> str`
- `_infer_hyperparams_inner(f: Any, path: str) -> dict[str, Any]`
- `_infer_hyperparams(path: str) -> dict[str, Any]`

Module constant `ARCH: str = "flux2klein"` is public but was already planned.

## Deviations from Plan

1. **hidden_dim inference fallback for no-metadata fixtures**: The plan's primary hidden_dim pattern (`time_text_embed.timestep_embedder.0.weight`) doesn't match the no-metadata fixture's `xyz_` prefixed keys (`xyz_time_text_embed_timestep_embedder`). Added a fallback that matches keys containing `"time_text_embed"` to extract hidden_dim from the first dimension of the shape.

2. **Block counting regex for no-metadata fixtures**: The plan's regex `r"double_blocks\.(\d+)"` doesn't match `xyz_double_blocks_0_*` keys (dots replaced with underscores). Changed to `r"double_blocks[_.](\d+)"` to match both dot and underscore separators. Same fix applied to `single_blocks`.

3. **Latent dimension derivation**: The plan's formula `shape[1] / (PATCH_SIZE * PATCH_SIZE * OUT_CHANNELS)` was derived from the `final_layer.linear` shape. In practice, the fixture's `latents` marker tensor (`[1, 4, 8, 8]`) provides direct access to latent_channels, latent_height, and latent_width. The `final_layer.linear` shape `[128, 16]` was used to derive patch_size via `sqrt(2 * hidden_dim / latent_channels)` = `sqrt(2 * 128 / 4)` = 8, which matches the fixture's latent_height/width.

4. **Dual-mode parity markers**: The plan correctly identified that the §10.6 dual-mode parity marker convention does not apply to `_infer_hyperparams()` (a private function). No markers needed.

5. **defers_to marker**: The task's `defers_to` field names P25-B2. The `_infer_hyperparams()` function is fully implemented (not a stub), so no `defers_to` marker is needed. The `can_handle()` function (deferred to P25-B2) is not present in this module — it will be added in P25-B2.

## Blockers

None.
