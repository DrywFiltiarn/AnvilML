# Plan Report: P904-A12

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A12                                    |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/loader.py: rewrite LoadModel/LoadVae/LoadClip's loader functions as thin per-arch wrappers |
| Depends on  | P904-A11                                    |
| Project     | anvilml                                     |
| Planned at  | 2026-06-24T11:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Refactor `worker/nodes/loader.py` so that all three loader nodes (`LoadModel`, `LoadVae`, `LoadClip`) dispatch loading through the architecture registry system instead of calling `diffusers` classes directly. `_load_model_from_hf_directory` is renamed to `_load_model_from_safetensors` and its `ZImageTransformer2DModel.from_single_file()` call is replaced with `arch.diffusion.get_module_by_name(detected_arch).load_transformer(model_id)`. `LoadVae.execute()`'s inline `loader_fn` closure is extracted into a named `_load_vae_from_safetensors(model_id, arch)` function that dispatches through `get_module_by_name(arch).load_vae(model_id)`. `LoadClip.execute()`'s dispatch is extracted into a named `_load_clip_from_safetensors(model_id, clip_type)` wrapper for naming symmetry — no behavior change since it already dispatches correctly via `arch_clip.get_module()`.

## Scope

### In Scope
- Rename `_load_model_from_hf_directory` to `_load_model_from_safetensors` in `worker/nodes/loader.py`
- Replace the `ZImageTransformer2DModel.from_single_file(...)` call inside `_load_model_from_safetensors` with `arch.diffusion.get_module_by_name(detected_arch).load_transformer(model_id)`, keeping all safetensors-metadata arch-detection logic unchanged
- Extract `LoadVae.execute()`'s inline `loader_fn` closure into a standalone `_load_vae_from_safetensors(model_id, arch)` function that dispatches through `arch.diffusion.get_module_by_name(arch).load_vae(model_id)` instead of `AutoencoderKL.from_single_file()`
- Extract `LoadClip.execute()`'s inline dispatch logic into a standalone `_load_clip_from_safetensors(model_id, clip_type)` wrapper — no behavior change, existing `arch_clip.get_module()` dispatch preserved
- Update `LoadModel.execute()` to call `_load_model_from_safetensors` by its new name
- Update `LoadVae.execute()` to call `_load_vae_from_safetensors` instead of the inline closure
- Update `LoadClip.execute()` to call `_load_clip_from_safetensors` instead of inline dispatch
- Update `worker/tests/test_nodes_loader.py`: rename the test that references `_load_model_from_hf_directory` to use `_load_model_from_safetensors`
- Verify mock-mode paths remain unchanged (all three loaders still return mock sentinels when `ANVILML_WORKER_MOCK=1`)

### Out of Scope
None. `defers_to (from JSON): []`. This task implements its full scope — no deferrals, no stubs.

## Existing Codebase Assessment

**What already exists:** `worker/nodes/loader.py` contains three loader node classes (`LoadModel`, `LoadVae`, `LoadClip`) and one private loading helper `_load_model_from_hf_directory`. `LoadModel.execute()` calls `_load_model_from_hf_directory` via `pipeline_cache.get_or_load()`. `LoadVae.execute()` defines an inline `loader_fn` closure that calls `AutoencoderKL.from_single_file()` directly. `LoadClip.execute()` already correctly dispatches through `arch_clip.get_module(clip_type).load(...)`.

The arch system in `worker/nodes/arch/diffusion/__init__.py` provides `get_module(model_obj)` which iterates over loaded modules and calls `can_handle(model_obj)` on each. `P904-A13` (prerequisite) adds `get_module_by_name(arch: str)` which creates a shim object with `.arch = arch` and passes it to `can_handle()`. The zit module (`worker/nodes/arch/diffusion/zit.py`) already implements `load_transformer(model_id)` and `load_vae(model_id)` from P904-A10/A11 — these are the functions the new wrappers will dispatch to.

**Established patterns:** All real-mode loading code uses lazy imports (torch/diffusers/safetensors imported inside the non-mock branch, never at module level). The `pipeline_cache.get_or_load(model_id, dtype, loader_fn)` pattern wraps the loading call. Device placement uses `.to(device)` with assignment of the return value. Module-level `_imported` flag ensures idempotent auto-import in arch modules.

**Gap between design doc and current source:** The design doc (ANVILML_DESIGN.md §12) describes the target state where all three loaders dispatch through the arch system. Currently only `LoadClip` does; `LoadModel` and `LoadVae` still call diffusers classes directly. This task closes that gap.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| python | diffusers  | 0.38.0          | pypi-query MCP | n/a                    |

Task context specifies diffusers 0.38.0 — confirmed by MCP result. The `load_transformer()` and `load_vae()` functions in zit.py (P904-A10/A11) already exist and use `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers` and `convert_ldm_vae_checkpoint` respectively, confirmed against the 0.38.0 API shape.

## Approach

1. **Rename `_load_model_from_hf_directory` to `_load_model_from_safetensors`** in `worker/nodes/loader.py` (line 645). Update the function's docstring to reflect the new name. Keep all internal logic unchanged: safetensors metadata reading, arch detection, path stripping, and the device placement call at the end. This is a mechanical rename — the function body stays identical except for step 2 below.

2. **Replace the direct `ZImageTransformer2DModel` call** inside `_load_model_from_safetensors`. Remove the `ZImageTransformer2DModel.from_single_file(model_id, torch_dtype=torch.float16)` call (lines 712–715) and replace it with:
   ```python
   transformer = arch.diffusion.get_module_by_name(detected_arch).load_transformer(model_id)
   ```
   This dispatches to the correct arch module's `load_transformer()` function (implemented in zit.py by P904-A10). The `detected_arch` variable is already computed earlier in the function from safetensors metadata. The device placement call `transformer = transformer.to(device)` remains after the dispatch. Remove the now-unused `from diffusers import ZImageTransformer2DModel` import since the arch module handles that internally.

3. **Extract `LoadVae.execute()`'s inline `loader_fn` into `_load_vae_from_safetensors(model_id, arch)`**. The current inline closure (lines 523–532) calls `AutoencoderKL.from_single_file(model_id, torch_dtype=torch.bfloat16)` then `.to(device)`. Replace it with a new module-level function that:
   - Takes `model_id` and `arch` parameters
   - Dispatches to `arch.diffusion.get_module_by_name(arch).load_vae(model_id)` (implemented in zit.py by P904-A11)
   - Applies `.to(device)` to the result before returning
   - Remove the inline `from diffusers import AutoencoderKL` and `import torch` imports from `LoadVae.execute()` since they are no longer needed there (the arch module handles them)

4. **Extract `LoadClip.execute()`'s inline dispatch into `_load_clip_from_safetensors(model_id, clip_type)`**. Create a new module-level function that:
   - Takes `model_id` and `clip_type` parameters
   - Calls `arch_clip.get_module(clip_type)` (existing pattern, unchanged)
   - If `module is None`, raises `ValueError(f"unsupported clip_type: {clip_type!r}")` (existing pattern, unchanged)
   - Calls `module.load(model_id, torch_dtype=torch.bfloat16, device=self.ctx.device)` — but since this is a module-level function (not a method), it cannot capture `self.ctx.device`. The function must accept `device` as an explicit parameter: `_load_clip_from_safetensors(model_id, clip_type, device)`. This is the only non-trivial change: the caller passes `self.ctx.device` explicitly.
   - Returns the result directly

5. **Update `LoadModel.execute()`** to call `_load_model_from_safetensors(model_id, model_id, self.ctx.device)` by its new name (line 437). No other change to this method.

6. **Update `LoadVae.execute()`** to call `_load_vae_from_safetensors(model_id, arch)` instead of defining the inline closure. The `device` variable captured from `self.ctx.device` is passed through the new function's parameter. Update the `get_or_load` call to pass the named function. The `arch` value comes from the model's metadata detection — since the VAE loader doesn't currently detect arch, it must be derived from `model_id` the same way `_load_model_from_safetensors` does. However, looking at the current code, `LoadVae.execute()` does NOT detect architecture — it just calls `AutoencoderKL.from_single_file()` which infers the architecture from checkpoint keys. The new `_load_vae_from_safetensors` function needs an `arch` parameter. Since the VAE loading path doesn't have a `detected_arch` variable, we need to either: (a) pass a default arch string, or (b) have `load_vae()` accept a generic arch. Looking at zit.py's `load_vae()`, it constructs `AutoencoderKL(block_out_channels=[128, 256, 512, 512])` — this is ZiT-specific. The arch parameter here is for dispatch, not for the VAE's own config. The simplest approach: pass `"zit"` as the default arch since that's the only architecture supported right now (consistent with `MockModel(arch="zit")`).

7. **Update `LoadClip.execute()`** to call `_load_clip_from_safetensors(model_id, clip_type, self.ctx.device)` instead of inline dispatch. The function handles the arch_clip lookup and module.load() call internally.

8. **Update the test file** `worker/tests/test_nodes_loader.py`: rename `test_loadmodel_hf_directory_accepts_device_param` to `test_loadmodel_safetensors_accepts_device_param` and update its import from `_load_model_from_hf_directory` to `_load_model_from_safetensors`. This test checks the function signature via `inspect.signature` — the rename preserves the signature (still accepts `device` with default `"cpu"`), so the test assertions remain valid.

9. **Verify no other references** to the old function name `_load_model_from_hf_directory` exist in the codebase (grep confirms zero hits after rename).

## Public API Surface

No new public items. All changes are to private module-level functions (prefixed with `_`) and internal method call sites. The `__all__` list in `loader.py` is unchanged:
```python
__all__ = [
    "LoadModel",
    "LoadVae",
    "LoadClip",
    "RealClip",
    "MockModel",
    "MockVae",
    "MockClip",
    "MockTokenizer",
    "MockTextEncoder",
]
```

New private functions introduced:
- `_load_model_from_safetensors(model_id: str, arch: str, device: str = "cpu") -> RealModel` (renamed from `_load_model_from_hf_directory`)
- `_load_vae_from_safetensors(model_id: str, arch: str, device: str) -> Any` (extracted from inline closure)
- `_load_clip_from_safetensors(model_id: str, clip_type: str, device: str) -> Any` (extracted from inline dispatch)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Rename `_load_model_from_hf_directory` → `_load_model_from_safetensors`; replace direct diffusers calls with arch dispatch; extract inline loader_fn → `_load_vae_from_safetensors`; extract inline dispatch → `_load_clip_from_safetensors`; update execute() call sites |
| MODIFY | `worker/tests/test_nodes_loader.py` | Rename test referencing old function name to use `_load_model_from_safetensors` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `test_nodes_loader.py` | `test_loadmodel_safetensors_accepts_device_param` | Renamed function `_load_model_from_safetensors` exists and accepts `device` param with default `"cpu"` | `ANVILML_WORKER_MOCK=1`, `torch` installed (skip if absent) | `inspect.signature(_load_model_from_safetensors)` | `"device"` parameter exists with default `"cpu"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_safetensors_accepts_device_param -v` exits 0 |
| `test_nodes_loader.py` | `test_loadmodel_execute_returns_mock_model` | LoadModel mock path unchanged — returns `MockModel(arch="zit")` | `ANVILML_WORKER_MOCK=1` | `LoadModel.execute(model_id="test-model")` | `result["model"]` is `MockModel` with `arch == "zit"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model -v` exits 0 |
| `test_nodes_loader.py` | `test_loadvae_execute_returns_mock_vae` | LoadVae mock path unchanged — returns `MockVae` | `ANVILML_WORKER_MOCK=1` | `LoadVae.execute(model_id="test-vae")` | `result["vae"]` is `MockVae` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae -v` exits 0 |
| `test_nodes_loader.py` | `test_loadclip_execute_returns_mock_clip_default_type` | LoadClip mock path unchanged — returns `MockClip(clip_type="qwen3")` | `ANVILML_WORKER_MOCK=1` | `LoadClip.execute(model_id="test-model")` | `result["clip"]` is `MockClip` with `clip_type == "qwen3"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type -v` exits 0 |

## CI Impact

No CI changes required. This task modifies only Python source files and their existing test file. The mock-mode test suite (`ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v`) already covers all three loader nodes' mock paths, which are unchanged by this refactor. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. This task is platform-neutral — it replaces Python function calls within the same execution path. The `arch.diffusion.get_module_by_name()` dispatch and `load_transformer()`/`load_vae()` calls are pure Python with no `#[cfg(unix)]`/`#[cfg(windows)]` guards, no path-separator handling, and no line-ending concerns. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| `arch.diffusion.get_module_by_name(detected_arch)` returns `None` if the detected architecture string does not match any registered module's `can_handle()` — this would raise `AttributeError` on `.load_transformer(model_id)` instead of a clear error. The current `from_single_file()` path would have raised its own error from diffusers. | Low | Medium | Add a guard after the dispatch: if `get_module_by_name()` returns `None`, raise `ValueError(f"unsupported architecture: {detected_arch!r}")` with the same ValueError pattern used by LoadClip for unsupported clip_type. This is a non-obvious branch — add an inline comment explaining it. |
| `load_vae()` in zit.py constructs `AutoencoderKL(block_out_channels=[128, 256, 512, 512])` with ZiT-specific VAE config. If a VAE checkpoint from a different architecture is loaded, the model construction would fail at `load_state_dict()` because the tensor shapes wouldn't match. The old `from_single_file()` path would also fail (diffusers infers architecture from checkpoint keys), but with a potentially different error message. | Low | Low | This is a pre-existing constraint — only ZiT VAEs are supported. The arch dispatch doesn't change this; it merely routes to the same zit.py code. Document in the new function's docstring that only ZiT-compatible VAE checkpoints are supported. |
| Removing `from diffusers import AutoencoderKL` and `import torch` from `LoadVae.execute()` could cause a NameError if any other code in the method references these names. Current inspection shows they are only used within the inline closure being extracted, so no residual references remain. | Very Low | Low | After extracting the closure, verify with grep that no residual `AutoencoderKL` or `torch` references exist in `LoadVae.execute()`. The py_compile step (ENVIRONMENT.md §7) will catch any such defect. |
| `_load_clip_from_safetensors` is a module-level function (not a method) so it cannot capture `self.ctx.device` from the caller's context. Passing `device` as an explicit parameter is the correct fix, but the test for LoadClip mock mode must continue to work since mock mode returns before reaching this function. | Very Low | Low | Mock mode returns early (line 616 in current code) before reaching the real-mode dispatch. The renamed test `test_loadclip_execute_returns_mock_clip_*` already verifies mock-mode behavior and will pass without any real-mode code path being exercised. |

## Acceptance Criteria

- [ ] `grep -n "_load_model_from_hf_directory" worker/nodes/loader.py` returns zero matches (old name fully retired)
- [ ] `grep -n "_load_model_from_safetensors" worker/nodes/loader.py` returns at least one match (new name present)
- [ ] `grep -n "_load_vae_from_safetensors" worker/nodes/loader.py` returns at least one match (VAE wrapper present)
- [ ] `grep -n "_load_clip_from_safetensors" worker/nodes/loader.py` returns at least one match (CLIP wrapper present)
- [ ] `grep -n "ZImageTransformer2DModel.from_single_file" worker/nodes/loader.py` returns zero matches (direct diffusers call removed)
- [ ] `grep -n "AutoencoderKL.from_single_file" worker/nodes/loader.py` returns zero matches (direct diffusers call removed)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 (all mock-mode tests pass)
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0 (syntax check passes)
