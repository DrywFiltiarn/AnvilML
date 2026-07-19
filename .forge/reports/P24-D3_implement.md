# Implementation Report: P24-D3

| Field       | Value                                                         |
|-------------|---------------------------------------------------------------|
| Task ID     | P24-D3                                                        |
| Phase       | 024 — Generic Conditioning/Sampling/Decode Nodes, Real Mode   |
| Description | worker/nodes/image.py: ImageResize node, mock + real (lanczos default) |
| Implemented | 2026-07-19T18:30:00Z                                          |
| Status      | COMPLETE                                                      |

## Summary

Added the `ImageResize` node to `worker/nodes/image.py` with exact `NODE_TYPE`, `CATEGORY`, `INPUT_SLOTS`, and `OUTPUT_SLOTS` per ANVILML_DESIGN.md §10.3. The node accepts an image (required), width (required), height (required), and optional method string (defaults to "lanczos"). Both mock and real branches implement the resize — mock returns a sentinel dict with dimensions, real calls `PIL.Image.resize()` with the resolved Pillow 12.x filter. Added 5 unit tests covering mock dimensions, real dimensions, default lanczos, explicit bilinear, and unrecognized method error. All 14 tests in `test_nodes_image.py` pass (7 mock + 7 real).

## Resolved Dependencies

| Type   | Name   | Version resolved | Source         |
|--------|--------|------------------|----------------|
| python | pillow | 12.3.0           | pypi-query MCP |

Pillow 12.3.0 confirmed filter names: `LANCZOS`, `NEAREST`, `BILINEAR`, `BICUBIC`, `BOX`. The `resample` parameter (not `filter`) is used — `filter` was removed in Pillow 10.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/image.py` | Added `ImageResize` class (122 lines) after `SaveImage`; added `from PIL import Image` at module level |
| MODIFY | `worker/tests/test_nodes_image.py` | Added 5 ImageResize tests (149 lines) |
| MODIFY | `docs/TESTS.md` | Added 5 test entries for ImageResize tests |

## Commit Log

```
 .forge/reports/P24-D3_plan.md    | 275 +++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +-
 docs/TESTS.md                    |  60 +++++++++
 worker/nodes/image.py            | 122 +++++++++++++++++
 worker/tests/test_nodes_image.py | 149 +++++++++++++++++++++
 6 files changed, 616 insertions(+), 9 deletions(-)
```

## Test Results

### Mock-mode tests (7 passed):
```
worker/tests/test_nodes_image.py::test_save_image_mock_emits_image_ready PASSED
worker/tests/test_nodes_image.py::test_save_image_in_registry PASSED
worker/tests/test_nodes_image.py::test_save_image_missing_image_input_raises PASSED
worker/tests/test_nodes_image.py::test_resize_mock_returns_correct_dimensions PASSED
worker/tests/test_nodes_image.py::test_resize_default_method_is_lanczos PASSED
worker/tests/test_nodes_image.py::test_resize_explicit_method_bilinear PASSED
worker/tests/test_nodes_image.py::test_resize_unrecognized_method_raises_error PASSED
```

### Real-mode tests (7 passed in image.py, 132 total):
```
worker/tests/test_nodes_image.py::test_save_image_real_emits_png PASSED
worker/tests/test_nodes_image.py::test_save_image_real_seed_pass_through PASSED
worker/tests/test_nodes_image.py::test_save_image_real_steps_pass_through PASSED
worker/tests/test_nodes_image.py::test_save_image_real_default_seed_steps PASSED
worker/tests/test_nodes_image.py::test_save_image_real_png_bytes_valid PASSED
worker/tests/test_nodes_image.py::test_save_image_real_returns_empty_dict PASSED
worker/tests/test_nodes_image.py::test_resize_real_produces_requested_dimensions PASSED
```

### Full suite:
- Mock-mode: 149 passed, 132 deselected
- Real-mode: 132 passed, 149 deselected

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0; no Rust files modified)
```

## Platform Cross-Check

Not applicable — no Rust files were modified. No `cargo check` or cross-check commands needed.

## Project Gates

- **Gate 4 — Mock/Real Parity Markers:** All node files have both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers. All markers reference collectible tests (verified via `pytest --collect-only`).
- **Gate 3 — Node Parity:** `worker/tests/test_parity.py` does not exist yet; gate not applicable for this task.
- **Gate 1 — Config Surface Sync:** Not applicable — no `ServerConfig` changes.
- **Gate 2 — OpenAPI Drift:** Not applicable — no handler signature changes.

## Public API Delta

No new `pub` items (Python does not use `pub` visibility). The new `ImageResize` class is module-level and auto-registered via `@register`. Verified collectible via subprocess import test pattern.

## Deviations from Plan

1. **Pillow 12.x `resample` parameter:** The approved plan referenced `filter=` for `PIL.Image.resize()`, but Pillow 12.x uses `resample=` (the `filter` keyword was removed in Pillow 10). Used `resample=filter_constant` instead. Documented in inline comment.

2. **Mock branch skips PIL resize:** In mock mode, test fixtures pass a dict sentinel (`{"mock": True, ...}`) as the image input, not a real PIL.Image. The mock branch now skips the resize call and returns the sentinel dimensions directly, matching SaveImage's mock pattern. The real branch performs the actual `PIL.Image.resize()` call. This was discovered during implementation and fixed before tests were written.

3. **Module-level PIL import:** Added `from PIL import Image` at module level (line 13) because the filter_map in `execute()` references `Image.LANCZOS`, etc. SaveImage imports PIL locally inside `execute()`, but ImageResize needs it at module level for the filter constants. PIL is a lightweight dependency (no torch) and does not violate the torch-free mock collection safety principle.

## Blockers

None.
