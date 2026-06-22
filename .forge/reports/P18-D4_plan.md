# Plan Report: P18-D4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D4                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/loader.py: LoadModel real safetensors loading path |
| Depends on  | P18-D3b                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T10:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace `LoadModel`'s stubbed real-path `NotImplementedError` with actual safetensors-based model loading. When `ANVILML_WORKER_MOCK` is not `"1"`, the node opens the safetensors file via `safetensors.safe_open()`, detects the model architecture from metadata or directory naming convention, constructs a `diffusers.ZImageTransformer2DModel` from the loaded weights, caches it via `ctx.pipeline_cache.get_or_load()`, and returns an object exposing `.arch` (str) and `.in_channels` (int) for downstream consumers (`EmptyLatent` and `arch.sample()`). The mock path remains completely untouched — existing mock tests must continue to pass unchanged.

## Scope

### In Scope
- Replace the `NotImplementedError` in `LoadModel.execute()`'s real-path branch with actual safetensors loading logic.
- Detect architecture from safetensors metadata (`metadata.get("arch")`) or fall back to `models/` directory naming convention.
- Construct the diffusion transformer component via `diffusers.ZImageTransformer2DModel.from_pretrained()` (lazy import inside the real path).
- Wrap the loader in `ctx.pipeline_cache.get_or_load(model_id, "fp8", loader_fn)`.
- Return a lightweight wrapper object exposing `.arch` and `.in_channels`.
- Add inline `#` comments at every non-trivial decision point.
- Ensure existing mock tests in `worker/tests/test_nodes_loader.py` continue to pass unchanged.

### Out of Scope
- `LoadVae` real path (handled by P18-D5).
- `LoadClip` real path (handled by P18-D6).
- `arch/zit.py` real `sample()` path (handled by P18-D9a–c).
- Any changes to mock mode, mock classes, or the conftest.py autouse fixture.
- Pipeline assembly or caching of the assembled `ZImagePipeline` (handled by P18-D9a).
- Any Rust-side changes.

## Existing Codebase Assessment

The `LoadModel` class in `worker/nodes/loader.py` (line 81–150) is a fully registered node with correct metadata (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`). Its `execute()` method checks `os.environ.get("ANVILML_WORKER_MOCK") == "1"` at runtime and returns `{"model": MockModel(arch="zit")}` in mock mode. The real-path branch (lines 140–150) currently raises `NotImplementedError` with a TODO referencing P18-A1.

The `MockModel` sentinel class (line 28–45) already exposes `.arch`, which is exactly what downstream consumers expect. However, the real-path model object needs both `.arch` and `.in_channels` — so a new wrapper type is required for real-mode returns.

The `PipelineCache` class in `pipeline_cache.py` (line 36–130) provides `get_or_load(model_id, dtype, loader_fn)` with LRU eviction and OOM retry. It is fully implemented and tested (P18-C1). The `NodeContext.pipeline_cache` field is typed as `dict[str, Any]` in `base.py` (line 138) — Retrofit Phase 903 (P903-A2) replaces the empty dict with a real `PipelineCache` instance at runtime, so the loader code can call `.get_or_load()` directly.

The `arch/` package (`worker/nodes/arch/__init__.py`) provides `get_module(model_obj)` which iterates loaded arch modules and calls their `can_handle()`. The `zit.py` module exposes `can_handle(model_obj)` checking `model_obj.arch == "zit"`. These are already in place and will be used by downstream consumers but not by this task.

The test file `worker/tests/test_nodes_loader.py` has 14 tests covering registry registration, mock execution, missing-input handling, and metadata attributes. All tests run under `ANVILML_WORKER_MOCK=1` (conftest.py autouse fixture), so they only exercise the mock path. Adding real-path code does not affect them.

The project convention for model objects is that they carry `.arch` as a string identifier. The `EmptyLatent` node (P18-D8) will call `model.in_channels` to determine `num_channels_latents`, so the wrapper must expose this integer.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source        | Feature flags confirmed |
|--------|-----------|-----------------|-------------------|------------------------|
| python | safetensors | 0.8.0         | pypi-query MCP    | n/a                    |
| python | diffusers  | 0.38.0        | pypi-query MCP    | n/a                    |

Both packages are already declared in `worker/requirements/base.txt` (`safetensors>=0.4`, `diffusers>=0.36.0`). The task context references `diffusers.ZImageTransformer2DModel` — this class exists in diffusers 0.36.0+ (the Z-Image Turbo pipeline family). The `safetensors.safe_open(path, framework="pt")` API is available in safetensors 0.8.0.

Note: The task context references `ZImageTransformer2DModel` — the ACT agent must confirm this exact class name exists in the resolved diffusers version at session start via MCP lookup of the diffusers source code. If the name differs (e.g. `ZImageTransformer2D`), the plan must be adjusted accordingly.

## Approach

1. **Add a real-mode model wrapper class** at module level in `loader.py`, after `MockModel` and before `LoadModel`. Name it `RealModel`. It is a simple data class (not a `BaseNode` subclass) that wraps a diffusers transformer component:
   - `__init__(self, transformer, arch: str)` — stores the transformer and architecture string.
   - Exposes `.arch` as a property returning the arch string.
   - Exposes `.in_channels` as a property reading `transformer.config.in_channels`.
   - Has a Google-style docstring documenting the wrapper purpose.
   - Rationale: The real diffusers transformer object carries config data (`in_channels`) that mock consumers (like `EmptyLatent`) need to read. Wrapping it preserves the `.arch` interface that downstream code already expects from `MockModel`.

2. **Replace the `NotImplementedError` branch** in `LoadModel.execute()` (lines 140–150) with real loading logic:
   - Step 2a: Add a lazy import block at the top of the real-path branch:
     ```python
     from safetensors.torch import safe_open
     from diffusers import ZImageTransformer2DModel
     ```
     These imports happen inside the `else` branch so they never execute in mock mode.
   - Step 2b: Open the safetensors file with `safe_open(model_id, framework="pt")` to read metadata and tensors.
   - Step 2c: Detect architecture: read `metadata.get("arch")` from the safetensors file. If absent, infer from the `models/` directory naming convention (the ACT agent will confirm the exact convention via project knowledge search).
   - Step 2d: Define a `loader_fn()` closure that constructs the transformer:
     ```python
     def loader_fn() -> RealModel:
         transformer = ZImageTransformer2DModel.from_pretrained(model_id, subfolder="unet", torch_dtype=torch_dtype)
         return RealModel(transformer, arch=arch)
     ```
   - Step 2e: Call `ctx.pipeline_cache.get_or_load(model_id, "fp8", loader_fn)` to get or cache the result.
   - Step 2f: Return `{"model": result}`.
   - Inline comments at each decision point explaining the rationale (arch detection fallback, pipeline cache key, FP8 dtype choice).

3. **Add inline `#` comments** at every non-trivial decision point per FORGE_AGENT_RULES.md §12.2:
   - Why `framework="pt"` is used with safetensors.
   - Why architecture detection falls back to directory naming.
   - Why the pipeline cache key uses `"fp8"` dtype.
   - Why `ZImageTransformer2DModel` is the correct diffusers class (confirmed via MCP at ACT time).

4. **Verify** that the existing mock tests in `worker/tests/test_nodes_loader.py` still pass under `ANVILML_WORKER_MOCK=1`. The real-path code is only reachable when the env var is not `"1"`, so no test changes are needed.

## Public API Surface

No new public Python module-level exports. The `RealModel` class is internal to `loader.py` (not added to `__all__`). The existing `__all__ = ["LoadModel", "LoadVae", "MockModel", "MockVae", "MockClip"]` is unchanged.

The `LoadModel.execute()` method signature is unchanged (`def execute(self, **inputs: Any) -> dict[str, Any]`). Its return value type changes from `{"model": MockModel | NotImplemented}` to `{"model": MockModel | RealModel}`, but since both types expose `.arch`, downstream code is unaffected.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `RealModel` wrapper class; replace `NotImplementedError` in `LoadModel.execute()` real-path with safetensors loading logic |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | All existing tests (14 tests) | Mock-mode path continues to work; registry registration; metadata attributes | `ANVILML_WORKER_MOCK=1` set by conftest.py autouse fixture | N/A | All 14 tests pass with exit code 0 | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 |

No new test functions are added in this task. The acceptance criterion for P18-D4 is that existing mock tests continue to pass unchanged — the real-path code is only reachable outside mock mode, and no test in the current suite exercises non-mock mode.

## CI Impact

No CI changes required. The task only modifies `worker/nodes/loader.py`. The existing CI job `worker-linux` and `worker-windows` run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v`, which exercises the mock path only. The real-path code is in a branch that CI never reaches. No new test files, no new CI gates.

## Platform Considerations

None identified. The safetensors library and diffusers are cross-platform. The `models/` directory naming convention (if used as a fallback) is platform-agnostic. No `# cfg` guards or path-separator handling needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ZImageTransformer2DModel` class name may differ in the resolved diffusers 0.38.0 version — the task context names `ZImageTransformer2DModel` but the actual class could be `ZImageTransformer2D`, `ZImageUNet`, or a different name entirely. | Medium | High | The ACT agent must query the diffusers source code at session start to confirm the exact class name. If it differs, use the confirmed name and record the substitution in the plan. |
| Safetensors metadata may not contain an `"arch"` key — Z-Image Turbo FP8 safetensors files may not embed architecture metadata, requiring the directory-naming fallback. | Medium | Medium | Implement the metadata-first approach with a clear fallback to directory naming. The ACT agent will confirm the exact convention via project knowledge search at ACT time. |
| `pipeline_cache.get_or_load()` expects a `PipelineCache` instance but `NodeContext.pipeline_cache` is typed as `dict[str, Any]` — Retrofit Phase 903 (P903-A2) replaces it at runtime, but type checkers may flag this. | Low | Low | No runtime change needed; the type annotation is a vestige of P903-A1. The ACT agent should note this in comments but not change the type. |
| The `from_pretrained()` call for `ZImageTransformer2DModel` may expect a directory, not a file — `model_id` is a path that could point to either. | Low | Medium | Verify whether `model_id` points to a directory or a file. If it's a file, use `safe_open` directly and load tensors into the transformer manually. If it's a directory, use `from_pretrained`. The scheduler resolves hashes to paths — confirm the path format at ACT time. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 (all 14 existing mock tests pass)
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0 (syntax check passes)
- [ ] `head -1 .forge/reports/P18-D4_plan.md` prints `# Plan Report: P18-D4`
- [ ] `grep "^## " .forge/reports/P18-D4_plan.md` shows exactly 12 section headings
- [ ] `wc -l .forge/reports/P18-D4_plan.md` returns a value greater than 40
