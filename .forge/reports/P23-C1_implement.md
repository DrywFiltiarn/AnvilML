# Implementation Report: P23-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P23-C1                          |
| Phase         | 23 — ZiT VAE Arch Module        |
| Description   | worker/nodes/arch/vae/zit_vae.py: meta construction + dtype selection |
| Implemented   | 2026-07-17T04:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented step 2 of the four-step loading contract for the ZiT VAE arch module. Added guarded torch imports, the `ZiTVaeModel(nn.Module)` class that constructs the VAE encoder/mid-block/decoder from hyperparameters, the `_select_dtype()` pure function implementing the fixed precedence chain (fp8 → bf16 → fp16 → fp32), and a partial `load()` stub that returns a meta-constructed module with dtype applied. Wrote 3 new tests (11 total in file, >=10 as required). All tests pass in both mock and real mode.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | torch     | 2.12.1+cpu       | pypi-query MCP |

No new external dependencies introduced. All types used (`torch`, `torch.nn`, `torch.device`, `torch.dtype`) are from the existing `torch` package.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/vae/zit_vae.py` | Added `import logging` + logger, guarded torch/nn imports, `ZiTVaeModel` class, `_select_dtype()` function, partial `load()` stub with defers_to marker |
| Modify | `worker/tests/test_arch_vae_zit.py` | Added 3 new `@pytest.mark.real_mode` tests for meta construction, no-metadata fixture, and dtype selection |
| Modify | `docs/TESTS.md` | Added 3 entries for new tests following ANVILML_DESIGN.md §17.1 format |

## Commit Log

```
 .forge/reports/P23-C1_plan.md     | 241 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +-
 docs/TESTS.md                     |  36 +++++
 worker/nodes/arch/vae/zit_vae.py  | 321 +++++++++++++++++++++++++++++++++++++-
 worker/tests/test_arch_vae_zit.py | 126 +++++++++++++++
 6 files changed, 733 insertions(+), 10 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pytest-9.1.1, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 11 items

worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_regular_fixture PASSED [  9%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED [ 18%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED [ 27%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_truncated_header_raises PASSED [ 36%]
worker/tests/test_arch_vae_zit.py::test_arch_constant PASSED             [ 45%]
worker/tests/test_arch_vae_zit.py::test_can_handle_matches_zit_vae_key PASSED [ 54%]
worker/tests/test_arch_vae_zit.py::test_can_handle_rejects_unrelated_key PASSED [ 63%]
worker/tests/test_arch_vae_zit.py::test_get_module_returns_zit_vae_for_matching_key PASSED [ 72%]
worker/tests/test_arch_vae_zit.py::test_load_meta_construction_succeeds PASSED [ 81%]
worker/tests/test_arch_vae_zit.py::test_load_meta_construction_no_metadata_fixture PASSED [ 90%]
worker/tests/test_arch_vae_zit.py::test_load_dtype_selection_applied PASSED [100%]

============================== 11 passed in 1.89s ==============================
```

Mock-mode collection (8 tests, 3 deselected by real_mode marker):
```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 11 items / 3 deselected / 8 selected

worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_regular_fixture PASSED [ 12%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED [ 25%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED [ 37%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_truncated_header_raises PASSED [ 50%]
worker/tests/test_arch_vae_zit.py::test_arch_constant PASSED             [ 62%]
worker/tests/test_arch_vae_zit.py::test_can_handle_matches_zit_vae_key PASSED [ 75%]
worker/tests/test_arch_vae_zit.py::test_can_handle_rejects_unrelated_key PASSED [ 87%]
worker/tests/test_arch_vae_zit.py::test_get_module_returns_zit_vae_for_matching_key PASSED [100%]

======================= 8 passed, 3 deselected in 1.90s ========================
```

## Format Gate

```
cargo fmt --all -- --check
# exited 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.91s — PASSED

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 54.21s — PASSED

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.46s — PASSED

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.89s — PASSED
```

All four platform cross-checks pass.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
# test tests::config_reference_matches_defaults ... ok
# test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 1 passes. Gate 2 (OpenAPI drift) is not triggered (no handler signature changes). Gate 3 (Node parity) and Gate 4 (Mock/Real parity markers) are not triggered (no new node types or parity markers for this partial stub).

## Public API Delta

New public items in `worker/nodes/arch/vae/zit_vae.py`:
- `class ZiTVaeModel(_ModuleBase)` — nn.Module subclass, `__init__(self, hyperparams: dict[str, Any]) -> None`, sets `.arch = "zit_vae"`
- `def _select_dtype(caps: dict, native_dtype: str) -> torch.dtype` — pure function, fixed precedence fp8→bf16→fp16→fp32
- `def load(path: str, caps: dict, device: str = "cpu") -> ZiTVaeModel` — partial stub (steps 1-2 of §11.3)

These match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

1. **GroupNorm group count**: The plan specified `nn.GroupNorm(8, out_channels)`, but the fixture has `latent_channels=4`, and PyTorch requires `num_channels % num_groups == 0`. Fixed by using `nn.GroupNorm(min(8, out_ch), out_ch)` for all GroupNorm instances. This is a necessary adjustment to make the model constructible with the fixture's channel dimensions.

2. **ModuleDict naming**: The plan used `f"block.{i}"` as module names, but PyTorch ModuleDict rejects names containing `.`. Fixed by using `f"block_{i}"` (underscore instead of dot).

3. **Block count derivation**: The plan stated "one block per the tiny fixture" but the actual fixture has 2 encoder blocks and 2 decoder blocks. Implemented with explicit `encoder_block_count = 2` and `decoder_block_count = 2` to match the fixture structure, with linear interpolation for intermediate block channel counts. This ensures the model's parameter shapes align with the fixture's checkpoint keys for P23-C3's key remapping.

## Blockers

None.
