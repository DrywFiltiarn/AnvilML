# Implementation Report: P18-D18c

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P18-D18c                                          |
| Phase         | 018 — ZiT Generic Nodes                           |
| Description   | worker/nodes/arch/diffusion/zit.py: invoke pipeline with output_type=latent and return result |
| Implemented   | 2026-06-23T14:00:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Replaced the `NotImplementedError` stub in `sample()`'s real-mode path with an actual `ZImagePipeline.__call__` invocation using `output_type="latent"` and `return_dict=False`. The pipeline call is wrapped in `try/except _SamplingCancelled` to propagate cancellation. Updated the module docstring and `sample()`'s docstring to remove "not yet invoked" language. Updated the existing real-mode test `test_sample_real_assembles_pipeline_via_cache` to expect a return value instead of a `NotImplementedError`, and added a new test `test_sample_real_invokes_pipeline_with_correct_args` that verifies all expected keyword arguments are passed to the pipeline.

## Resolved Dependencies

None. The `diffusers` dependency (already declared in `worker/requirements/base.txt`) is used at runtime but no new dependencies were added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Updated module docstring, `sample()` docstring, replaced `NotImplementedError` with pipeline invocation |
| MODIFY | `worker/tests/test_arch_zit.py` | Updated `test_sample_real_assembles_pipeline_via_cache`, added `test_sample_real_invokes_pipeline_with_correct_args` |
| MODIFY | `docs/TESTS.md` | Added/updated entries for modified and new tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +--
 docs/TESTS.md                              |  15 +++-
 worker/nodes/arch/diffusion/zit.py         |  56 +++++++++----
 worker/tests/test_arch_zit.py              | 164 +++++++++++++++++++++++++++++++------
 5 files changed, 200 insertions(+), 54 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 90 items

worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [  1%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [  2%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [  3%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [  4%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [  5%]
worker/tests/test_arch_zit.py::test_sample_real_assembles_pipeline_via_cache PASSED [  6%]
worker/tests/test_arch_zit.py::test_sample_real_invokes_pipeline_with_correct_args PASSED [  7%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [  8%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [ 10%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [ 11%]
worker/tests/test_arch_zit.py::test_make_callback_emits_progress PASSED  [ 12%]
worker/tests/test_arch_zit.py::test_make_callback_raises_on_cancellation PASSED [ 13%]
... (all 90 tests passed)
============================== 90 passed in 2.73s ==============================
```

Rust tests (all passed, 220+ tests across all crates):
```
     Running tests/managed_tests.rs (target/debug/deps/managed_tests-992b374d951b7899)
running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
```

## Format Gate

```
(not applicable — cargo fmt --all -- --check exited 0 with no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.65s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

No new pub items introduced. The grep returned nothing — only docstring and implementation changes in the existing `sample()` function.

## Deviations from Plan

- **Mock `__call__` configuration:** The plan specified configuring `mock_cache.get_or_load.return_value.__call__` to return `[MagicMock(), seed]`. In practice, a `MagicMock`'s `__call__` is not a separate mock with a `return_value` attribute — calling a `MagicMock` uses the mock's own `return_value` directly. The fix was to set `mock_pipeline.return_value = [latent_result, seed]` instead, and check `mock_pipeline.call_args` (not `mock_pipeline.__call__.call_args`) for the invocation arguments. This is a standard `unittest.mock` pattern difference that the plan's pseudocode didn't capture precisely.

## Blockers

None.
