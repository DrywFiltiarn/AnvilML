# Implementation Report: P20-B1

| Field         | Value                                                       |
|---------------|-------------------------------------------------------------|
| Task ID       | P20-B1                                                      |
| Phase         | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description   | worker/nodes/arch/diffusion/zit.py: shape inference from safetensors header |
| Implemented   | 2026-07-13T15:30:00Z                                        |
| Status        | COMPLETE                                                      |

## Summary

Created `worker/nodes/arch/diffusion/zit.py` implementing `_infer_hyperparams(path: str) -> dict`, the first step of the four-step loading contract (ANVILML_DESIGN.md §11.3). The function opens a safetensors checkpoint header-only (no tensor data loaded), reads ALL keys via `f.keys()` (P904 regression prevention), inspects `get_slice(key).get_shape()` for each key, and returns a dict of inferred hyperparameters including hidden_dim, double_block_count, single_block_count, latent_channels, latent_height, latent_width, patch_size, and arch string. Handles both the regular ZiT fixture (with `arch` metadata) and the no-metadata fixture (metadata-fallback path deriving arch from key naming patterns). Created 4 tests in `worker/tests/test_arch_zit.py` covering regular fixture, no-metadata fixture, non-existent path, and truncated header.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| python | safetensors | 0.8.0            | pypi-query MCP |

No new dependency introduced — `safetensors` is already pinned in `worker/requirements/base.txt`. The `safe_open(path, framework="pt")` API and `get_slice(key).get_shape()` method are confirmed live in 0.8.0.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/diffusion/zit.py` | New file; `_infer_hyperparams()` function implementing step 1 of §11.3 loading contract. |
| CREATE | `worker/nodes/arch/diffusion/zit.py` (inner) | `_infer_hyperparams_inner()` helper factored out for clean exception wrapping. |
| CREATE | `worker/tests/test_arch_zit.py` | New test file; 4 tests for `_infer_hyperparams()` against both fixture variants and error cases. |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new tests. |

## Commit Log

```
 .forge/reports/P20-B1_plan.md      | 154 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +-
 docs/TESTS.md                      |  48 +++++++
 worker/nodes/arch/diffusion/zit.py | 253 +++++++++++++++++++++++++++++++++++++
 worker/tests/test_arch_zit.py      | 122 ++++++++++++++++++
 6 files changed, 587 insertions(+), 9 deletions(-)
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

worker/tests/test_arch_zit.py::test_infer_hyperparams_regular_fixture PASSED [ 25%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED [ 50%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED [ 75%]
worker/tests/test_arch_zit.py::test_infer_hyperparams_truncated_header_raises PASSED [100%]

============================== 4 passed in 1.89s ===============================
```

Full worker mock-mode suite (96 passed, 31 deselected):
```
====================== 96 passed, 31 deselected in 3.76s =======================
```

## Format Gate

```
(Not applicable — cargo fmt --all -- --check exited 0 with no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.26s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 58.94s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.37s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.22s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  Running tests/config_reference.rs
  running 1 test
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passes. No other gates triggered (no handler signatures, node types, or arch module methods modified).

## Public API Delta

```
(no output — no new pub items introduced)
```

The sole function `_infer_hyperparams()` uses the underscore prefix convention, making it internal-but-testable. No `pub` items introduced. The helper `_infer_hyperparams_inner()` is also underscore-prefixed (internal implementation detail).

## Deviations from Plan

- **Exception wrapping:** The plan specified raising `ValueError` for malformed/truncated input, but `safe_open()` raises `FileNotFoundError` (for missing files) and `SafetensorError` (for corrupted headers). I wrapped the `safe_open` call in try/except to convert these to `ValueError` with descriptive messages, providing a uniform error interface for callers.
- **Block counting fallback:** The plan specified `double_blocks.\d+.*` pattern for counting. The no-metadata fixture uses `xyz_double_block_*` keys without numeric suffixes. I implemented a dual-pattern approach: primary pattern extracts numeric suffixes; if no matches, falls back to counting keys containing `double_block` and dividing by 2 (since each block has 2 tensors).
- **Latent key matching:** The plan specified the `latents` key, but the no-metadata fixture uses `xyz_latents`. I used `endswith("latents")` to match both.
- **Hidden_dim inference:** The plan specified exact key names (`input_proj.weight`, `time_text_emb.weight`, `c_crossattn_dim`), but the no-metadata fixture uses `xyz_c_crossattn_dim`. I used `endswith()` matching for all three.
- **Function split:** I factored the logic into `_infer_hyperparams()` (public, with exception wrapping) and `_infer_hyperparams_inner()` (internal, runs inside the `with safe_open` context) for cleaner exception handling. The plan described a single function.

## Blockers

None.
