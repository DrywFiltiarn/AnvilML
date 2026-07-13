# Implementation Report: P20-E1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P20-E1                          |
| Phase         | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description   | Runnable Proof: LoadModel node loads the ZiT fixture checkpoint for real |
| Implemented   | 2026-07-14T00:38:00Z            |
| Status        | COMPLETE                        |

## Summary

This task executed the phase's Runnable Proof: running the real-mode pytest suites for
`test_arch_zit.py` and `test_nodes_loader.py` against the P20-A1 ZiT fixture checkpoint.
The full real-mode chain succeeds — shape inference, meta construction, dtype selection,
key remapping, weight loading, and LoadModel's real branch — with zero skips and zero
xfails. One prerequisite defect was fixed: three tests in `test_arch_zit.py` that were
intended to be real-mode tests (named with `_real` suffix and carrying `REAL_PATH_VERIFIED`
markers) lacked the `@pytest.mark.real_mode` decorator, so they were not collected by
`-m real_mode`. The decorator was added to each.

## Resolved Dependencies

None. This task introduces no new dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/tests/test_arch_zit.py | Added `@pytest.mark.real_mode` decorator to 3 real-mode tests |

## Commit Log

```
 .forge/reports/P20-E1_plan.md | 179 ++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 +--
 worker/tests/test_arch_zit.py |   3 +
 4 files changed, 192 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 37 items / 27 deselected / 10 selected

worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real PASSED     [ 10%]
worker/tests/test_arch_zit.py::test_load_real_zit_fixture PASSED         [ 20%]
worker/tests/test_arch_zit.py::test_load_no_metadata_real PASSED         [ 30%]
worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture PASSED [ 40%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented PASSED [ 50%]
worker/tests/test_nodes_loader.py::test_load_vae_real_cache_key_format PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_no_diffusion_arch PASSED [ 70%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_not_implemented PASSED [ 80%]
worker/tests/test_nodes_loader.py::test_load_clip_real_cache_key_format PASSED [ 90%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_no_diffusion_arch PASSED [100%]

====================== 10 passed, 27 deselected in 2.50s =======================
```

- 10 tests collected (3 from `test_arch_zit.py`, 7 from `test_nodes_loader.py`)
- 0 skipped, 0 xfailed
- All 10 tests PASSED

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.81s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 61.00s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 64.00s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 63.00s
```

All four checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
    Running tests/config_reference.rs
    running 1 test
    test tests::config_reference_matches_defaults ... ok
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passes.

## Public API Delta

```
(no new pub items — only test file modifications)
```

## Deviations from Plan

- **Added `@pytest.mark.real_mode` decorator to 3 tests in `test_arch_zit.py`.** The approved
  plan listed these tests as real-mode tests and expected them to be collected by
  `pytest -m real_mode`, but they lacked the decorator. This was a prerequisite defect from
  the prior tasks (P20-B1 through P20-D1) that authored the test functions with `_real`
  suffixes and `REAL_PATH_VERIFIED` markers but omitted the pytest marker. The fix was a
  one-line decorator addition per test. Without this fix, only 7 of the expected 10 tests
  would have been collected, and the 3 zit-specific tests would have been silently skipped
  by the real-mode filter.

## Blockers

None.

## Runnable Proof Transcript

The acceptance criterion command was executed verbatim:

```bash
worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py worker/tests/test_nodes_loader.py -v -m real_mode
```

Output (verbatim, from test results section above):

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 37 items / 27 deselected / 10 selected

worker/tests/test_arch_zit.py::test_dtype_selection_bf16_real PASSED     [ 10%]
worker/tests/test_arch_zit.py::test_load_real_zit_fixture PASSED         [ 20%]
worker/tests/test_arch_zit.py::test_load_no_metadata_real PASSED         [ 30%]
worker/tests/test_nodes_loader.py::test_load_model_real_loads_zit_fixture PASSED [ 40%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented PASSED [ 50%]
worker/tests/test_nodes_loader.py::test_load_vae_real_cache_key_format PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_no_diffusion_arch PASSED [ 70%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_not_implemented PASSED [ 80%]
worker/tests/test_nodes_loader.py::test_load_clip_real_cache_key_format PASSED [ 90%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_no_diffusion_arch PASSED [100%]

====================== 10 passed, 27 deselected in 2.50s =======================
```

Acceptance criteria satisfied:
- [x] Command exits 0
- [x] Zero SKIPPED markers in selected test set
- [x] Zero XFAIL markers in selected test set
- [x] All 3 real-mode tests from `test_arch_zit.py` show PASSED
- [x] All 7 real-mode tests from `test_nodes_loader.py` show PASSED
- [x] Literal output recorded above
