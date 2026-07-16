# Implementation Report: P23-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P23-B1                          |
| Phase         | 023 — ZiT VAE Arch Module       |
| Description   | worker/nodes/arch/vae/zit_vae.py: shape inference from safetensors header |
| Implemented   | 2026-07-16T19:55:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `worker/nodes/arch/vae/zit_vae.py` implementing the first step of the four-step loading contract (`ANVILML_DESIGN.md §11.3`) for the ZiT-compatible VAE architecture family. The module provides `_infer_hyperparams(path: str) -> dict[str, Any]` which reads only the safetensors header (no tensor data loaded) using `framework="np"`, infers encoder/decoder/latent channel counts, native dtype, and architecture string from VAE-specific key patterns, and returns them as a dict. Also created `worker/tests/test_arch_vae_zit.py` with 5 tests exercising both fixtures and error paths. Updated `docs/TESTS.md` with entries for all new tests.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| (stdlib) | safetensors | already in base.txt | pypi-query MCP |

No new dependencies. The `safetensors` package is already listed in `requirements/base.txt`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/vae/zit_vae.py` | New file — `_infer_hyperparams()`, `_infer_hyperparams_inner()`, `_safetensors_dtype_to_canonical()`, and `ARCH` constant. |
| CREATE | `worker/tests/test_arch_vae_zit.py` | New file — 5 tests for `_infer_hyperparams()` against both fixtures and malformed input. |
| MODIFY | `docs/TESTS.md` | Added 5 test entries for the new tests. |

## Commit Log

```
 .forge/state/CURRENT_TASK.md |  6 ++---
 .forge/state/state.json      | 13 +++++-----
 docs/TESTS.md                | 60 ++++++++++++++++++++++++++++++++++++++++++++
 3 files changed, 70 insertions(+), 9 deletions(-)
```

(Plus two untracked files: `worker/nodes/arch/vae/zit_vae.py` and `worker/tests/test_arch_vae_zit.py`)

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 5 items

worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_regular_fixture PASSED [ 20%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_no_metadata_fixture PASSED [ 40%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_nonexistent_path_raises PASSED [ 60%]
worker/tests/test_arch_vae_zit.py::test_infer_hyperparams_truncated_header_raises PASSED [ 80%]
worker/tests/test_arch_vae_zit.py::test_arch_constant PASSED             [100%]

============================== 5 passed in 1.94s ===============================
```

Full mock-mode suite: 122 passed, 75 deselected.
Full real-mode suite: 75 passed, 122 deselected.
Full Rust suite: all tests passed.

## Format Gate

```
FORMAT_PASS2_OK
```

`cargo fmt --all -- --check` exited 0.

## Platform Cross-Check

```
---CHECK1_DONE---
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.99s
---CHECK2_DONE---
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.15s
---CHECK3_DONE---
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.69s
---CHECK4_DONE---
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.92s
```

All four platform cross-checks passed:
1. Mock-hardware Linux: OK
2. Mock-hardware Windows: OK
3. Real-hardware Linux: OK
4. Real-hardware Windows: OK

## Project Gates

Gate 1 (config_reference) and Gate 2 (openapi-drift) are not triggered — this task does not modify ServerConfig fields or handler function signatures.

## Public API Delta

No new `pub` items introduced. All three functions (`_infer_hyperparams`, `_infer_hyperparams_inner`, `_safetensors_dtype_to_canonical`) are private (prefixed with `_`), matching the established pattern from `zit.py` and `qwen3.py`.

## Deviations from Plan

1. **Error message format fix**: The initial error message used `{path}` instead of `{exc}` in the ValueError message. This caused the `test_infer_hyperparams_nonexistent_path_raises` test to fail because the regex `match="No such file"` did not match. Fixed by using `{exc}` (which includes the OS error message "No such file or directory") to match the existing pattern in `zit.py` line 824.

2. **Added `test_arch_constant` test**: Added a fifth test to verify the `ARCH` constant equals `"zit_vae"`, providing explicit confirmation of the module's architecture identifier.

## Blockers

None.
