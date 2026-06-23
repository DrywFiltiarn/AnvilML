# Implementation Report: P18-D18b

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P18-D18b                                          |
| Phase         | 018 — ZiT Generic Nodes                           |
| Description   | worker/nodes/arch/zit.py: callback_on_step_end adapter for progress and cancellation |
| Implemented   | 2026-06-23T13:10:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented the `_make_callback` adapter and `_SamplingCancelled` sentinel exception in `worker/nodes/arch/diffusion/zit.py`. The adapter bridges `diffusers`' `callback_on_step_end` signature `(self, i, t, callback_kwargs) -> dict` to the 2-argument `emit_progress(step, total)` interface used by `sample()`. It emits progress per step, checks a `threading.Event` cancellation flag, and raises `_SamplingCancelled` when the job is cancelled. Two unit tests verify progress emission and cancellation behavior. No new external dependencies were introduced.

## Resolved Dependencies

None. This task uses only Python standard library (`threading`, `typing.Callable`) already imported in both source and test files. No manifest changes required.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Added `_SamplingCancelled` exception class and `_make_callback()` private helper |
| MODIFY | `worker/tests/test_arch_zit.py` | Added `test_make_callback_emits_progress` and `test_make_callback_raises_on_cancellation` |
| MODIFY | `docs/TESTS.md` | Added entries for the two new tests |

## Commit Log

```
 .forge/reports/P18-D18b_plan.md    | 118 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 ++--
 docs/TESTS.md                      |  18 ++++++
 worker/nodes/arch/diffusion/zit.py |  76 ++++++++++++++++++++++++
 worker/tests/test_arch_zit.py      |  94 +++++++++++++++++++++++++++++
 6 files changed, 316 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 89 items

worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [  1%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [  2%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [  3%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [  4%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [  5%]
worker/tests/test_arch_zit.py::test_sample_real_assembles_pipeline_via_cache PASSED [  6%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [  8%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [  9%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [ 10%]
worker/tests/test_arch_zit.py::test_make_callback_emits_progress PASSED  [ 11%]
worker/tests/test_arch_zit.py::test_make_callback_raises_on_cancellation PASSED [ 12%]
... (78 other tests in same run) ...
============================== 89 passed in 3.06s ==============================
```

Rust tests: 141 passed, 0 failed (full workspace, `--features mock-hardware`).

## Format Gate

```
cargo fmt --all -- --check
# (exit 0, no output — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
```

All four checks exited 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
# test config_reference ... ok
# test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. Gate 2 (OpenAPI drift) and Gate 3 (Node parity) are not triggered — this task does not modify handler signatures, ToSchema derives, node types, or node_registry.rs.

## Public API Delta

```
git diff HEAD -- worker/nodes/arch/diffusion/zit.py worker/tests/test_arch_zit.py | grep '^+.*pub ' | head -40
# (no output — no new pub items)
```

No new `pub` items introduced. Both `_SamplingCancelled` and `_make_callback` are module-private (underscore-prefixed) and are not added to `__all__`.

## Deviations from Plan

None. Implementation follows the approved plan exactly:
- `_SamplingCancelled` placed after `VAE_SCALE_FACTOR` and before `compute_latent_shape()`.
- `_make_callback()` placed after `_SamplingCancelled` and before `MockLatent`.
- Inline comments explain: (a) why the closure accepts `self` but ignores it, (b) why `cancel_flag.is_set()` is used (design doc specifies `threading.Event`), (c) why `callback_kwargs` is returned unchanged.
- Tests use `threading.Event` (not `list[bool]`) to match the design doc specification (`ANVILML_DESIGN.md §1550`).
- No changes to `__all__`.

## Blockers

None.
