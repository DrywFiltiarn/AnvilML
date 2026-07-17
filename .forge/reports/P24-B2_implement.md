# Implementation Report: P24-B2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P24-B2                          |
| Phase         | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description   | worker/nodes/decode.py: VaeDecode real branch dispatches to vae module |
| Implemented   | 2026-07-17T22:45:00Z            |
| Status        | COMPLETE                        |

## Summary

Replaced the `NotImplementedError` stub in `VaeDecode.execute()`'s real branch with actual dispatch to the loaded VAE architecture module via `arch.vae.get_module(vae.arch).decode(vae, latent)`. The implementation validates inputs (vae, latent, .arch attribute), handles the `get_module()` returning `None` case with a descriptive `RuntimeError`, and returns the list of PIL Images from `decode()`. Added 7 new real-mode tests in `worker/tests/test_nodes_decode.py` covering end-to-end decoding, batched latents, pixel validation, dispatch key verification, and error cases. Updated `docs/TESTS.md` with entries for all 7 new tests.

## Resolved Dependencies

None. This task uses only existing imports already present in `decode.py` — `arch.vae.get_module` and `arch.vae.zit_vae.decode` were already implemented in Phase 23 (P23-D1).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/decode.py` | Replaced `NotImplementedError` stub with real dispatch to `arch.vae.get_module(vae.arch).decode(vae, latent)`; updated docstring Raises section; added `from worker.nodes import arch` import; removed `defers_to: P24-B2` comment marker. |
| MODIFY | `worker/tests/test_nodes_decode.py` | Added 7 new real-mode tests: `test_vae_decode_real_decodes_zit_vae_fixture`, `test_vae_decode_real_batched_latent`, `test_vae_decode_real_output_rgb_uint8`, `test_vae_decode_real_arch_dispatch_uses_vae_arch`, `test_vae_decode_real_missing_arch_raises`, `test_vae_decode_real_unregistered_arch_raises`, `test_vae_decode_real_missing_vae_input_raises`. Added `import pytest` and `from pathlib import Path` imports. |
| MODIFY | `docs/TESTS.md` | Added 7 new test catalogue entries for the real-mode tests above. |

## Commit Log

```
 .forge/reports/P24-B2_plan.md     | 224 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 ++-
 docs/TESTS.md                     |  70 ++++++++++++
 worker/nodes/decode.py            |  57 ++++++++--
 worker/tests/test_nodes_decode.py | 227 ++++++++++++++++++++++++++++++++++++++
 6 files changed, 580 insertions(+), 17 deletions(-)
```

## Test Results

### Mock-mode tests (141 passed, 102 deselected)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 243 items / 102 deselected / 141 selected

worker/tests/test_nodes_decode.py::test_vae_decode_class_attributes PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_mock_returns_sentinel PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_in_registry PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_decodes_zit_vae_fixture PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_batched_latent PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_output_rgb_uint8 PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_arch_dispatch_uses_vae_arch PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_missing_arch_raises PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_unregistered_arch_raises PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_missing_vae_input_raises PASSED
... (131 additional tests) ...

===================== 141 passed, 102 deselected in 5.95s ======================
```

### Real-mode tests (102 passed, 141 deselected)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 243 items / 141 deselected / 102 selected

worker/tests/test_nodes_decode.py::test_vae_decode_real_decodes_zit_vae_fixture PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_batched_latent PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_output_rgb_uint8 PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_arch_dispatch_uses_vae_arch PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_missing_arch_raises PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_unregistered_arch_raises PASSED
worker/tests/test_nodes_decode.py::test_vae_decode_real_missing_vae_input_raises PASSED
... (95 additional tests) ...

=============== 102 passed, 141 deselected, 3 warnings in 20.46s ===============
```

Note: The 3 warnings are pre-existing in `test_e2e_zit_pipeline.py` (`RuntimeWarning: invalid value encountered in cast` in `zit_vae.py:900`) and are unrelated to this task.

## Format Gate

```
Not applicable — cargo fmt --all -- --check exited 0 with no output.
```

## Platform Cross-Check

### Check 1: Mock-hardware Linux
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.57s
```

### Check 2: Mock-hardware Windows
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.81s
```

### Check 3: Real-hardware Linux
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 01s
```

### Check 4: Real-hardware Windows
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 05s
```

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 4 — Mock/Real Parity Markers
All node files with `execute()` methods (`decode.py`, `encoder.py`, `sampler.py`, `loader.py`) contain both `REAL_PATH_VERIFIED:` and `MOCK_PATH_VERIFIED:` markers. No findings.

## Public API Delta

No new `pub` items introduced. The `VaeDecode` class already existed; only the `execute()` method body was modified. No signature changes.

## Deviations from Plan

- **Test file path in TESTS.md entry:** The `test_vae_decode_real_output_rgb_uint8` entry in `docs/TESTS.md` initially had a typo (`test_nodes.py` instead of `test_nodes_decode.py`), corrected before staging.
- **Recursion fix in `test_vae_decode_real_arch_dispatch_uses_vae_arch`:** The initial test implementation imported `get_module` inside the `side_effect` function, which resolved to the patched version and caused infinite recursion. Fixed by importing the real `get_module` before entering the patch context and referencing it via closure.

## Blockers

None.
