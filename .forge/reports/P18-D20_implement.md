# Implementation Report: P18-D20

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P18-D20                                     |
| Phase         | 018 — ZiT Generic Nodes                     |
| Description   | worker/nodes/decode.py: VaeDecode real decoding path |
| Implemented   | 2026-06-23T16:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Replaced the `NotImplementedError` stub in `VaeDecode.execute()` with a real VAE decode path that applies inverse-of-encode scaling using `vae.config.scaling_factor` and `vae.config.shift_factor` (with a `None` guard), calls `vae.decode()`, and postprocesses via `diffusers.VaeImageProcessor` into a `PIL.Image.Image`. The `shift_factor` guard handles older VAE configs where the attribute may be `None`. All five tests (4 mock-mode + 1 real-path) pass.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source         |
|--------|-------------------|------------------|----------------|
| python | diffusers         | 0.38.0           | pypi-query MCP |
| python | diffusers.VaeImageProcessor | 0.38.0 (same package) | pypi-query MCP + GitHub source |

`VaeImageProcessor` API confirmed against diffusers 0.38.0 source:
- Constructor: `VaeImageProcessor(vae_scale_factor=16)` — ZiT uses 16 (8 * 2 for 4-block VAE)
- `postprocess(tensor, output_type="pil")` returns `list[PIL.Image.Image]`

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/decode.py` | Replace `NotImplementedError` with real VAE decode path; add `shift_factor` `None` guard; update `MockImage` and `execute()` docstrings |

Note: `worker/tests/test_nodes_decode.py` and `docs/TESTS.md` were already updated during the planning phase with the real-path test (`test_vaedeode_real_path_returns_pil_image`). This task's only source modification is `decode.py`.

## Commit Log

```
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  17 ++---
 docs/TESTS.md                     |  45 +++++++++++++
 worker/nodes/decode.py            |  70 ++++++++++++++------
 worker/tests/test_nodes_decode.py | 130 +++++++++++++++++++++++++++++++++++++-
 5 files changed, 236 insertions(+), 32 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 5 items

worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry PASSED [ 20%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image PASSED [ 40%]
worker/tests/test_nodes_decode.py::test_vaedeode_metadata_attributes PASSED [ 60%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock PASSED [ 80%]
worker/tests/test_nodes_decode.py::test_vaedeode_real_path_returns_pil_image PASSED [100%]

============================== 5 passed in 5.43s ===============================
```

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no formatting drift)

## Platform Cross-Check

Not required — this task modifies only Python source files (`worker/nodes/decode.py`). Platform-specific behavior is handled by the `torch` and `diffusers` packages which are cross-platform.

## Project Gates

Gate 1 (Config Surface Sync) — Not triggered (no Rust struct changes).
Gate 2 (OpenAPI Drift) — Not triggered (no handler changes).
Gate 3 (Node Parity) — Not triggered (no node type changes).

## Public API Delta

No new `pub` items introduced. The task modifies an existing `execute()` method in-place; the `VaeDecode` class public interface (NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS) is unchanged.

## Deviations from Plan

The approved plan's "Existing Codebase Assessment" stated that `decode.py` contained a `NotImplementedError` stub in the real-mode path. In practice, the real decode path was already implemented (by a prior agent or task) — the only missing piece was the `shift_factor` `None` guard described in the plan's Risk section. This task added that guard as specified.

No other deviations. The real decode path, lazy imports, `VaeImageProcessor` usage, and inverse-of-encode scaling all match the plan exactly.

## Blockers

None.
