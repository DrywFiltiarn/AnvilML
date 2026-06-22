# Implementation Report: P18-D3b

| Field         | Value                                         |
|---------------|-----------------------------------------------|
| Task ID       | P18-D3b                                       |
| Phase         | 018 — ZiT Generic Nodes                       |
| Description   | worker/nodes/arch/zit.py: add compute_latent_shape() function |
| Implemented   | 2026-06-22T11:00:00Z                          |
| Status        | COMPLETE                                      |

## Summary

Added the `compute_latent_shape(batch_size, height, width, num_channels_latents) -> tuple[int, ...]` function to `worker/nodes/arch/zit.py`, implementing the exact latent shape formula from `ZImagePipeline.prepare_latents`. The function computes `h = 2 * (height // (VAE_SCALE_FACTOR * 2))` and `w = 2 * (width // (VAE_SCALE_FACTOR * 2))`, returning `(batch_size, num_channels_latents, h, w)`. Updated `__all__` to export the new function. Added two unit tests verifying the canonical ZiT case (1024×1024 → 128×128) and a non-divisible edge case. All 9 tests pass (7 existing + 2 new).

## Resolved Dependencies

None. This task introduces no new external dependencies. It is a pure Python function using only built-in types (`int`, `tuple`) and the existing `VAE_SCALE_FACTOR` constant.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/zit.py` | Add `compute_latent_shape()` function with Google-style docstring; update `__all__` to include `"compute_latent_shape"` in alphabetical order |
| MODIFY | `worker/tests/test_arch_zit.py` | Add `compute_latent_shape` to imports; add `test_compute_latent_shape_known_dims` and `test_compute_latent_shape_non_divisible` tests |
| MODIFY | `docs/TESTS.md` | Add two entries for the new tests following §16.1 format |

## Commit Log

```
 docs/TESTS.md                 | 18 ++++++++++++++
 worker/nodes/arch/zit.py      | 52 ++++++++++++++++++++++++++++++++++++++-
 worker/tests/test_arch_zit.py | 57 ++++++++++++++++++++++++++++++++++++++++++-
 3 files changed, 135 insertions(+), 11 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 9 items

worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [ 11%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [ 22%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [ 33%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [ 44%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [ 55%]
worker/tests/test_arch_zit.py::test_sample_real_path_raises_not_implemented PASSED [ 66%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [ 77%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [ 88%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [100%]

============================== 9 passed in 0.04s ===============================
```

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no formatting drift)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.56s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

All four checks exit 0.

## Project Gates

Gate 1 — Config Surface Sync:
```
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 — OpenAPI Drift: Not triggered (task does not modify handler signatures, `#[utoipa::path]` annotations, or `AppState` fields).

Gate 3 — Node Parity: Not triggered (task does not add, remove, or rename a node type in `worker/nodes/`, and does not modify `crates/anvilml-scheduler/src/node_registry.rs`).

## Public API Delta

No new `pub` items introduced (Python uses `__all__` for public API; the function is exported via `__all__` in `worker/nodes/arch/zit.py`).

## Deviations from Plan

None. Implementation follows the approved plan exactly:
- Function signature matches plan: `def compute_latent_shape(batch_size: int, height: int, width: int, num_channels_latents: int) -> tuple[int, ...]`
- Formula matches plan: `h = 2 * (height // (VAE_SCALE_FACTOR * 2))`, `w = 2 * (width // (VAE_SCALE_FACTOR * 2))`
- `__all__` updated with `"compute_latent_shape"` in alphabetical order
- Two tests added with the exact assertions specified in the plan
- `# defers_to: P18-D8 — consumed by EmptyLatent real path` comment added at the function body per plan instruction

## Blockers

None.
