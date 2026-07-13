# Implementation Report: P20-B2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P20-B2                          |
| Phase         | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description   | worker/nodes/arch/diffusion/zit.py: can_handle() + dispatch registration |
| Implemented   | 2026-07-13T16:30:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented `can_handle(key: str) -> bool` in `worker/nodes/arch/diffusion/zit.py` with a module-level `ARCH = "zit"` constant, registered `zit` as the first real entry in `worker/nodes/arch/diffusion/__init__.py`'s `_REGISTERED_MODULES` list, and added 3 new tests in `worker/tests/test_arch_zit.py` covering match, rejection, and dispatcher integration. Also fixed 3 pre-existing tests in `worker/tests/test_arch_dispatch.py` that assumed an empty registry — they now use a pytest fixture to temporarily clear zit from the registry. Total test count: 7 (4 existing + 3 new).

## Resolved Dependencies

None. This task uses only Python standard library types (`str`, `bool`) and existing project modules.

| Type | Name | Version resolved | Source |
|------|------|-----------------|--------|
| (none) | — | — | — |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `ARCH = "zit"` constant and `can_handle(key: str) -> bool` function with Google-style docstring |
| MODIFY | `worker/nodes/arch/diffusion/__init__.py` | Import `zit` from `worker.nodes.arch.diffusion` and append to `_REGISTERED_MODULES` |
| MODIFY | `worker/tests/test_arch_zit.py` | Add 3 new tests: `test_can_handle_matches_zit`, `test_can_handle_rejects_unrelated_key`, `test_get_module_returns_zit_for_matching_key`; import `get_module`, `zit`, and `can_handle` |
| MODIFY | `worker/tests/test_arch_dispatch.py` | Add `pytest.fixture(autouse=True)` to clear diffusion registry before each test, fixing 3 tests that assumed empty registry |
| MODIFY | `docs/TESTS.md` | Add 3 entries for the new tests |

## Commit Log

```
 .forge/reports/P20-B2_plan.md           | 113 ++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md            |   6 +-
 .forge/state/state.json                 |  13 ++--
 docs/TESTS.md                           |  36 ++++++++++
 worker/nodes/arch/diffusion/__init__.py |   4 +-
 worker/nodes/arch/diffusion/zit.py      |  23 +++++++
 worker/tests/test_arch_dispatch.py      |  23 ++++++-
 worker/tests/test_arch_zit.py           |  40 ++++++++++-
 8 files changed, 243 insertions(+), 15 deletions(-)
```

## Test Results

### Python mock-mode (99 passed)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 130 items / 31 deselected / 99 selected

worker/tests/test_arch_dispatch.py::test_get_module_returns_none_when_empty PASSED
worker/tests/test_arch_dispatch.py::test_get_module_does_not_raise_for_various_key_types PASSED
worker/tests/test_arch_dispatch.py::test_get_module_skips_module_with_can_handle_false PASSED
worker/tests/test_arch_dispatch.py::test_clip_get_module_returns_none_when_empty PASSED
worker/tests/test_arch_dispatch.py::test_clip_get_module_does_not_raise_for_various_key_types PASSED
worker/tests/test_arch_dispatch.py::test_clip_get_module_skips_module_with_can_handle_false PASSED
worker/tests/test_arch_dispatch.py::test_vae_get_module_returns_none_when_empty PASSED
worker/tests/test_arch_dispatch.py::test_vae_get_module_does_not_raise_for_various_key_types PASSED
worker/tests/test_arch_dispatch.py::test_vae_get_module_skips_module_with_can_handle_false PASSED
worker/tests/test_arch_zit.py::test_infer_hyperparams_regular_fixture PASSED
worker/tests/test_arch_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED
worker/tests/test_arch_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED
worker/tests/test_arch_zit.py::test_infer_hyperparams_truncated_header_raises PASSED
worker/tests/test_arch_zit.py::test_can_handle_matches_zit PASSED
worker/tests/test_arch_zit.py::test_can_handle_rejects_unrelated_key PASSED
worker/tests/test_arch_zit.py::test_get_module_returns_zit_for_matching_key PASSED
... (93 more passed) ...

====================== 99 passed, 31 deselected in 3.53s =======================
```

### Python real-mode (31 passed)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 130 items / 99 deselected / 31 selected

worker/tests/test_capability.py::TestProbeDtypes::test_fp32_cpu_returns_true PASSED
... (30 more passed) ...

====================== 31 passed, 99 deselected in 2.02s =======================
```

### Rust full workspace (all passed)

```
cargo test --workspace --features mock-hardware
... all crates passed ...
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (cli_help)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (config_reference)
... all sub-crate tests passed ...
all doctests ran in 0.75s; merged doctests compilation took 0.73s
```

## Format Gate

```
cargo fmt --all -- --check
# exited 0 — no output (clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.07s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.96s

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.53s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.96s
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:** Not triggered — task modifies no handler signatures or ToSchema derives.

**Gate 3 — Node Parity:** Not triggered — task modifies no node types. `test_parity.py` does not exist yet.

**Gate 4 — Mock/Real Parity Markers:** Not triggered — `can_handle()` is not in the dual-mode marker scope (`ANVILML_DESIGN.md §10.4` covers only `load()`, `sample()`, `decode()`, `compute_latent_shape()`, and `execute()`).

## Public API Delta

```
git diff --cached HEAD -- worker/nodes/arch/diffusion/zit.py | grep '^+' | grep -E '(ARCH|def can_handle)'
+ARCH: str = "zit"
+def can_handle(key: str) -> bool:
+    return key == ARCH
```

New public items (Python module-level):
- `ARCH: str = "zit"` — module-level constant in `worker.nodes.arch.diffusion.zit`
- `can_handle(key: str) -> bool` — module-level function in `worker.nodes.arch.diffusion.zit`

Both match the plan's Public API Surface table exactly.

## Deviations from Plan

- **Fix to `test_arch_dispatch.py`:** The plan's `Files Affected` table listed only `zit.py`, `__init__.py`, and `test_arch_zit.py`. However, 3 pre-existing tests in `test_arch_dispatch.py` (`test_get_module_returns_none_when_empty`, `test_get_module_does_not_raise_for_various_key_types`, `test_get_module_skips_module_with_can_handle_false`) assumed an empty `_REGISTERED_MODULES` list. Since this task registers `zit` at import time, these tests fail without modification. Fixed by adding a `@pytest.fixture(autouse=True)` that clears the diffusion registry before each test and restores it afterward. This is a direct consequence of the registration change and is required for the test suite to pass.
- **Test assertion style in `test_get_module_returns_zit_for_matching_key`:** The plan's risk assessment suggested using `result.__name__ == "zit"` as a workaround for module identity comparison. The actual module name is `worker.nodes.arch.diffusion.zit`, not `zit`. Fixed by using identity comparison (`result is zit`) after importing the `zit` module in the test file. This is more robust than name comparison.

## Blockers

None.
