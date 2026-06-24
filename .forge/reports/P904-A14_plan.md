# Plan Report: P904-A14

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A14                                    |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/loader.py: LoadVae missing device arg (TypeError on first real call); LoadClip stale docstring |
| Depends on  | P904-A13                                    |
| Project     | anvilml                                     |
| Planned at  | 2026-06-24T13:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Fix two defects in the already-committed P904-A9–A13 implementation: (1) `LoadVae.execute()` calls `_load_vae_from_safetensors(model_id, "zit")` with only 2 of the function's 3 required positional arguments — `device` is missing entirely, causing a `TypeError` on first real invocation; (2) `LoadClip.execute()`'s docstring still claims the real path is stubbed until P18-D1, which is stale since the real dispatch via `_load_clip_from_safetensors` landed in P904-A12. Both are one-line fixes in `worker/nodes/loader.py`.

## Scope

### In Scope
- `worker/nodes/loader.py`: Fix `LoadVae.execute()` line 513 — change `_load_vae_from_safetensors(model_id, "zit")` to `_load_vae_from_safetensors(model_id, "zit", self.ctx.device)` so the VAE is placed on the worker's assigned device.
- `worker/nodes/loader.py`: Update `LoadClip.execute()`'s `Raises:` docstring section (lines 564–566) to describe actual behavior — no `NotImplementedError` is raised in non-mock mode; real loading proceeds via `_load_clip_from_safetensors`.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferred scope. Both defects are fully in scope and will be resolved in this task.

## Existing Codebase Assessment

The file `worker/nodes/loader.py` (773 lines) defines three loader nodes (`LoadModel`, `LoadVae`, `LoadClip`) plus helper functions `_load_model_from_safetensors`, `_load_vae_from_safetensors`, and `_load_clip_from_safetensors`. The established pattern is:

- Each node's `execute()` method checks `ANVILML_WORKER_MOCK == "1"` first, returning a mock sentinel. The real-mode branch calls a module-level `_load_*_from_safetensors` helper.
- All three helpers accept a `device` parameter and move the loaded model to that device via `.to(device)` before returning.
- `LoadModel.execute()` correctly passes `self.ctx.device` as the third argument to `_load_model_from_safetensors(model_id, model_id, self.ctx.device)` (line 439).
- `LoadClip.execute()` correctly passes `self.ctx.device` to `_load_clip_from_safetensors(model_id, clip_type, self.ctx.device)` (lines 594–596).
- `LoadVae.execute()` is the outlier: it calls `_load_vae_from_safetensors(model_id, "zit")` with only two arguments, omitting `device`. The function signature `def _load_vae_from_safetensors(model_id: str, arch: str, device: str)` has no default on `device`, making this a guaranteed `TypeError` at runtime.

The test file `worker/tests/test_nodes_loader.py` (492 lines) tests all three nodes exclusively in mock mode. No test exercises the real-mode code path (which requires torch/diffusers/safetensors). This is consistent with the phase's design: Group Z will provide real-mode CPU tests later.

## Resolved Dependencies

None. This task touches no external packages, crates, or libraries. It only modifies a Python source file's function call and docstring.

## Approach

1. **Fix `LoadVae.execute()` call site (line 513).** Change:
   ```python
   result = self.ctx.pipeline_cache.get_or_load(
       model_id, "bf16", lambda: _load_vae_from_safetensors(model_id, "zit")
   )
   ```
   to:
   ```python
   result = self.ctx.pipeline_cache.get_or_load(
       model_id, "bf16", lambda: _load_vae_from_safetensors(model_id, "zit", self.ctx.device)
   )
   ```
   Rationale: `self.ctx.device` is the worker's assigned device string (e.g. `"cuda:0"`). This matches the exact pattern used by `LoadModel.execute()` (line 439) and `LoadClip.execute()` (line 595). Without this, `_load_vae_from_safetensors` raises `TypeError: missing 1 required positional argument: 'device'`.

2. **Update `LoadClip.execute()` docstring.** Replace the stale `Raises:` section:
   ```
   Raises:
       NotImplementedError: If called in non-mock mode. The real
           safetensors loading path is stubbed until P18-D1.
   ```
   with accurate behavior:
   ```
   Raises:
       OSError: If the model file or directory does not exist.
       ValueError: If no architecture module claims the specified
           clip_type (e.g. unknown tokeniser type).
   ```
   Rationale: The real dispatch via `_load_clip_from_safetensors` (landed in P904-A12) calls `module.load()` which raises `OSError` for missing files and `ValueError` for unsupported clip types — matching the other loader nodes' docstrings.

3. **Verify correctness.** Run the acceptance criteria commands to confirm both fixes are applied.

## Public API Surface

No public API surface changes. Both modifications are internal:
- `LoadVae.execute()`'s signature is unchanged; only its internal call site is fixed.
- `LoadClip.execute()`'s docstring is updated but the function signature and behavior are unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Fix `LoadVae.execute()` call to include `self.ctx.device` argument; update `LoadClip.execute()` docstring |

## Tests

No new tests are added. The existing mock-mode test suite (`worker/tests/test_nodes_loader.py`) is unaffected — all tests exercise the `ANVILML_WORKER_MOCK=1` early-return path which does not call `_load_vae_from_safetensors`. The acceptance criteria are grep-based verification commands that confirm the code changes are present.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_nodes_loader.py` (existing) | All existing tests | Mock-mode execution of LoadModel, LoadVae, LoadClip still works after the call-site fix | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 |

## CI Impact

No CI changes required. The fix is in a Python source file; existing CI already runs `py_compile` on all `worker/*.py` files (ENVIRONMENT.md §7) and `pytest worker/tests/` in mock mode (ENVIRONMENT.md §8). Both will continue to pass.

## Platform Considerations

None identified. The `self.ctx.device` string is passed through to `vae.to(device)`, which is a `diffusers`/`torch` method that handles device placement identically across platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `self.ctx.device` value might be `None` or empty at this call site, causing `vae.to(None)` to fail. | Low | High | Verify that `NodeContext.device` is always set by the worker spawn path — it is set from `ANVILML_DEVICE_INDEX` at worker startup (ENVIRONMENT.md §3.4), so it is never None or empty at runtime. |
| The docstring change might be rejected if the `Raises:` section should match `_load_clip_from_safetensors`'s exact error types rather than general ones. | Low | Low | `_load_clip_from_safetensors` raises `OSError` (from `module.load()`'s file-not-found path) and `ValueError` (from unsupported clip_type). This matches the documented exceptions from other loader functions. |

## Acceptance Criteria

- [ ] `grep -n '_load_vae_from_safetensors(model_id, "zit", self.ctx.device)' worker/nodes/loader.py` returns exactly one match (the fixed call site)
- [ ] `grep -n 'stubbed until P18-D1' worker/nodes/loader.py` returns zero matches (stale docstring removed)
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 (existing tests still pass)
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0 (syntax check passes)
