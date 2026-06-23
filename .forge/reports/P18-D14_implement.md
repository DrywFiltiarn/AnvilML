# Implementation Report: P18-D14

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P18-D14                                           |
| Phase         | 018 — ZiT Generic Nodes                           |
| Description   | worker/nodes/loader.py: LoadVae single-file path via from_single_file(), fixes ctx bug |
| Implemented   | 2026-06-23T10:15:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Replaced `LoadVae.execute()`'s real-mode loading path from `AutoencoderKL.from_pretrained(model_id, subfolder="vae", torch_dtype=torch.bfloat16)` to `AutoencoderKL.from_single_file(model_id, torch_dtype=torch.bfloat16)`, making `LoadVae` consistent with `LoadModel` (P18-D13). Created the `_load_from_hf_directory()` helper function preserving the original directory-based code, never called but kept for future reactivation. Verified that the `ctx.pipeline_cache` bug referenced in the task description was already fixed — the source uses `self.ctx.pipeline_cache.get_or_load()` on line 353 (now 363), not bare `ctx.pipeline_cache`. Updated the module docstring to describe all three loader nodes. All 11 Python loader tests pass, all 230+ Rust tests pass, all format/lint/cross-check gates pass.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | diffusers | 0.38.0           | pypi-query MCP |

`AutoencoderKL.from_single_file()` is confirmed available via `FromOriginalModelMixin` in diffusers 0.38.0. No new dependencies added.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Updated module docstring; replaced `from_pretrained` with `from_single_file` in `LoadVae.execute()` loader_fn; created `_load_from_hf_directory()` helper; updated inline comments |

## Commit Log

```
 .forge/reports/P18-D14_plan.md | 153 +++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md   |   6 +-
 .forge/state/state.json        |  13 ++--
 worker/nodes/loader.py         |  60 +++++++++++++---
 4 files changed, 213 insertions(+), 19 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 11 items

worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [  9%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 18%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 27%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 36%]
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED [ 45%]
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED [ 54%]
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED [ 63%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 72%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 81%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 90%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [100%]

============================== 11 passed in 0.05s ==============================
```

## Format Gate

```
(no output — cargo fmt --all -- --check exited 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.58s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
```

## Project Gates

### Gate 1 — Config Surface Sync
```
running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not applicable — task does not modify handler signatures, ToSchema derives, or AppState fields.

### Gate 3 — Node Parity
Not applicable — task does not add, remove, or rename a node type. The LoadVae node type string remains `"LoadVae"`.

### Full py_compile gate
```
(no output — all worker/*.py files compiled successfully)
```

## Public API Delta

```
(no output — no new pub items introduced)
```

No new public items. The `_load_from_hf_directory()` function is module-private (underscore-prefixed) and not exported in `__all__`. No changes to any class signatures or public method signatures.

## Deviations from Plan

- The plan step 1 (verify the bare `ctx.pipeline_cache` bug) was completed at ACT time: the source already uses `self.ctx.pipeline_cache.get_or_load(...)` on line 353 (now 363). The bare `ctx.pipeline_cache` bug described in the task context does not exist in the current source. No code change was needed for this step. Documented here rather than listed as a deviation.
- The module docstring was updated to describe all three loader nodes (LoadModel, LoadVae, LoadClip) rather than just LoadModel. This is a minor scope expansion from the plan's "MODIFY loader.py" bullet, but it provides necessary context about the module's full purpose and follows the established docstring convention used elsewhere in the crate.

## Blockers

None.
