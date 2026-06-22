# Plan Report: P18-D2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D2                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/__init__.py: add get_module() dispatcher returning matching arch module |
| Depends on  | P903-A2                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T07:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Add `get_module(model_obj) -> ModuleType | None` to `worker/nodes/arch/__init__.py`, returning the actual architecture module that claims a given model object. Refactor `can_handle()` to delegate to `get_module()` internally so there is exactly one iteration implementation. This enables downstream tasks (P18-D8 EmptyLatent, P18-D10 Sampler) to call `.sample()` or `.compute_latent_shape()` on the matching module, which is impossible with the current boolean-only `can_handle()`.

## Scope

### In Scope
- Add `get_module(model_obj: Any) -> ModuleType | None` function in `worker/nodes/arch/__init__.py`
- Refactor existing `can_handle()` to delegate to `get_module() is not None` (single iteration, no duplicated loop logic)
- Add `get_module` to `__all__`
- Import `ModuleType` from `types` module
- Create `worker/tests/test_arch_init.py` with ≥ 3 tests verifying:
  - `get_module()` returns the zit module for `arch="zit"`
  - `get_module()` returns `None` for unknown architecture
  - `can_handle()` still works correctly post-refactor

### Out of Scope
- Adding new arch modules (flux.py) — handled by later tasks
- Any changes to `worker/nodes/sampler.py` or other consumers — those are separate tasks (P18-D8, P18-D10)
- Any changes to `worker/nodes/arch/zit.py` — that file's public API is unchanged

## Existing Codebase Assessment

The file `worker/nodes/arch/__init__.py` (130 lines) currently implements two functions:

1. `_ensure_imported()` — idempotent auto-import of sibling `.py` modules in the `arch/` directory using `pkgutil.iter_modules(__path__)` and `importlib.import_module()`. Called at module load time (line 82).

2. `can_handle(model_obj)` — iterates the same `pkgutil.iter_modules(__path__)` loop, imports each module, checks for a `can_handle` attribute, calls it, and returns `True` on first match.

The `worker/nodes/arch/zit.py` module exports `can_handle(model_obj)` which checks `model_obj.arch == "zit"`. No `flux.py` module exists yet (Phase 019).

The test file `worker/tests/test_arch_zit.py` (265 lines) tests the zit module's `can_handle()` and `sample()` functions. No `test_arch_init.py` exists yet.

The project follows Google-style docstrings for Python, uses `from __future__ import annotations`, and all Python tests live in `worker/tests/` with one file per source module. The `conftest.py` autouse fixture sets `ANVILML_WORKER_MOCK=1` for all tests.

The design doc (ANVILML_DESIGN.md §10.4, lines 1111–1128) specifies both `can_handle()` and `get_module()` as the public API of `arch/__init__.py`, confirming this task aligns with the planned architecture.

## Resolved Dependencies

None. This task uses only Python standard library modules: `pkgutil`, `importlib`, `types`, `logging`, `typing.Any`. No new external packages are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| stdlib | types (ModuleType) | Python 3.12 built-in | n/a | n/a |

## Approach

1. **Add `ModuleType` import.** In `worker/nodes/arch/__init__.py`, add `from types import ModuleType` alongside the existing imports (`importlib`, `logging`, `pkgutil`, `Any`). This is the standard-library type for Python module objects.

2. **Update `__all__`.** Change `__all__ = ["can_handle"]` to `__all__ = ["can_handle", "get_module"]`. This exposes the new function to importers who use `from worker.nodes.arch import *`.

3. **Implement `get_module()` function.** Add a new function after `can_handle()` (or before, since `can_handle` will call it):

   ```python
   def get_module(model_obj: Any) -> ModuleType | None:
       """Return the first loaded arch module whose can_handle() matches.

       Iterates through all modules in this package's namespace,
       imports each one, and checks if it exposes a ``can_handle()``
       function that returns ``True`` for the given model object.
       Returns the matching module on first match, or ``None`` if
       no module matches.

       Args:
           model_obj: A model descriptor object carrying attributes
               like ``arch`` (architecture type string).

       Returns:
           The matching architecture module, or ``None`` if no
           loaded arch module claims this model.
       """
       for mod_info in pkgutil.iter_modules(__path__):
           if mod_info.ispkg:
               continue

           full_name = f"worker.nodes.arch.{mod_info.name}"

           try:
               mod = importlib.import_module(full_name)
           except ImportError:
               # Module failed to import earlier; skip it.
               # The warning was already logged by _ensure_imported().
               continue

           handler = getattr(mod, "can_handle", None)
           if handler is not None:
               try:
                   if handler(model_obj):
                       return mod  # Found the matching module
               except Exception:
                   # A can_handle() that raises is a bug in that
                   # module; skip it rather than failing dispatch.
                   continue

       return None
   ```

   This is structurally identical to the existing `can_handle()` loop body, but returns `mod` instead of `True`. The iteration logic is shared — `can_handle()` will delegate to this.

4. **Refactor `can_handle()` to delegate to `get_module()`.** Replace the body of `can_handle()` with a single delegation:

   ```python
   def can_handle(model_obj: Any) -> bool:
       """Check whether any loaded architecture module claims this model.

       Delegates to ``get_module()`` — returns ``True`` if a matching
       module is found, ``False`` otherwise. This avoids duplicating
       the module iteration logic that both functions need.

       Args:
           model_obj: A model descriptor object carrying attributes
               like ``arch`` (architecture type string), ``model_id``, etc.

       Returns:
           ``True`` if at least one loaded arch module's ``can_handle()``
           function returns ``True`` for the model; ``False`` otherwise.
       """
       return get_module(model_obj) is not None
   ```

   The docstring is preserved (same content, just updated to note delegation). The function body shrinks from ~30 lines of iteration to one delegation line. This is the key change: exactly one iteration implementation lives in `get_module()`.

5. **Create `worker/tests/test_arch_init.py`.** New test file with ≥ 3 tests:

   - `test_get_module_returns_zit_for_zit_model`: Constructs a model with `arch="zit"`, calls `get_module()`, asserts the returned module is `worker.nodes.arch.zit` (check via `mod.__name__ == "worker.nodes.arch.zit"`).
   - `test_get_module_returns_none_for_unknown_arch`: Constructs a model with `arch="unknown"`, calls `get_module()`, asserts `None`.
   - `test_can_handle_still_works_after_refactor`: Calls `can_handle()` with `arch="zit"` (expect `True`) and `arch="unknown"` (expect `False`), verifying the delegation works correctly and produces identical results to the original implementation.

   Test file structure mirrors the existing `test_arch_zit.py` style: Google docstrings, `_make_model()` helper, `from __future__ import annotations`.

6. **Pre-stop verification.** Run `head -1`, `grep "^## "`, and `wc -l` on the report file to confirm all 12 headings are present and the file is > 40 lines.

## Public API Surface

| Module | Item | Signature | Description |
|--------|------|-----------|-------------|
| `worker.nodes.arch` | `get_module` (new) | `def get_module(model_obj: Any) -> ModuleType \| None` | Returns the first loaded arch module whose `can_handle()` matches the model, or `None` |
| `worker.nodes.arch` | `can_handle` (modified) | `def can_handle(model_obj: Any) -> bool` (unchanged signature) | Now delegates to `get_module(model_obj) is not None`; signature and behaviour unchanged |
| `worker.nodes.arch` | `__all__` (modified) | `["can_handle", "get_module"]` | Previously `["can_handle"]` |

No new types, traits, or structs are introduced. The `ModuleType` type is from the Python standard library `types` module.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/__init__.py` | Add `get_module()`, refactor `can_handle()` to delegate, update `__all__`, import `ModuleType` |
| CREATE | `worker/tests/test_arch_init.py` | New test file with ≥ 3 tests for `get_module()` and `can_handle()` delegation |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_init.py` | `test_get_module_returns_zit_for_zit_model` | `get_module()` returns the zit module for a model with `arch="zit"` | `ANVILML_WORKER_MOCK=1` (conftest autouse) | `_make_model("zit")` | `mod.__name__ == "worker.nodes.arch.zit"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_get_module_returns_zit_for_zit_model -v` exits 0 |
| `worker/tests/test_arch_init.py` | `test_get_module_returns_none_for_unknown_arch` | `get_module()` returns `None` for a model with unknown architecture | `ANVILML_WORKER_MOCK=1` (conftest autouse) | `_make_model("flux")` | `result is None` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_get_module_returns_none_for_unknown_arch -v` exits 0 |
| `worker/tests/test_arch_init.py` | `test_can_handle_still_works_after_refactor` | `can_handle()` returns correct bool for both matching and non-matching models post-refactor | `ANVILML_WORKER_MOCK=1` (conftest autouse) | `_make_model("zit")` and `_make_model("unknown")` | `True` for zit, `False` for unknown | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_can_handle_still_works_after_refactor -v` exits 0 |

## CI Impact

No CI changes required. The new test file `worker/tests/test_arch_init.py` follows the existing convention (one test file per source module in `worker/tests/`) and is automatically picked up by the existing pytest invocation `worker/.venv/bin/python -m pytest worker/tests/ -v` used in all CI jobs (`worker-linux`, `worker-windows`). No CI workflow files are modified.

## Platform Considerations

None identified. The `pkgutil.iter_modules(__path__)` pattern, `importlib.import_module()`, and `types.ModuleType` are all cross-platform standard library APIs with identical behaviour on Linux and Windows. No `#` conditional imports or path-separator handling required — `__path__` is managed by Python's import machinery.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `get_module()` and `can_handle()` are called before `_ensure_imported()` has run (unlikely since `_ensure_imported()` is called at module load time on line 82, but a test could import `can_handle` from `worker.nodes.arch.zit` directly without going through the package `__init__`) | Low | Medium | The `pkgutil.iter_modules(__path__)` loop imports each module on-demand (same as the original `can_handle()`), so even if `_ensure_imported()` hasn't run, the iteration will import modules as needed. The `importlib.import_module(full_name)` call inside the loop handles this. |
| Refactoring `can_handle()` to delegate to `get_module()` changes the exception handling semantics: the original `can_handle()` catches exceptions from individual `can_handle()` calls and continues iterating; the new `get_module()` also catches exceptions but returns `None` instead of continuing — however, both have identical exception handling in the inner try/except, so behaviour is preserved | Low | Low | The try/except around `handler(model_obj)` is identical in both functions. If an exception is raised, both skip that module and continue. The only difference is the return value (True vs mod), which is the intended change. |
| Test file imports `worker.nodes.arch` which triggers `_ensure_imported()` and imports zit.py, but the test also needs to verify the returned module is specifically `worker.nodes.arch.zit` — if zit.py fails to import, the test would fail spuriously | Low | Medium | The existing `test_arch_zit.py` already imports `worker.nodes.arch.zit` directly, proving it works. The `conftest.py` mock-mode fixture ensures no torch is loaded. The `_ensure_imported()` warning path handles import failures gracefully. |

## Acceptance Criteria

- [ ] `head -1 .forge/reports/P18-D2_plan.md` prints `# Plan Report: P18-D2`
- [ ] `grep "^## " .forge/reports/P18-D2_plan.md` returns exactly 12 heading lines
- [ ] `wc -l .forge/reports/P18-D2_plan.md` returns > 40
- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/__init__.py worker/tests/test_arch_init.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py -v` exits 0 with ≥ 3 tests passing
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 (existing zit tests still pass post-refactor)
