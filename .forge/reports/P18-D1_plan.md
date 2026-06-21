# Plan Report: P18-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D1                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/zit.py: ZiT FP8 dispatch module |
| Depends on  | P18-A1, P18-A2, P18-A3, P18-B2, P18-C1      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-21T14:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the Z-Image Turbo (ZiT) FP8 architecture dispatch module and its parent `__init__.py` registry. This task produces two Python files (`worker/nodes/arch/__init__.py` and `worker/nodes/arch/zit.py`) plus a test file (`worker/tests/test_arch_zit.py`) that together provide `can_handle(model_obj) -> bool` for architecture detection and `sample(...)` for the real mockable sampling loop. The mock path returns `(MockLatent(), seed)` without importing torch or diffusers, enabling CI to verify the module loads cleanly under `ANVILML_WORKER_MOCK=1`.

## Scope

### In Scope
- `worker/nodes/arch/__init__.py`: Package init with auto-import of arch modules and `can_handle(model_obj)` dispatcher.
- `worker/nodes/arch/zit.py`: ZiT-specific `can_handle()` and `sample()` functions with mock path returning `(MockLatent(), seed)` and real path stubbed with `NotImplementedError`.
- `worker/tests/test_arch_zit.py`: Unit tests for `can_handle`, mock `sample`, and mock mode isolation (≥ 3 tests).
- Import guard: `torch`, `diffusers`, and `safetensors` must never be imported at module top level.

### Out of Scope
- Real-mode implementation of `sample()` (uses `diffusers.ZImagePipeline`) — stubbed with `NotImplementedError` for now, real path will be filled in by a future task once the pipeline cache integration is fully wired.
- Integration with the `Sampler` node's `execute()` method to call `arch.sample()` — that stub is already present in `sampler.py` and will be wired by the Sampler node task.
- FP8 dtype upcast logic — documented as inline comment but not implemented.

## Existing Codebase Assessment

The `worker/nodes/arch/` directory exists as a package (has `__pycache__/`) but contains no Python source files. The parent `worker/nodes/__init__.py` auto-imports all sibling modules via `pkgutil.iter_modules()`, so any `.py` file placed in `arch/` will be imported at worker startup.

The `PipelineCache` class is fully implemented in `worker/pipeline_cache.py` with tests. It uses `OrderedDict` for LRU eviction and handles `torch.cuda.OutOfMemoryError` by clearing all entries and retrying once. The cache key format is `f"{model_id}:{dtype}"`.

The `Sampler` node in `worker/nodes/sampler.py` already has a mock code path returning `(MockLatent(latent.width, latent.height, latent.batch_size), seed)` and a `NotImplementedError` stub for the real path. The `EMITS_PROGRESS = True` flag is set on the class.

The test conventions are clear: one test file per source module under `worker/tests/`, with `conftest.py` providing an autouse `mock_mode` fixture that sets `ANVILML_WORKER_MOCK=1`. Tests use `importlib.reload()` to clear `NODE_REGISTRY` state. Docstrings follow Google style. The `MockLatent` class carries `width`, `height`, `batch_size` attributes.

The design doc (`ANVILML_DESIGN.md §10.4`) specifies the arch module interface: `can_handle(model_obj) -> bool` and `sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[Any, int]`.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| python | diffusers | 0.38.0 (latest) | pypi-query MCP | n/a                    |
| python | diffusers | 0.36.0 (required) | pypi-query MCP | n/a — ZImagePipeline present from 0.36.0 onward per project docs |

Note: The MCP query confirmed diffusers 0.38.0 is the latest version on PyPI. The project's `worker/requirements/base.txt` pins `diffusers>=0.36.0`, which is the version that introduced `ZImagePipeline` per the task context. The class name `ZImagePipeline` (not `ZitPipeline`) was confirmed by the design doc and task context.

## Approach

1. **Create `worker/nodes/arch/__init__.py`** — Package init module that:
   - Provides `can_handle(model_obj) -> bool` which iterates through all loaded arch modules' `can_handle` functions and returns `True` if any match.
   - Auto-imports sibling `.py` modules in the `arch/` directory using `pkgutil.iter_modules(__path__)` (same pattern as `worker/nodes/__init__.py`).
   - Exports `can_handle` in `__all__`.
   - Includes Google-style docstring at module level.

2. **Create `worker/nodes/arch/zit.py`** — ZiT-specific dispatch module:
   - Define `MockLatent` class (a lightweight sentinel identical to `worker.nodes.sampler.MockLatent` but scoped locally to this module, since arch modules should not import from `sampler.py` — they are architecture-specific and must remain independent).
   - Implement `can_handle(model_obj) -> bool`: returns `model_obj.arch == "zit"` if the model object has an `arch` attribute; returns `False` otherwise.
   - Implement `sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[Any, int]`:
     - **Mock path** (when `ANVILML_WORKER_MOCK=1`): return `(MockLatent(), seed)` immediately. No torch/diffusers imports.
     - **Real path stub**: raise `NotImplementedError("Real ZiT sampling path not yet implemented — use ANVILML_WORKER_MOCK=1 for testing")`. The real implementation will:
       - Check `cancel_flag.is_set()` at every step via the `callback_on_step_end` hook.
       - Call `emit_progress(step, total_steps)` per step.
       - Assemble `ZImagePipeline` from cached transformer/vae/text_encoder components via `pipeline_cache.get_or_load(f"{model_id}:pipeline", ...)`.
       - Keep transformer at `float8` dtype (no upcast) when `InferenceCaps.fp8=True`; text_encoder/vae stay `bf16`.
     - Import guard: `torch`, `diffusers`, and `safetensors` must never be imported at module top level. Any real-mode imports must be inside the `if not _mock:` guard.
   - Include inline comments at every decision point (mock check, can_handle condition, NotImplementedError stub).
   - Include Google-style docstrings on all public functions.

3. **Create `worker/tests/test_arch_zit.py`** — Unit tests:
   - `test_can_handle_zit`: Verify `can_handle` returns `True` for a model object with `arch == "zit"`.
   - `test_can_handle_non_zit`: Verify `can_handle` returns `False` for a model object with `arch == "flux"` or no `arch` attribute.
   - `test_sample_mock_returns_mock_latent_and_seed`: Verify `sample()` in mock mode returns `(MockLatent(), seed)` with correct seed.
   - `test_sample_mock_no_torch_import`: Verify the module can be imported without torch (mock mode isolation).

4. **Verify with pytest**: Run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` — must exit 0 with ≥ 3 tests.

## Public API Surface

| Module Path | Item | Signature | Description |
|-------------|------|-----------|-------------|
| `worker/nodes/arch/__init__.py` | `can_handle` | `def can_handle(model_obj: Any) -> bool` | Iterate loaded arch modules; return True if any `can_handle()` matches. |
| `worker/nodes/arch/zit.py` | `can_handle` | `def can_handle(model_obj: Any) -> bool` | Return True if `model_obj.arch == "zit"`. |
| `worker/nodes/arch/zit.py` | `sample` | `def sample(model: Any, conditioning: Any, latent: Any, steps: int, cfg: float, seed: int, device: str, cancel_flag: Any, emit_progress: Callable[[int, int], None]) -> tuple[Any, int]` | Run ZiT sampling loop. Returns (latent_tensor, actual_seed). Mock path returns (MockLatent(), seed). |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/__init__.py` | Architecture registry package with `can_handle()` dispatcher and auto-import of arch modules. |
| CREATE | `worker/nodes/arch/zit.py` | ZiT FP8 dispatch module: `can_handle()`, `sample()` with mock path. |
| CREATE | `worker/tests/test_arch_zit.py` | Unit tests for `can_handle` and `sample` (mock mode). |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_can_handle_zit` | `can_handle()` returns True for a model with `arch == "zit"` | `ANVILML_WORKER_MOCK=1` set by conftest | model object with `arch = "zit"` | `True` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_can_handle_zit -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_can_handle_non_zit` | `can_handle()` returns False for non-ZiT models | `ANVILML_WORKER_MOCK=1` set by conftest | model object with `arch = "flux"` or no `arch` | `False` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_can_handle_non_zit -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_mock_returns_mock_latent_and_seed` | `sample()` returns `(MockLatent(), seed)` in mock mode | `ANVILML_WORKER_MOCK=1` set by conftest | `seed=42`, all other args as None or empty | `(MockLatent(), 42)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_sample_mock_no_torch_import` | Module imports cleanly without torch in mock mode | `ANVILML_WORKER_MOCK=1` set by conftest | Module import | No ImportError | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import -v` exits 0 |

## CI Impact

No CI changes required. The new test file `worker/tests/test_arch_zit.py` follows the existing convention of one test file per source module under `worker/tests/`. The `worker-linux` and `worker-windows` CI jobs run `pytest worker/tests/ -v` which automatically discovers and runs this new test file. No new file types, gates, or test modules are introduced beyond the standard convention.

## Platform Considerations

None identified. The Python worker code uses only standard library modules (`os`, `pkgutil`, `importlib`, `logging`, `threading`) and the project's own modules (`worker.nodes.base`, `worker.pipeline_cache`). No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `diffusers.ZImagePipeline` API may differ between 0.36.0 and 0.38.0 — constructor signature, pipeline assembly method, or `callback_on_step_end` hook may have changed. | Medium | High | The real path is stubbed with `NotImplementedError` in this task. At ACT time, the acting agent must verify `ZImagePipeline` exists in the installed diffusers version via `python -c "from diffusers import ZImagePipeline"` and confirm the constructor accepts `transformer`, `vae`, `text_encoder` keyword arguments. If the API differs, the real path implementation must adapt. |
| The `can_handle` dispatcher in `arch/__init__.py` may encounter arch modules that have not yet been implemented (empty directory). | Low | Low | The dispatcher iterates over loaded modules' `can_handle` functions. If no arch modules are loaded yet, it simply returns `False`. This is correct behavior — no arch matches means no sample path is selected. |
| `MockLatent` defined in `zit.py` duplicates the one in `sampler.py`. | Low | Low | Both `MockLatent` classes serve different purposes: `sampler.MockLatent` carries dimensions from the `EmptyLatent` node, while `arch.MockLatent` is a bare sentinel for the arch module's mock output. They are intentionally separate — arch modules must not import from other arch-specific or node modules to maintain isolation. |
| The `sample()` function signature uses `emit_progress` as a callable rather than using `NodeContext.emit` directly. This decouples the arch module from the node context but requires the Sampler node to wrap the emit callable. | Low | Low | This matches the design doc's specified interface (`ANVILML_DESIGN.md §10.4`). The wrapper is a one-line adapter in the Sampler node's real path. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m py_compile worker/nodes/arch/__init__.py worker/nodes/arch/zit.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with ≥ 3 tests
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.arch import can_handle; assert callable(can_handle)"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.arch.zit import can_handle, sample, MockLatent; assert callable(can_handle); assert callable(sample)"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; import worker.nodes.arch.zit as z; print('import ok')" 2>&1` exits 0 (verifies no torch import at module load time)
