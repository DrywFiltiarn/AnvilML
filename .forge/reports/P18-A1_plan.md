# Plan Report: P18-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-A1                                        |
| Phase       | 018 — ZiT Generic Nodes                       |
| Description | worker/nodes/loader.py: LoadModel generic node |
| Depends on  | P17-A1 (worker_main Execute handler), P17-C1 (base.py + NODE_REGISTRY) |
| Project     | anvilml                                       |
| Planned at  | 2026-06-21T10:55:00Z                          |
| Attempt     | 1                                             |

## Objective

Create the `LoadModel` node in `worker/nodes/loader.py` — the first of three loader nodes for Phase 018. The node accepts a `model_id` STRING input and outputs a `MODEL` slot. In mock mode (`ANVILML_WORKER_MOCK=1`), it returns a lightweight `MockModel` sentinel with `arch="zit"`. In real mode, it would use `safetensors.safe_open` and `pipeline_cache.get_or_load()` to load actual FP8 safetensors. Every public class and function has a docstring; every decision point has an inline comment.

## Scope

### In Scope
- Create `worker/nodes/loader.py` with the `LoadModel` class:
  - `NODE_TYPE = "LoadModel"`, `CATEGORY = "Loaders"`, `DISPLAY_NAME = "Load Model"`
  - `INPUT_SLOTS = [SlotSpec("model_id", "STRING")]`
  - `OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]`
  - `execute(**inputs)` method with mock and real paths
- Define `MockModel` class as a simple sentinel for mock mode
- Create `worker/tests/test_nodes_loader.py` with ≥4 tests
- Import the module into `NODE_REGISTRY` via the auto-import mechanism (the `__init__.py` scans for `.py` files and imports them; `loader.py` will be picked up automatically)

### Out of Scope
- `LoadVae` and `LoadClip` nodes (tasks P18-A2 and P18-A3)
- Real safetensors loading logic (only stub with inline comment explaining the path)
- `pipeline_cache.py` (task P18-D1)
- Architecture dispatch module (`worker/nodes/arch/`) — task P18-C1
- Any Rust-side changes

## Existing Codebase Assessment

The codebase has a well-established node registration infrastructure. `worker/nodes/base.py` defines `BaseNode` (ABC with `execute()` abstract method), `SlotSpec` (dataclass with `name`, `slot_type`, `optional`), `NodeContext` (runtime context with `job_id`, `device`, `cancel_flag`, `emit`, `pipeline_cache`), and the `@register` decorator that validates six metadata attributes and stores the class in `NODE_REGISTRY`.

`worker/nodes/__init__.py` auto-imports all sibling `.py` modules via `pkgutil.iter_modules()`, excluding `base.py`. This means any new `.py` file placed in `worker/nodes/` (like `loader.py`) will be automatically registered at import time — no manual registration needed.

`worker/nodes/image.py` provides the established pattern for mock-mode node implementations: a `@register`-decorated class extending `BaseNode`, with mock sentinel objects and stdlib-only imports. The `SaveImage` node generates a minimal PNG using only Python stdlib (no PIL, torch, or diffusers).

`worker/tests/test_nodes_base.py` demonstrates the test style: `@pytest.fixture(autouse=True) registry_clean` clears `NODE_REGISTRY` before each test, and tests use direct class definitions or the `_make_node_class` helper.

`worker/tests/conftest.py` sets `ANVILML_WORKER_MOCK=1` for every test via an autouse fixture, ensuring mock mode is always active during testing.

No `MockModel` or similar sentinel exists yet — this task introduces the first model sentinel type.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------|-----------------|----------------|------------------------|
| python | safetensors | 0.4+ (in base.txt) | pypi-query MCP | n/a                    |

The `safetensors` package is already declared in `worker/requirements/base.txt` (line 4: `safetensors>=0.4`). No new external dependencies are introduced by this task — the real path uses `safetensors.safe_open` and `pipeline_cache.get_or_load()`, both of which are either existing or planned (P18-D1) modules. The mock path imports nothing beyond Python stdlib.

## Approach

1. **Create `worker/nodes/loader.py`** with the `LoadModel` node class.

   The module begins with a Google-style module docstring describing the file's purpose, the mock/real path split, and the constraint that `torch`/`diffusers`/`safetensors` must never be imported at the top level.

   Define `MockModel` as a simple sentinel class:
   ```python
   class MockModel:
       """Sentinel model object for mock mode.

       Carries only the `arch` attribute so that downstream nodes
       (Sampler, etc.) can inspect the model architecture without
       needing a real diffusers pipeline object.
       """
       def __init__(self, arch: str) -> None:
           self.arch = arch
   ```

   Define `LoadModel` as a `@register`-decorated class extending `BaseNode`:
   ```python
   @register
   class LoadModel(BaseNode):
       """Load a diffusion model from a safetensors file.

       Accepts a ``model_id`` string input and returns a ``MODEL``
       slot containing either a real loaded pipeline component
       (in non-mock mode) or a ``MockModel`` sentinel (in mock mode).

       Attributes:
           NODE_TYPE: The type string used by the scheduler to route
               jobs to this node.
           CATEGORY: The UI category for this node type.
           DISPLAY_NAME: Human-readable name shown in UI.
           DESCRIPTION: Brief description of node behaviour.
           INPUT_SLOTS: One required ``STRING`` slot named ``"model_id"``.
           OUTPUT_SLOTS: One ``MODEL`` slot named ``"model"``.
       """

       NODE_TYPE = "LoadModel"
       CATEGORY = "Loaders"
       DISPLAY_NAME = "Load Model"
       DESCRIPTION = "Load a diffusion model (UNet or DiT) from a safetensors file"
       INPUT_SLOTS = [SlotSpec("model_id", "STRING")]
       OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]

       def execute(self, **inputs: Any) -> dict[str, Any]:
           """Execute the LoadModel node.

           Reads the ``model_id`` input, checks mock mode, and either
           returns a ``MockModel`` sentinel or loads a real model via
           safetensors + pipeline_cache.

           Args:
               **inputs: Must contain ``"model_id"`` — the identifier
                   of the model to load.

           Returns:
               Dict with key ``"model"`` containing either a ``MockModel``
               (mock mode) or a loaded pipeline model object (real mode).
           """
           # Read the model_id input. In mock mode this is a
           # placeholder string; in real mode it references a
           # model directory or file path registered in the model store.
           model_id = inputs.get("model_id", "")

           # Check mock mode by inspecting the environment variable.
           # This must be a runtime check (not a module-level import)
           # so that CI tests running with ANVILML_WORKER_MOCK=1
           # never touch torch/diffusers/safetensors at import time.
           if os.environ.get("ANVILML_WORKER_MOCK") == "1":
               # In mock mode, return a lightweight sentinel object
               # instead of loading a real pipeline. This keeps tests
               # fast and avoids requiring GPU hardware or torch.
               # The arch="zit" matches the Phase 018 baseline model.
               return {"model": MockModel(arch="zit")}

           # Real mode: load actual safetensors weights.
           # This path is stubbed — the real implementation will use
           # safetensors.safe_open() to read weight tensors, detect
           # architecture from metadata, and load via
           # pipeline_cache.get_or_load(). The pipeline_cache module
           # is implemented in task P18-D1.
           # TODO(P18-A1): Implement real safetensors loading path.
           raise NotImplementedError(
               "Real LoadModel path not yet implemented — "
               "use ANVILML_WORKER_MOCK=1 for testing"
           )
   ```

   The module-level imports are: `from __future__ import annotations`, `import os`, `from typing import Any`, and `from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register`. No `torch`, `diffusers`, or `safetensors` imports at the top level.

2. **Create `worker/tests/test_nodes_loader.py`** with ≥4 tests.

   Follow the established test patterns from `test_nodes_base.py` and `test_executor.py`:
   - `registry_clean` autouse fixture to clear `NODE_REGISTRY` before each test
   - `mock_context` fixture to build a `NodeContext` with captured emit callable
   - Direct import of `LoadModel` from `worker.nodes.loader` after the registry is cleaned

   Tests:
   a. **`test_loadmodel_registered_in_registry`** — Verify `LoadModel` is in `NODE_REGISTRY` after importing `worker.nodes.loader`. Assert `NODE_TYPE == "LoadModel"`, `CATEGORY == "Loaders"`, correct slot specs.

   b. **`test_loadmodel_execute_returns_mock_model`** — Execute `LoadModel` with `model_id="test-model"`. Assert the returned dict has key `"model"` and the value is a `MockModel` with `arch == "zit"`.

   c. **`test_loadmodel_execute_missing_model_id_defaults_empty`** — Execute `LoadModel` without providing `model_id`. Assert it returns a `MockModel` (mock mode ignores the model_id).

   d. **`test_loadmodel_metadata_attributes`** — Verify all six required metadata attributes: `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`. Assert each has the correct value and type.

3. **Verify Python syntax** — Run `worker/.venv/bin/python -m py_compile worker/nodes/loader.py worker/tests/test_nodes_loader.py` to confirm no syntax errors before running pytest.

4. **Run tests** — Execute `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` and verify all tests pass.

## Public API Surface

| Item | Module Path | Description |
|------|-------------|-------------|
| `class MockModel` | `worker.nodes.loader` | Sentinel model object for mock mode. Constructor: `MockModel(arch: str)`. Attribute: `arch: str`. |
| `class LoadModel` | `worker.nodes.loader` | `@register`-decorated node extending `BaseNode`. Metadata: `NODE_TYPE="LoadModel"`, `CATEGORY="Loaders"`, `DISPLAY_NAME="Load Model"`, `DESCRIPTION="Load a diffusion model (UNet or DiT) from a safetensors file"`. Slots: `INPUT_SLOTS=[SlotSpec("model_id", "STRING")]`, `OUTPUT_SLOTS=[SlotSpec("model", "MODEL")]`. Method: `execute(**inputs: Any) -> dict[str, Any]` — reads `model_id`, returns `{"model": MockModel(arch="zit")}` in mock mode. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/loader.py` | LoadModel node implementation with MockModel sentinel |
| CREATE | `worker/tests/test_nodes_loader.py` | ≥4 unit tests for LoadModel node |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_registered_in_registry` | `LoadModel` is registered in `NODE_REGISTRY` with correct `NODE_TYPE` key | `NODE_REGISTRY` cleared by `registry_clean` fixture; `worker.nodes.loader` imported | None (module import triggers `@register`) | `"LoadModel" in NODE_REGISTRY`, `NODE_REGISTRY["LoadModel"] is LoadModel` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_execute_returns_mock_model` | `execute()` returns `{"model": MockModel(arch="zit")}` in mock mode | `ANVILML_WORKER_MOCK=1` set by `conftest.py` autouse fixture; `NODE_REGISTRY` cleared; `LoadModel` instantiated with `mock_context` | `model_id="test-model"` | `result["model"]` is `MockModel`, `result["model"].arch == "zit"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_execute_missing_model_id_defaults_empty` | `execute()` handles missing `model_id` gracefully in mock mode | Same as above | `{}` (no `model_id` key) | `result["model"]` is `MockModel(arch="zit")` — mock mode ignores model_id | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty -v` exits 0 |
| `worker/tests/test_nodes_loader.py` | `test_loadmodel_metadata_attributes` | All six required metadata attributes are correct | `LoadModel` class available via direct import | None | `NODE_TYPE="LoadModel"`, `CATEGORY="Loaders"`, `DISPLAY_NAME="Load Model"`, `DESCRIPTION` non-empty, `INPUT_SLOTS` has one `SlotSpec("model_id", "STRING")`, `OUTPUT_SLOTS` has one `SlotSpec("model", "MODEL")` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes -v` exits 0 |

## CI Impact

No CI changes required. The new `.py` files under `worker/` are automatically picked up by the existing `worker-linux` and `worker-windows` CI jobs, which run `py_compile` on all `worker/*.py` files and then `pytest worker/tests/ -v`. The new test file `test_nodes_loader.py` will be discovered and executed by pytest automatically. No CI workflow modifications are needed.

## Platform Considerations

None identified. The `LoadModel` node uses only Python stdlib (`os`, `typing`) and the project's own `BaseNode`/`SlotSpec`/`register` infrastructure. No platform-specific paths, line-endings, or path-separator handling is involved. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `__init__.py` auto-import may fail if `loader.py` has an import error at module load time | Low | Medium | The `__init__.py` catches `ImportError` and logs a warning — a broken `loader.py` won't crash the worker, it just won't be registered. Write the module carefully and run `py_compile` before testing. |
| `ANVILML_WORKER_MOCK` env var check at runtime may not work if the conftest fixture doesn't set it early enough | Low | Low | The `conftest.py` autouse `mock_mode` fixture sets the env var before each test. This is a well-established pattern used by all existing tests. |
| `NODE_REGISTRY` pollution between tests — `LoadModel` registered in one test may appear in another | Medium | Medium | The `registry_clean` fixture clears `NODE_REGISTRY` before each test. Since `LoadModel` is a module-level class decorated with `@register`, re-importing the module after clearing will re-register it. Tests that import `worker.nodes.loader` directly must also clear the registry. |
| Real path `NotImplementedError` may be caught by downstream code expecting a dict | Low | Low | The real path is unreachable in mock mode (the `if os.environ.get("ANVILML_WORKER_MOCK") == "1"` branch returns first). Tests run in mock mode, so the real path is never executed. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/loader.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_nodes_loader.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v` exits 0 with ≥ 4 tests
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` exits 0 (full test suite, no regressions)
