# Plan Report: P18-D14

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-D14                                           |
| Phase       | 018 — ZiT Generic Nodes                           |
| Description | worker/nodes/loader.py: LoadVae single-file path via from_single_file(), fixes ctx bug |
| Depends on  | P18-D13                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-23T09:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace `LoadVae.execute()`'s real-mode loading path from `AutoencoderKL.from_pretrained(model_id, subfolder="vae")` to `AutoencoderKL.from_single_file(model_id, torch_dtype=torch.bfloat16)`, moving the original directory-based code into an unreachable `_load_from_hf_directory(model_id)` helper. This makes `LoadVae` consistent with `LoadModel` (P18-D13) which already uses `from_single_file()`. Also fix the pre-existing `ctx` bug where `self.ctx.pipeline_cache` was referenced as bare `ctx.pipeline_cache` (a `NameError` at runtime).

## Scope

### In Scope
- Modify `LoadVae.execute()` in `worker/nodes/loader.py`: replace `AutoencoderKL.from_pretrained(model_id, subfolder="vae", torch_dtype=torch.bfloat16)` with `AutoencoderKL.from_single_file(model_id, torch_dtype=torch.bfloat16)` inside the `loader_fn` closure.
- Create `_load_from_hf_directory(model_id: str) -> Any` function in `worker/nodes/loader.py` containing the original `from_pretrained` code (moved unchanged from the current `loader_fn` body), kept but never called — same design as `_load_model_from_hf_directory` and `_load_clip_from_hf_directory`.
- Fix the pre-existing `ctx` bug: change bare `ctx.pipeline_cache` to `self.ctx.pipeline_cache` in `LoadVae.execute()` if present (verified at ACT time; current source shows `self.ctx.pipeline_cache` which may already be correct from a prior fix).
- No new tests added — the existing mock tests in `worker/tests/test_nodes_loader.py` continue to pass unchanged (mock mode never touches the real loading path).

### Out of Scope
None. This task's `defers_to` is `[]` (empty) — no scope is deferred. All functionality described in the task context is implemented in full.

## Existing Codebase Assessment

The `LoadModel` node was already migrated to `from_single_file()` in P18-D13, establishing the pattern this task follows: the active loading path uses `ZImageTransformer2DModel.from_single_file()` and the old directory-based code lives in `_load_model_from_hf_directory()`. The `LoadClip` node was migrated to the `arch.clip.get_module()` dispatcher in P18-D12, with old directory code preserved in `_load_clip_from_hf_directory()`.

The current `LoadVae` implementation (lines 339–356 of `loader.py`) still uses `AutoencoderKL.from_pretrained(model_id, subfolder="vae", torch_dtype=torch.bfloat16)` — this is the only loader node that has not yet been migrated to `from_single_file()`. The `ctx.pipeline_cache` access in `LoadVae.execute()` currently reads `self.ctx.pipeline_cache.get_or_load(...)` (line 353), which is already correct — the bare `ctx.pipeline_cache` bug may have been fixed in P18-D12 or P18-D13, or it may exist in a code path not yet visible. The plan accounts for both cases.

Established patterns to follow:
- Lazy imports inside the real-mode code path (not top-level).
- The `loader_fn` closure pattern for `pipeline_cache.get_or_load()`.
- The `_load_from_hf_directory()` helper naming convention and "kept but never called" design.
- Mock mode returns `MockVae()` sentinel — unchanged.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0          | pypi-query MCP | n/a                    |

`AutoencoderKL.from_single_file()` is confirmed available via `FromOriginalModelMixin` in diffusers 0.38.0. The method signature is `from_single_file(pretrained_model_link_or_path_or_dict=None, **kwargs)` with `torch_dtype` supported as a kwarg. `AutoencoderKL` is registered in `SINGLE_FILE_LOADABLE_CLASSES` at key `"AutoencoderKL"` (confirmed in `single_file_model.py` line 92). No feature flags required.

## Approach

1. **Read `worker/nodes/loader.py`** to identify the exact current state of `LoadVae.execute()` (lines 290–356) and verify whether the bare `ctx.pipeline_cache` bug exists. The task says it's a pre-existing `NameError` from P18-D5, but the current source shows `self.ctx.pipeline_cache.get_or_load(...)` on line 353 — confirm at ACT time whether the bug is already fixed or if it exists elsewhere (e.g., in a different branch or the old `loader_fn` body).

2. **Replace the `loader_fn` closure body** in `LoadVae.execute()`: change from:
   ```python
   def loader_fn() -> AutoencoderKL:
       return AutoencoderKL.from_pretrained(
           model_id,
           subfolder="vae",
           torch_dtype=torch.bfloat16,
       )
   ```
   to:
   ```python
   def loader_fn() -> AutoencoderKL:
       return AutoencoderKL.from_single_file(
           model_id,
           torch_dtype=torch.bfloat16,
       )
   ```
   Rationale: `from_single_file()` infers the VAE architecture from checkpoint tensor keys automatically (registered in `SINGLE_FILE_LOADABLE_CLASSES`), so no `config.json` or `original_config` kwarg is needed. The `subfolder="vae"` argument is not applicable for single-file loading — it was only needed for directory-based `from_pretrained`.

3. **Create `_load_from_hf_directory(model_id: str) -> Any`** function at module level (after `LoadClip` class, before `_load_model_from_hf_directory`), containing the original `from_pretrained` code moved unchanged:
   ```python
   def _load_from_hf_directory(model_id: str) -> Any:
       """(Deprecated) Load a VAE from an HF-style directory.

       This function preserves the original from_pretrained-based loading
       path that was replaced by ``from_single_file()`` in P18-D14.
       It is kept but never called — it may be reactivated in a future
       task if HF-directory loading is needed again.

       Args:
           model_id: Path to the VAE model directory.

       Returns:
           An ``AutoencoderKL`` instance loaded from the directory.
       """
       from diffusers import AutoencoderKL
       import torch

       return AutoencoderKL.from_pretrained(
           model_id,
           subfolder="vae",
           torch_dtype=torch.bfloat16,
       )
   ```
   Rationale: Mirrors the existing `_load_model_from_hf_directory` and `_load_clip_from_hf_directory` pattern — the old code is preserved for future reactivation but never invoked.

4. **Fix the `ctx` bug** if present: search for bare `ctx.pipeline_cache` (without `self.`) in `LoadVae.execute()`. If found, change to `self.ctx.pipeline_cache`. If already `self.ctx.pipeline_cache`, no change needed. Document whatever is found.

5. **Run Python syntax check**: `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` — must exit 0 before running tests.

6. **Run existing tests**: `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` — must exit 0 with all existing tests passing. Mock mode ensures the real loading path is never exercised.

## Public API Surface

No new public items. The `_load_from_hf_directory()` function is module-private (underscore-prefixed) and not exported in `__all__`. No changes to any class signatures or public method signatures.

| Action | Item | Module Path |
|--------|------|-------------|
| MODIFY | `LoadVae.execute()` loader_fn body | `worker.nodes.loader.LoadVae.execute` |
| CREATE | `_load_from_hf_directory(model_id: str) -> Any` | `worker.nodes.loader._load_from_hf_directory` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Replace `from_pretrained` with `from_single_file` in `LoadVae.execute()`; create `_load_from_hf_directory()` helper; fix bare `ctx.pipeline_cache` bug if present |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadvae_registered_in_registry` | LoadVae is registered in NODE_REGISTRY | NODE_REGISTRY cleared by fixture | None | "LoadVae" in registry | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadvae_execute_returns_mock_vae` | execute() returns MockVae in mock mode | ANVILML_WORKER_MOCK=1 set by conftest | `model_id="test-vae"` | `result["vae"]` is `MockVae` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadvae_metadata_attributes` | All six metadata attributes correct | None | None | NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS all correct | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes -v` exits 0 |

No new tests are added because the real loading path is never exercised in mock mode (the mock code path returns before reaching any imports of `diffusers`). The existing three LoadVae tests verify registry registration, mock-mode execution, and metadata attributes — all unaffected by the real-path change.

## CI Impact

No CI changes required. The modified file (`worker/nodes/loader.py`) is already covered by the existing `worker-linux` and `worker-windows` CI jobs which run `py_compile` and `pytest` against all test files. The `py_compile` step will catch any syntax errors introduced by the change. No new test files, CI gates, or configuration changes are needed.

## Platform Considerations

None identified. The change is a Python-level API substitution (`from_pretrained` → `from_single_file`) that is platform-neutral. Both methods accept a filesystem path string as the first argument and work identically on Linux and Windows. The `torch_dtype=torch.bfloat16` argument is also platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `AutoencoderKL.from_single_file()` may have different kwarg names in the installed diffusers version than `torch_dtype` — e.g., the method might use `weight_dtype` or another name. | Low | High | The MCP lookup confirms `torch_dtype` is a documented parameter in diffusers 0.38.0's `FromOriginalModelMixin.from_single_file()` (verified via source at `single_file_model.py` line 237). At ACT time, re-confirm the exact kwarg name against the installed version. |
| The bare `ctx.pipeline_cache` bug may not exist in the current codebase (it may have been fixed in P18-D12 or P18-D13). The plan assumes it exists but does not force a change if it doesn't. | Medium | Low | Read the file at ACT time to verify. If `self.ctx.pipeline_cache` is already correct, skip the fix. Document whatever is found. |
| `from_single_file()` may fail on VAE safetensors that lack the expected tensor keys for VAE architecture detection. | Low | High | `AutoencoderKL` is registered in `SINGLE_FILE_LOADABLE_CLASSES` with `convert_ldm_vae_checkpoint` and `create_vae_diffusers_config_from_ldm` mapping functions (confirmed in `single_file_model.py` lines 92-95). These functions infer architecture from checkpoint keys. If the ZiT FP8 VAE checkpoint has incompatible keys, the error will surface at runtime during integration testing. |
| Moving the old `loader_fn` body into `_load_from_hf_directory()` could introduce a duplicate import of `diffusers` and `torch` if the old code is not properly isolated. | Low | Low | The helper function already contains its own lazy imports (`from diffusers import AutoencoderKL; import torch`), matching the pattern used in `_load_model_from_hf_directory`. No change needed. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0
- [ ] `grep -n "from_single_file" worker/nodes/loader.py` returns at least one match in `LoadVae.execute()`'s `loader_fn`
- [ ] `grep -n "_load_from_hf_directory" worker/nodes/loader.py` returns a function definition (not just a comment)
- [ ] `grep -n "from_pretrained" worker/nodes/loader.py | grep -v "_load_from_hf_directory" | grep -v "_load_model_from_hf_directory" | grep -v "_load_clip_from_hf_directory"` returns no matches (no `from_pretrained` outside the preserved helpers)
- [ ] `worker/.venv/bin/python -c "import ast; ast.parse(open('worker/nodes/loader.py').read())"` exits 0 (Python AST parse confirms valid syntax)
