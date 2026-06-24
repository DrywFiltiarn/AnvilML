# Plan Report: P904-A10

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-A10                                          |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/arch/diffusion/zit.py: add load_transformer() — offline transformer loading, no HF network access |
| Depends on  | P904-A9                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-24T12:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add `load_transformer(model_id: str) -> ZImageTransformer2DModel` to `worker/nodes/arch/diffusion/zit.py`, an offline transformer-loading function that reads a raw `.safetensors` checkpoint, remaps its keys via `diffusers`' own internal conversion helper, and loads the weights into a freshly-constructed `ZImageTransformer2DModel`. Zero network calls — this is the actual fix for the confirmed HuggingFace Hub `config.json` download defect that occurs when `ZImageTransformer2DModel.from_single_file()` falls through to `fetch_diffusers_config()`.

## Scope

### In Scope
- Add `load_transformer(model_id: str) -> ZImageTransformer2DModel` function to `worker/nodes/arch/diffusion/zit.py`
- Append `"load_transformer"` to the module's `__all__` list
- Implement the function body: construct `ZImageTransformer2DModel()` with zero args, load checkpoint via `safetensors.torch.load_file(model_id)`, remap keys via `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers(checkpoint)`, call `model.load_state_dict(remapped)`, return the model
- Add Google-style docstring with Args/Returns sections
- Add a unit test in `worker/tests/test_arch_zit.py` that verifies `load_transformer` is importable and callable in mock mode (mock-mode tests cannot exercise the real loading path without torch, but must confirm the symbol exists)
- Add `load_transformer` to the test file's import list

### Out of Scope
None. This task has an empty `defers_to` field (`[]` from JSON). All described functionality is implemented fully — no stubs, no `NotImplementedError`, no deferred scope.

## Existing Codebase Assessment

The existing `worker/nodes/arch/diffusion/zit.py` (358 lines) follows a well-established pattern: module-level constants (`VAE_SCALE_FACTOR`), private sentinel classes (`_SamplingCancelled`, `MockLatent`), and public functions (`can_handle`, `compute_latent_shape`, `sample`, `_make_callback`). All real-mode heavy dependencies (`torch`, `diffusers`, `safetensors`) are imported lazily inside the `if _mock:` guard in `sample()`. The module's `__all__` exports five symbols.

The existing `worker/nodes/arch/clip/{qwen3,clip_l,t5}.py` files already implement the exact loading pattern this task will replicate for the transformer: they import `load_file as safetensors_load_file` from `safetensors.torch`, call `model.load_state_dict(safetensors_load_file(model_id))`, and use lazy imports inside real-mode branches. This is the established convention for this project.

The existing `worker/nodes/loader.py`'s `_load_model_from_hf_directory` (line 645–723) uses `ZImageTransformer2DModel.from_single_file(model_id, torch_dtype=torch.float16)` — the exact code path that triggers the HF network bug. P904-A9 (prerequisite) will delete dead code but leave this function for P904-A12 to rewrite; this task's `load_transformer()` will be the replacement called by that future rewrite.

The existing `worker/tests/test_arch_zit.py` (601 lines) follows a consistent style: mock-mode tests use `ANVILML_WORKER_MOCK=1` (via conftest.py autouse fixture), import all tested symbols at the top, and use helper functions like `_make_model()` for building test fixtures. Real-mode tests that need to override mock mode capture `os.environ.get("ANVILML_WORKER_MOCK")`, set it to `"0"`, and restore in a `finally` block.

There is no gap between the design doc and current source that affects this approach. The design doc (`ANVILML_DESIGN.md §10.5`) specifies the exact loading pattern (construct model with defaults, load safetensors, remap keys, `load_state_dict`) — this task implements it verbatim.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0          | pypi-query MCP | n/a                    |
| python | safetensors | 0.8.0        | pypi-query MCP | n/a                    |

diffusers 0.38.0 is confirmed as the latest published version and matches the project's `base.txt` requirement (`>=0.38.0`). safetensors 0.8.0 is confirmed as the latest published version and matches `base.txt` (`>=0.8`).

API shape confirmed via MCP:
- `ZImageTransformer2DModel` exists in diffusers 0.38.0 as a class that can be constructed with zero arguments (all architecture-specific defaults are registered via `@register_to_config`)
- `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers` exists as a function taking a checkpoint dict and returning a remapped dict — this is a private module path, not re-exported from `diffusers.loaders.__init__`
- `safetensors.torch.load_file` exists as the standard API for loading a safetensors file into a dict of tensors

Note: `convert_z_image_transformer_checkpoint_to_diffusers` is a private internal helper in diffusers. Its API may change across version bumps without deprecation warning. This risk is documented in the Risks section.

## Approach

1. **Append `"load_transformer"` to `__all__`** in `worker/nodes/arch/diffusion/zit.py`. The existing `__all__` is a module-level list literal on lines 32–38. Add `"load_transformer"` as the last element. This makes the function discoverable by `dir()` and explicit imports.

2. **Implement `load_transformer(model_id: str) -> Any`** at module level in `zit.py`, after the existing `can_handle()` function (line 209) and before `sample()` (line 212). The function signature uses `-> Any` as the return type hint (matching the design doc's `Any` placeholder) — the ACT agent may refine to the concrete `ZImageTransformer2DModel` type if it's importable at module level without breaking mock-mode isolation.

   The function body:
   - Check mock mode via `os.environ.get("ANVILML_WORKER_MOCK") == "1"` (same pattern as `sample()`, line 275). In mock mode, return `None` (no real model can be constructed without torch).
   - In real mode:
     a. Lazy-import `ZImageTransformer2DModel` from `diffusers` — same import style as `sample()`'s real-mode branch (line 287–288), which already imports `ZImagePipeline` and `FlowMatchEulerDiscreteScheduler` lazily.
     b. Lazy-import `load_file as safetensors_load_file` from `safetensors.torch` — same import style as the existing CLIP arch modules (`qwen3.py` line 102, `clip_l.py` line 102, `t5.py` line 102).
     c. Lazy-import `convert_z_image_transformer_checkpoint_to_diffusers` from `diffusers.loaders.single_file_utils` — private import, same as the design doc specifies.
     d. Construct the model: `model = ZImageTransformer2DModel()` — zero arguments, relying on the class's registered defaults (`dim=3840, n_layers=30, n_heads=30, cap_feat_dim=2560`).
     e. Load the checkpoint: `checkpoint = safetensors_load_file(model_id)`.
     f. Remap the keys: `remapped = convert_z_image_transformer_checkpoint_to_diffusers(checkpoint)`.
     g. Load weights: `model.load_state_dict(remapped)`.
     h. Return `model`.

3. **Add Google-style docstring** to `load_transformer()` describing:
   - What it does: loads a ZiT diffusion transformer from a raw `.safetensors` file without network access
   - Why zero-arg construction: defaults match the published 6B ZiT architecture config
   - How key remapping works: reuses diffusers' internal conversion function
   - Args section: `model_id` — path to a `.safetensors` file
   - Returns section: the loaded `ZImageTransformer2DModel` instance
   - Raises section: `OSError` if the file does not exist, `ValueError` if the checkpoint is malformed

4. **Add test in `worker/tests/test_arch_zit.py`**: Add `load_transformer` to the existing import list (line 20–28). Add a new test function `test_load_transformer_is_callable` that verifies `load_transformer` is a callable in mock mode. This test follows the same pattern as `test_can_handle_zit()` — it runs in mock mode, requires no torch, and confirms the symbol exists. The test does NOT exercise the real loading path (that requires P904-B1b's raw-format fixtures, which is a separate task).

5. **Update `docs/TESTS.md`** to add a catalogue entry for the new test, per `ENVIRONMENT.md §11.4`/`§5.10`'s test catalogue sync obligation.

6. **Verify**: Run `python3 -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion.zit import load_transformer; assert callable(load_transformer)"` — this is the acceptance criterion from the task context.

## Public API Surface

New public item:

```python
# Module: worker.nodes.arch.diffusion.zit
def load_transformer(model_id: str) -> Any:
    """Load the Z-Image Turbo diffusion transformer from a raw .safetensors file.

    Constructs a ZImageTransformer2DModel() with zero arguments (defaults match
    the published 6B ZiT config), loads the raw checkpoint via safetensors,
    remaps keys via diffusers' internal conversion function, and returns the
    loaded model. Zero network calls.

    Args:
        model_id: Path to a .safetensors file containing the transformer
            weights in the original (pre-remap) key format.

    Returns:
        A loaded ZImageTransformer2DModel instance with weights loaded.

    Raises:
        OSError: If the model file does not exist.
        ValueError: If the safetensors file is malformed or missing
            required keys.
    """
    ...
```

`load_transformer` is appended to `__all__` in `worker/nodes/arch/diffusion/zit.py`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `"load_transformer"` to `__all__`; implement `load_transformer()` function |
| MODIFY | `worker/tests/test_arch_zit.py` | Add `load_transformer` to import list; add `test_load_transformer_is_callable` test |
| MODIFY | `docs/TESTS.md` | Add catalogue entry for the new test |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_zit.py` | `test_load_transformer_is_callable` | `load_transformer` is importable and callable in mock mode (no torch import at module load time) | `python3 -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion.zit import load_transformer; assert callable(load_transformer)"` exits 0 |
| `worker/tests/test_arch_zit.py` | (existing tests) | No regressions — all existing mock-mode tests still pass | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 |

## CI Impact

No CI changes required. This task modifies only Python source files and a test file within the existing `worker/tests/` directory. The existing CI worker job (`ANVILML_WORKER_MOCK=1 <python> -m pytest worker/tests/ -v`) will automatically pick up the new test file changes. No new CI gates, markers, or configuration files are introduced.

## Platform Considerations

None identified. The function operates on file paths and tensor operations — `safetensors.torch.load_file` and `model.load_state_dict()` are platform-neutral. The `ZImageTransformer2DModel()` constructor uses only Python-side defaults. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `diffusers.loaders.single_file_utils.convert_z_image_transformer_checkpoint_to_diffusers` is a private internal API — it is not re-exported from `diffusers.loaders.__init__` and may be renamed, moved, or have its signature changed in a future `diffusers` version bump without deprecation warning. | Medium | High | The function is documented in `ANVILML_DESIGN.md §10.5` and the task context; P904-B1b (a later task) will exercise the remap path via its raw-format fixtures, so a signature change will be caught when B1b runs. If tight version pinning is needed, pin `diffusers` to `==0.38.0` rather than `>=0.38.0`. |
| Constructing `ZImageTransformer2DModel()` with zero args may not match the actual architecture of a given checkpoint — if a checkpoint was trained with different defaults (e.g., different `dim`, `n_layers`, or `cap_feat_dim`), `load_state_dict()` will raise a `RuntimeError` for shape mismatch. | Low | Medium | The task context confirms these defaults match the published 6B ZiT config. If future checkpoints use different configs, the function signature will need to accept optional override params. For now, zero-arg construction is correct per the design spec. |
| The lazy import of `diffusers.loaders.single_file_utils` may fail in environments where `diffusers` is installed but `single_file_utils` is not available (e.g., a minimal or corrupted install). | Low | Medium | The import is inside the real-mode branch (reachable only when `ANVILML_WORKER_MOCK != "1"`), so mock-mode tests are unaffected. A `ModuleNotFoundError` would surface as a clear runtime error during actual model loading, not a silent failure. |

## Acceptance Criteria

- [ ] `python3 -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion.zit import load_transformer; assert callable(load_transformer)"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (no regressions)
- [ ] `grep -n "load_transformer" worker/nodes/arch/diffusion/zit.py | head -3` — at least two matches: one in `__all__`, one as function definition
- [ ] `grep -n "test_load_transformer_is_callable" worker/tests/test_arch_zit.py` — at least one match (test function exists)
- [ ] `python3 -c "import ast; tree = ast.parse(open('worker/nodes/arch/diffusion/zit.py').read()); funcs = [n.name for n in ast.walk(tree) if isinstance(n, ast.FunctionDef)]; assert 'load_transformer' in funcs"` exits 0
- [ ] `grep -n "load_transformer" docs/TESTS.md` — at least one match (catalogue entry exists)
