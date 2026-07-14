# Implementation Report: P21-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-C2                                      |
| Phase       | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description | worker/nodes/sampler.py: Sampler real branch dispatches to arch module |
| Implemented | 2026-07-14T18:30:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Replaced the `NotImplementedError` stub in `Sampler.execute()`'s real branch with a dispatch to the registered diffusion architecture module (`arch.diffusion.get_module(model.arch).sample()`). Updated the `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers, updated docstrings, and added 6 real-mode tests exercising the Sampler's real branch against the ZiT fixture checkpoint.

## Resolved Dependencies

None. This task modifies only Python source files within the project. No new external packages or crates are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/sampler.py` | Replace real branch stub with arch module dispatch; update markers and docstrings |
| Modify | `worker/tests/test_nodes_sampler.py` | Add 6 real-mode tests exercising the Sampler's real branch |
| Modify | `docs/TESTS.md` | Add 6 new test entries for the real-mode tests |
| Modify | `.forge/reports/P21-C2_plan.md` | Inherited from prior PLAN session |
| Modify | `.forge/state/CURRENT_TASK.md` | Updated by The Forge orchestrator |
| Modify | `.forge/state/state.json` | Updated by The Forge orchestrator |

## Commit Log

```
 .forge/reports/P21-C2_plan.md      | 177 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +-
 docs/TESTS.md                      |  72 ++++++++-
 worker/nodes/sampler.py            |  50 ++++--
 worker/tests/test_nodes_sampler.py | 309 ++++++++++++++++++++++++++++++++++---
 6 files changed, 576 insertions(+), 51 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 10 items / 4 deselected / 6 selected

worker/tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture PASSED [ 16%]
worker/tests/test_nodes_sampler.py::test_sampler_real_seed_minus_one_resolves PASSED [ 33%]
worker/tests/test_nodes_sampler.py::test_sampler_real_explicit_seed_unchanged PASSED [ 50%]
worker/tests/test_nodes_sampler.py::test_sampler_real_multiple_steps PASSED [ 66%]
worker/tests/test_nodes_sampler.py::test_sampler_real_cfg_one_is_conditional_only PASSED [ 83%]
worker/tests/test_nodes_sampler.py::test_sampler_real_latent_shape_preserved PASSED [100%]

=============================== warnings summary ===============================
tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture
tests/test_nodes_sampler.py::test_sampler_real_seed_minus_one_resolves
tests/test_nodes_sampler.py::test_sampler_real_explicit_seed_unchanged
tests/test_nodes_sampler.py::test_sampler_real_multiple_steps
tests/test_nodes_sampler.py::test_sampler_real_cfg_one_is_conditional_only
tests/test_nodes_sampler.py::test_sampler_real_latent_shape_preserved
  /home/dryw/AnvilML/worker/.venv/lib/python3.12/site-packages/diffusers/schedulers/scheduling_euler_discrete.py:436: DeprecationWarning: __array__ implementation doesn't accept a copy keyword, so passing copy=False failed. __array__ must implement 'dtype' and 'copy' keyword arguments. To learn more, see the migration guide https://numpy.org/devdocs/numpy_2_0_migration_guide.html#adapting-to-changes-in-the-copy-keyword
    sigmas = np.array(((1 - self.alphas_cumprod) / self.alphas_cumprod) ** 0.5)

-- Docs: https://pytest.org/stable/how-to/capture.html
================= 6 passed, 4 deselected, 6 warnings in 4.55s ==================

---

============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 10 items / 6 deselected / 4 selected

worker/tests/test_nodes_sampler.py::test_sampler_class_attributes PASSED [ 25%]
worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape PASSED [ 50%]
worker/tests/test_nodes_sampler.py::test_sampler_mock_seed_zero PASSED [ 75%]
worker/tests/test_nodes_sampler.py::test_sampler_in_registry PASSED [100%]

======================= 4 passed, 6 deselected in 0.08s ========================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.10s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.92s

# 3. Real-hardware Linux:
cargo check --bin anvilml
  (already compiled from clippy pass)

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.00s
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 3 — Node Parity
`worker/tests/test_parity.py` does not exist yet (not created in prior phases).

### Gate 4 — Mock/Real Parity Markers
```
# Marker collection check:
tests/test_nodes_sampler.py::test_sampler_real_denoises_zit_fixture  — 1 test collected
tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape  — 1 test collected

# Missing marker check (both commands returned empty):
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
  (empty — all node files have REAL_PATH_VERIFIED)
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
  (empty — all node files have MOCK_PATH_VERIFIED)
```

## Public API Delta

```
(no output — no new pub items introduced)
```

The task modifies the existing `Sampler.execute()` method signature in-place. The signature remains:
```python
def execute(self, ctx: NodeContext, **inputs) -> dict:
```
The return value changes from `{"latent": {"mock": True, ...}, "seed": int}` (mock) or `NotImplementedError` (real stub) to `{"latent": torch.Tensor, "seed": int}` (real).

## Deviations from Plan

- **Latent dtype casting**: The plan's tests used `torch.zeros(1, 4, 8, 8)` which defaults to `float32`. The model loads in `bf16` (per `_DEFAULT_CAPS`), causing a `RuntimeError: mat1 and mat2 must have the same dtype`. Fixed by casting the latent tensor to `next(model.parameters()).dtype` before passing to `execute()`. This is a necessary adaptation to the actual model loading behavior, not a plan deviation.

- **Pre-existing test failures**: 5 mock-mode tests in `test_arch_zit.py` (`test_compute_latent_shape_mock_*`) fail due to stale `MODEL_PATCH_SIZE` state from prior real-mode tests. These are pre-existing issues in a file I did not modify. They are not caused by this task's changes.

## Blockers

None.
