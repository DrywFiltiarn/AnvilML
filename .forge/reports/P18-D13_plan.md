# Plan Report: P18-D13

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-D13                                           |
| Phase       | 018 — ZiT Generic Nodes                           |
| Description | worker/nodes/loader.py: LoadModel single-file path via from_single_file(), fixes ctx bug |
| Depends on  | P18-D12                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-23T08:35:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace `LoadModel`'s real-mode model loading path from `diffusers.ZImageTransformer2DModel.from_pretrained(model_id, subfolder="unet")` to `ZImageTransformer2DModel.from_single_file(model_id, torch_dtype=torch.float16)`, which loads a single `.safetensors` file directly without requiring a `config.json` or directory structure. Preserve the original directory-based code in a new `_load_from_hf_directory(model_id, arch)` function kept for future reactivation. Fix the pre-existing `ctx.pipeline_cache` → `self.ctx.pipeline_cache` bug (bare `ctx` NameError from P18-D4). All existing mock-mode tests must continue to pass unchanged.

## Scope

### In Scope
- Replace the `ZImageTransformer2DModel.from_pretrained(...)` call in `LoadModel.execute()`'s real-mode `loader_fn` with `ZImageTransformer2DModel.from_single_file(model_id, torch_dtype=torch.float16)`.
- Move the original `from_pretrained` block (including the `safe_open` metadata read, arch detection, and the `loader_fn` closure body) unchanged into a new `_load_from_hf_directory(model_id, arch) -> RealModel` function.
- Keep the existing `safe_open(model_id, framework="pt")` metadata read for arch detection — unchanged.
- Keep the result wrapped in `RealModel(transformer, arch=arch)` as before.
- Fix the `ctx.pipeline_cache` bare-name bug → `self.ctx.pipeline_cache` in `LoadModel.execute()`.
- Update the `execute()` docstring to reflect that real-mode now uses single-file loading instead of `from_pretrained`.
- All existing mock-mode tests in `worker/tests/test_nodes_loader.py` continue to pass.

### Out of Scope
None. `defers_to (from JSON): []`. This task has no deferrals.

## Existing Codebase Assessment

The `LoadModel` class in `worker/nodes/loader.py` (lines 191–308) implements a `BaseNode` with `INPUT_SLOTS=[SlotSpec("model_id", "STRING")]` and `OUTPUT_SLOTS=[SlotSpec("model", "MODEL")]`. The `execute()` method checks `ANVILML_WORKER_MOCK==1` for mock mode (returns `MockModel(arch="zit")`) and enters real mode where it:

1. Lazily imports `safe_open` from `safetensors.torch`, `ZImageTransformer2DModel` from `diffusers`, and `torch`.
2. Opens the safetensors file with `safe_open(model_id, framework="pt")` to read metadata for arch detection.
3. Defines a `loader_fn()` closure that calls `ZImageTransformer2DModel.from_pretrained(model_id, subfolder="unet", torch_dtype=torch.float16)` and wraps the result in `RealModel(transformer, arch=arch)`.
4. Calls `self.ctx.pipeline_cache.get_or_load(model_id, "fp8", loader_fn)`.

The `RealModel` wrapper class (lines 152–188) stores the transformer and arch, exposing `.arch` and `.in_channels` properties. The `NodeContext` class (in `worker/nodes/base.py`) stores `self.ctx` as the constructor parameter `ctx`, with `pipeline_cache` as a typed `dict[str, Any]` that is a `PipelineCache` instance at runtime.

The existing `_load_from_hf_directory(model_id, clip_type)` function (line 494) was added by P18-D12 for `LoadClip` and follows the same preservation pattern. No such function exists for `LoadModel` yet — the directory-based code is still inline in `execute()`.

A pre-existing bug exists: the current code already uses `self.ctx.pipeline_cache` (line 305), so the bare `ctx` bug may already be fixed in this file. However, the task context references a `ctx.pipeline_cache` NameError from P18-D4 — this is the same bug pattern that P18-D12 fixed for `LoadClip`. The plan includes verifying the exact state at ACT time; if `self.ctx` is already used, no change is needed there.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | diffusers   | 0.38.0          | pypi-query MCP | n/a                    |
| python | safetensors | (existing)      | installed      | n/a                    |

`ZImageTransformer2DModel` is confirmed to inherit `FromOriginalModelMixin` (via `ModelMixin, ConfigMixin, PeftAdapterMixin, FromOriginalModelMixin` in `diffusers/models/transformers/transformer_z_image.py:359`), which provides the `from_single_file` classmethod. The method signature is `from_single_file(cls, pretrained_model_link_or_path_or_dict: str | None = None, **kwargs) -> Self`. The `torch_dtype` kwarg is supported directly. No feature flags needed.

## Approach

1. **Read `LoadModel.execute()` carefully to identify the bare `ctx` bug.** Search for any occurrence of bare `ctx.` (not `self.ctx.`) in the real-mode path of `LoadModel.execute()`. If the bug exists, fix it to `self.ctx.pipeline_cache`. If it does not exist (already fixed by a prior task), proceed without change.

2. **Extract the existing real-mode loading code into `_load_from_hf_directory(model_id, arch) -> RealModel`.** This new function at module level (after the `LoadClip` class, similar to the existing `_load_from_hf_directory` at line 494) will contain:
   - The `safe_open(model_id, framework="pt")` metadata read for arch detection.
   - The arch path normalization (`"/"` and `"\\"` splitting).
   - The original `loader_fn` body calling `ZImageTransformer2DModel.from_pretrained(model_id, subfolder="unet", torch_dtype=torch.float16)`.
   - Return `RealModel(transformer, arch=arch)`.
   - Add a Google-style docstring explaining this is the preserved deprecated path.

3. **Replace the inline loading code in `LoadModel.execute()`'s real-mode path.** The new `loader_fn` closure will call `_load_from_hf_directory(model_id, arch)` and return its result. The `safe_open` block and arch detection are removed from inline code (they now live in `_load_from_hf_directory`). The `loader_fn` body becomes a single line: `return _load_from_hf_directory(model_id, arch)`.

4. **Add the `from_single_file` call as the active loading path.** Inside `_load_from_hf_directory`, the `loader_fn` body is replaced with `ZImageTransformer2DModel.from_single_file(model_id, torch_dtype=torch.float16)`. The function still wraps the result in `RealModel(transformer, arch=arch)` and returns it.

   Wait — re-reading the task context more carefully: the `_load_from_hf_directory` function is the *preserved* original code (with `from_pretrained`). The *active* path should use `from_single_file`. So the approach is:
   
   a. Create `_load_from_hf_directory(model_id, arch) -> RealModel` containing the original `from_pretrained` code (preserved, never called).
   b. In `LoadModel.execute()`'s real-mode path, replace the inline code with a call to a new active loading path that uses `from_single_file`. Since the task says "replace from_pretrained with from_single_file" and "the original directory-based code moves into _load_from_hf_directory", the active path in `execute()` should directly call `from_single_file` (not go through `_load_from_hf_directory`).

   Revised approach:
   a. Create `_load_from_hf_directory(model_id, arch) -> RealModel` with the original `from_pretrained` code (preserved, never called).
   b. In `LoadModel.execute()`'s real-mode path, replace the inline `safe_open` + `loader_fn` block with direct `ZImageTransformer2DModel.from_single_file(model_id, torch_dtype=torch.float16)` call, wrapping in `RealModel(transformer, arch=arch)`, then passing through `pipeline_cache.get_or_load()`.

5. **Fix the `ctx` bug if present.** Change any bare `ctx.pipeline_cache` to `self.ctx.pipeline_cache` in `LoadModel.execute()`.

6. **Update the `execute()` docstring.** Change the `Raises` section from "NotImplementedError" (which is no longer accurate for real mode) and update the description to reflect single-file loading.

7. **Verify all existing tests pass.** Run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` — mock mode is unaffected by these changes since the mock path returns before reaching any real-mode code.

## Public API Surface

No new public items are introduced. The `_load_from_hf_directory` function is private (prefixed with `_`). No changes to any `NODE_TYPE`, `INPUT_SLOTS`, `OUTPUT_SLOTS`, or class-level attributes. The existing `RealModel` wrapper class is unchanged in its public interface.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Replace `from_pretrained` with `from_single_file` in LoadModel's real path; extract original code into `_load_from_hf_directory(model_id, arch)`; fix `ctx` bug |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_registered_in_registry` | LoadModel is registered in NODE_REGISTRY | NODE_REGISTRY cleared by fixture; loader reloaded | None | "LoadModel" in NODE_REGISTRY, NODE_TYPE == "LoadModel" | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_execute_returns_mock_model` | execute() returns MockModel with arch="zit" in mock mode | ANVILML_WORKER_MOCK=1 | model_id="test-model" | result["model"] is MockModel, arch == "zit" | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_execute_missing_model_id_defaults_empty` | execute() handles missing model_id in mock mode | ANVILML_WORKER_MOCK=1 | (no model_id) | result["model"] is MockModel(arch="zit") | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_metadata_attributes` | All six metadata attributes on LoadModel are correct | None | None | NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS correct | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes -v` exits 0 |

## CI Impact

No CI changes required. This task modifies only `worker/nodes/loader.py`, which is already covered by the existing `worker-linux` and `worker-windows` CI jobs that run `pytest worker/tests/`. No new file types, gates, or test modules are introduced. The `py_compile` step (Step 7 in ENVIRONMENT.md §6) will validate syntax of the modified file.

## Platform Considerations

None identified. The `from_single_file` method is platform-neutral — it reads a local `.safetensors` file and loads weights into PyTorch tensors. The `torch_dtype=torch.float16` argument is handled by PyTorch's dtype system, which is cross-platform. No `#[cfg]` guards or platform-specific code needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ZImageTransformer2DModel.from_single_file()` signature may differ from the task context's `from_single_file(model_id, torch_dtype=torch.float16)` — the actual first positional argument name is `pretrained_model_link_or_path_or_dict`, not `model_id`. | Low | Medium | MCP confirmed: the method signature is `from_single_file(cls, pretrained_model_link_or_path_or_dict: str \| None = None, **kwargs)`. The task context's `model_id` maps to `pretrained_model_link_or_path_or_dict`. ACT agent passes `model_id` as the positional first argument (after `cls`), which maps correctly. |
| The `safe_open` metadata read produces `arch` that may not match what `from_single_file` expects — if the safetensors file has no `arch` key, arch falls back to the path basename which may not be "zit". | Low | Medium | The existing arch detection logic is preserved unchanged in `_load_from_hf_directory`. The active path reuses the same `safe_open` + arch detection before calling `from_single_file`, so behavior is identical. |
| The `ctx.pipeline_cache` bug may already be fixed (current code shows `self.ctx.pipeline_cache` on line 305). | High | Low | The plan includes verifying the exact state at ACT time. If already fixed, no change needed — the task's "fix the ctx bug" is already satisfied. |
| `from_single_file` may have different kwargs than `from_pretrained` — e.g. it does not accept `subfolder` the same way. | Low | Medium | MCP confirmed: `from_single_file` accepts `subfolder` as a kwarg but it defaults to `""`. Since we're loading a single file (not a directory), `subfolder` is irrelevant and will be omitted. The ACT agent will confirm the exact kwargs at session start. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0
- [ ] `grep -n "from_single_file" worker/nodes/loader.py` returns at least 1 match (the active import)
- [ ] `grep -n "_load_from_hf_directory" worker/nodes/loader.py` returns at least 2 matches (definition + call from old inline code preserved inside it)
- [ ] `grep -c "from_pretrained" worker/nodes/loader.py` returns 0 (no remaining `from_pretrained` in active code — only inside `_load_from_hf_directory` if it still appears there)
