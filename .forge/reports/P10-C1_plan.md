# Plan Report: P10-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-C1                                      |
| Phase       | 10 — Generic Node Groundwork                |
| Description | worker/nodes/__init__.py: auto-import wiring for nodes/ submodules |
| Depends on  | P10-A1, P10-A2, P10-A3, P10-A4, P10-B1, P10-B2 |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T09:50:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the auto-import mechanism in `worker/nodes/__init__.py` that iterates all `.py` files directly under `worker/nodes/` (excluding `arch/`) via `importlib`, triggering each module's `@register` decorator as a side effect of import. At this phase, no concrete node files exist yet, so `NODE_REGISTRY` is correctly empty after import — that is the expected, correct state. This mechanism replaces Phase 9's placeholder stub and provides the wiring that Phase 10 Group D (P10-D1) will call from `worker_main.py`.

## Scope

### In Scope
- Create the auto-import loop in `worker/nodes/__init__.py` using `importlib` to discover and import all `.py` files directly under `worker/nodes/` (not recursively into `arch/`)
- Ensure idempotent re-import: importing `worker.nodes` a second time must not cause duplicate registration errors
- Write `worker/tests/test_nodes_init.py` with >=3 tests:
  1. Importing `worker.nodes` does not raise
  2. `NODE_REGISTRY` is empty immediately after import (no node files exist yet)
  3. Re-importing is idempotent (no duplicate registration error)

### Out of Scope
None. The `defers_to` field is `[]` (empty), and this task must implement its full scope without deferring any functionality.

## Existing Codebase Assessment

The `worker/nodes/` directory already exists with two files: `base.py` (complete, containing `SlotSpec`, `NODE_REGISTRY`, `@register`, `NodeContext`, and `BaseNode` — all from P10-A1 through P10-A4) and `__init__.py` (currently empty, 0 lines). The `arch/` subdirectory contains three dispatch packages (`diffusion/__init__.py`, `clip/__init__.py`, `vae/__init__.py`) with their `get_module()` dispatcher logic from P10-B1 and P10-B2.

The established test style uses plain `pytest` functions with descriptive docstrings, module-level helper classes for shared fixtures (e.g., `_FullySpecifiedNode` in `test_base.py`), and explicit cleanup of global state (`del base.NODE_REGISTRY[...]`) after tests that mutate it. Tests import from the specific sub-module (e.g., `from worker.nodes import base`) rather than the package-level namespace.

No gap exists between the design doc and current source for this task: `NODE_REGISTRY` is a module-level `dict[str, type["BaseNode"]]` in `base.py`, and `register()` is a simple function that inserts into it. The auto-import mechanism only needs to discover sibling `.py` modules and import them via `importlib`, triggering each module's `@register` decorator as a side effect.

## Resolved Dependencies

None. This task uses only Python standard library modules (`importlib`, `importlib.util`, `pkgutil`, `os`). No external crates, PyPI packages, or npm packages are introduced.

## Approach

1. **Write the auto-import loop in `worker/nodes/__init__.py`.**
   - The file declares a module-level `_imported: bool = False` flag for idempotency.
   - Defines `_import_nodes() -> None`: a function that, if not already called, iterates all `.py` files directly under `worker/nodes/` using `pkgutil.iter_modules([nodes_dir])`.
   - For each discovered module name, skips `__init__` (this package) and `base` (already loaded as a dependency of sibling modules). Skips packages (`is_pkg == True`) to avoid recursing into `arch/`.
   - For each remaining module, uses `importlib.util.find_spec(f"worker.nodes.{mod_name}")` to get the spec, creates the module object with `importlib.util.module_from_spec(spec)`, and executes it with `spec.loader.exec_module(module)`. This triggers the module's top-level code, including any `@register` decorator side effects.
   - The function is guarded by the `_imported` flag: on the second call, it returns immediately without re-executing any imports.
   - Calls `_import_nodes()` at module load time (bottom of `__init__.py`), so that `import worker.nodes` automatically triggers node registration.
   - The complete file:
     ```python
     """Node package — auto-imports node modules to trigger @register side effects."""

     _imported: bool = False

     def _import_nodes() -> None:
         """Import all .py files directly under nodes/ (not recursively into arch/).

         Each imported module's @register decorator side-effect populates
         NODE_REGISTRY. This function is idempotent — calling it a second
         time has no effect.
         """
         global _imported
         if _imported:
             return
         _imported = True

         import os
         import pkgutil
         import importlib.util

         nodes_dir = os.path.dirname(__file__)
         for _importer, mod_name, is_pkg in pkgutil.iter_modules([nodes_dir]):
             # Skip __init__ (this package) and base (loaded as dependency).
             # Skip packages (is_pkg=True) to avoid recursing into arch/.
             if mod_name in ("__init__", "base") or is_pkg:
                 continue
             spec = importlib.util.find_spec(f"worker.nodes.{mod_name}")
             if spec is None:
                 continue
             module = importlib.util.module_from_spec(spec)
             spec.loader.exec_module(module)

     # Run auto-import at package load time.
     _import_nodes()
     ```
   - Rationale for `pkgutil.iter_modules` + `importlib.util`: this is the same pattern referenced in `ANVILML_DESIGN.md §10.4` for arch dispatch, ensuring consistency across the node system.
   - Rationale for skipping `base`: `base.py` defines `NODE_REGISTRY` and `register()`. Sibling modules import these via `from .base import register` and `from .base import NODE_REGISTRY`, which triggers `base.py`'s execution as a side effect of the relative import. We don't need to import it explicitly here.
   - Rationale for skipping packages (`is_pkg`): this prevents recursing into `arch/`, which is imported by its family's dispatcher, not by this top-level loop.

2. **Write `worker/tests/test_nodes_init.py`.**
   - Follow the established test style: plain `pytest` functions with docstrings, no class wrapper needed (this is a simple module-level test file).
   - Test 1: `test_import_does_not_raise` — import `worker.nodes` and assert no exception.
   - Test 2: `test_node_registry_empty_after_import` — import `worker.nodes`, then import `worker.nodes.base` and assert `NODE_REGISTRY == {}`.
   - Test 3: `test_reimport_is_idempotent` — import `worker.nodes` twice (or call `_import_nodes()` twice), assert no exception and no error.
   - Each test has a descriptive docstring explaining what it verifies and the expected outcome.

## Public API Surface

| Path | Item | Description |
|------|------|-------------|
| `worker.nodes._imported` | Module-level `bool` | Idempotency flag for the auto-import loop. |
| `worker.nodes._import_nodes()` | Module-level function | Imports all `.py` files directly under `nodes/`, triggering `@register` side effects. Idempotent. |

No new public types, classes, or functions are introduced. The only change is the auto-import mechanism in `__init__.py`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/__init__.py` | Add auto-import loop using `pkgutil.iter_modules()` + `importlib.util` |
| CREATE | `worker/tests/test_nodes_init.py` | Tests for the auto-import mechanism (>=3 tests) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_init.py` | `test_import_does_not_raise` | Importing `worker.nodes` does not raise an exception. | None. | None. | Import succeeds silently. | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py::test_import_does_not_raise -v` exits 0 |
| `worker/tests/test_nodes_init.py` | `test_node_registry_empty_after_import` | `NODE_REGISTRY` is empty immediately after import (no concrete node files exist yet). | `worker.nodes` has been imported. | None. | `base.NODE_REGISTRY == {}`. | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py::test_node_registry_empty_after_import -v` exits 0 |
| `worker/tests/test_nodes_init.py` | `test_reimport_is_idempotent` | Re-importing or re-calling `_import_nodes()` does not raise or duplicate registrations. | `worker.nodes` has been imported once. | None. | No exception on second import/call; `NODE_REGISTRY` still empty. | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py::test_reimport_is_idempotent -v` exits 0 |

## CI Impact

The new test file `worker/tests/test_nodes_init.py` is picked up by the existing CI jobs:
- `worker-linux-mock`: runs `pytest worker/tests -v -m "not real_mode"` — this test has no `real_mode` marker, so it runs in the mock CI job.
- `worker-linux-real`: runs `pytest worker/tests -v -m real_mode` — this test has no `real_mode` marker, so it does NOT run in the real CI job. This is correct: the auto-import mechanism has no real-mode vs mock-mode divergence (it doesn't import torch at all), so it only needs to run in the mock job.
- `worker-windows-mock` and `worker-windows-real`: same behavior on Windows.

No CI configuration changes are needed.

## Platform Considerations

None identified. The auto-import mechanism uses `os.path.dirname(__file__)` which is cross-platform. `pkgutil.iter_modules()` and `importlib.util` are standard library modules with identical behavior on Linux, macOS, and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pkgutil.iter_modules()` on a directory that contains `__pycache__/` or `.pyc` files could surface non-module entries. | Low | Medium | `pkgutil.iter_modules` only yields actual importable modules (it checks the filesystem for `.py` files and `__init__.py` directories). The `is_pkg` flag distinguishes packages from modules. No additional filtering needed. |
| Importing sibling modules via `importlib.util` could fail if a sibling module has a top-level import error (e.g., missing dependency). | Low | High | A single module's import failure would crash the entire `_import_nodes()` call, preventing the package from loading. Mitigation: wrap the `exec_module` call in a try/except that logs the failure and continues to the next module. At this phase, no concrete node files exist, so this risk is theoretical but the guard is cheap. |
| The `_imported` flag approach could leak state if `sys.modules` is manipulated (e.g., `sys.modules.pop("worker.nodes")` then re-import). | Low | Low | This is an edge case that would require deliberate `sys.modules` manipulation. The flag is sufficient for normal use. If needed, a future task can add a reset function. |
| `importlib.util.find_spec()` returns `None` for modules that cannot be found (e.g., a `.py` file that isn't a valid Python module). | Low | Low | The `spec is None` check already handles this — such modules are skipped. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/__init__.py worker/tests/test_nodes_init.py` exits 0
- [ ] `worker/.venv/bin/python -c "import worker.nodes; print('OK')"` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py -v` exits 0 with >=3 tests passing
