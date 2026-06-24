# Implementation Report: P904-A6b

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P904-A6b                           |
| Phase         | 904 — retrofit                     |
| Description   | Remove vestigial vae parameter from sample()/loader_fn |
| Implemented   | 2026-06-24T07:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Removed the vestigial `vae` parameter from `sample()` in `worker/nodes/arch/diffusion/zit.py` — its signature, docstring, and the `loader_fn`'s `ZImagePipeline(...)` call. Updated two test call sites in `worker/tests/test_arch_zit.py` to remove the now-unused `vae=None` and `vae=mock_vae` arguments and the `mock_vae` variable. Updated `docs/TESTS.md` to remove the mock VAE reference from the affected test entry. All 12 tests in the module pass; all Rust checks, clippy, platform cross-checks, and project gates pass.

## Resolved Dependencies

None. This task removes a parameter; no dependency changes.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/nodes/arch/diffusion/zit.py | Removed `vae: Any = None,` from `sample()` signature, removed `vae:` bullet from docstring Args section, removed `vae=vae,` from `ZImagePipeline(...)` call, removed stale VAE comment |
| MODIFY | worker/tests/test_arch_zit.py | Removed `vae=None,` from `test_sample_real_assembles_pipeline_via_cache`; removed `mock_vae = MagicMock()` and `vae=mock_vae,` from `test_sample_real_invokes_pipeline_with_correct_args`; updated docstring to remove mock VAE reference |
| MODIFY | docs/TESTS.md | Removed "mock VAE" from `test_sample_real_invokes_pipeline_with_correct_args` Context line |

## Commit Log

```
 .forge/state/CURRENT_TASK.md       |  6 +++---
 .forge/state/state.json            | 13 +++++++------
 docs/TESTS.md                      |  2 +-
 worker/nodes/arch/diffusion/zit.py |  5 -----
 worker/tests/test_arch_zit.py      |  7 ++-----
 5 files changed, 13 insertions(+), 20 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 12 items

worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [  8%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [ 16%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [ 25%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [ 33%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [ 41%]
worker/tests/test_arch_zit.py::test_sample_real_assembles_pipeline_via_cache PASSED [ 50%]
worker/tests/test_arch_zit.py::test_sample_real_invokes_pipeline_with_correct_args PASSED [ 58%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [ 66%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [ 75%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [ 83%]
worker/tests/test_arch_zit.py::test_make_callback_emits_progress PASSED  [ 91%]
worker/tests/test_arch_zit.py::test_make_callback_raises_on_cancellation PASSED [100%]

============================== 12 passed in 6.70s ==============================
```

## Format Gate

```
(Exit 0 — no output from `cargo fmt --all -- --check`)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.00s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.63s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  running 1 test
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed

# Gate 2 — OpenAPI Drift
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
  (Exit 0 — no diff)
```

## Public API Delta

```
(No new pub items introduced — grep returned empty.)
```

## Deviations from Plan

- The approved plan's step 6 said "No change needed" for `test_sample_real_assembles_pipeline_via_cache` in TESTS.md because it had no VAE references. Confirmed during inspection: that entry was already clean. No change was needed there.
- The approved plan's step 6 said "update the Context to remove the mention of mock VAE" for `test_sample_real_invokes_pipeline_with_correct_args`. This was done — the Context line no longer mentions "mock VAE".

## Blockers

None.
