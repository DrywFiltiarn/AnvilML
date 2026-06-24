# Plan Report: P904-A13

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-A13                                      |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | worker/nodes/arch/diffusion/__init__.py: add an arch-by-name lookup, since LoadModel/LoadVae dispatch before any model object exists |
| Depends on  | P904-A10, P904-A11                             |
| Project     | anvilml                                       |
| Planned at  | 2026-06-24T12:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Provide a name-based architecture lookup (`get_module_by_name(arch: str) -> ModuleType | None`) in `worker/nodes/arch/diffusion/__init__.py` so that `LoadModel` and `LoadVae` (P904-A12) can dispatch to the correct architecture module using only a bare architecture string extracted from safetensors metadata — no model object needs to exist at dispatch time. This is a pure addition alongside the existing `get_module(model_obj)`, with zero risk to already-passing callers. The function is already implemented in source; this task completes it by adding test coverage.

## Scope

### In Scope
- Verify the existing `get_module_by_name(arch: str) -> ModuleType | None` implementation in `worker/nodes/arch/diffusion/__init__.py` matches the task specification (uses `pkgutil.iter_modules()` scan, delegates to `get_module()` with a minimal shim object carrying only `.arch = arch`, does not change `can_handle()` or `get_module()`).
- Add unit tests for `get_module_by_name` in `worker/tests/test_arch_init.py`: test that it returns the zit module for `arch="zit"`, returns `None` for unknown architectures, and that the shim object pattern works correctly (no real model construction).
- Update `docs/TESTS.md` to catalogue the new tests (per ENVIRONMENT.md §5.10 / §11.4).

### Out of Scope
- No changes to `can_handle()` or `get_module()` — these are not modified.
- No changes to `worker/nodes/arch/__init__.py` re-exports — `get_module_by_name` is imported directly from `worker.nodes.arch.diffusion` by `loader.py`, and re-exporting it at the parent level is not required by any caller.
- No changes to clip arch dispatch — `arch/clip/__init__.py` uses a different dispatch pattern (passes string directly to `can_handle()`) and is not affected.
- No changes to `loader.py` — it already imports and calls `get_module_by_name` correctly.

## Existing Codebase Assessment

The `get_module_by_name` function is already fully implemented in `worker/nodes/arch/diffusion/__init__.py` (lines 129–151). It was likely authored alongside the other P904-A10–A12 changes. The implementation:

1. **Pattern**: Defines a local `_Shim` class with a class-level `arch: str = arch` attribute, then calls `get_module(_Shim())`. This is correct because `zit.py`'s `can_handle()` reads `getattr(model_obj, "arch", None)` — a bare shim with just that one attribute satisfies it without constructing a real model.

2. **Existing callers**: `loader.py` calls `get_module_by_name(detected_arch)` in both `_load_model_from_safetensors` (line 661) and `_load_vae_from_safetensors` (line 711), confirming the function is wired into the production code path.

3. **Established patterns**: The diffusion arch module follows Google-style docstrings, `from __future__ import annotations`, and the `pkgutil.iter_modules()` + `importlib.import_module()` scan pattern shared by `get_module()`, `_ensure_imported()`, and the clip arch module. Error handling uses try/except with `continue` for import failures and `can_handle()` exceptions.

4. **Gap**: No tests exist for `get_module_by_name` in `worker/tests/test_arch_init.py`. The existing test file tests `get_module()` and `can_handle()` but not the new name-based lookup. This is the only gap — the implementation is complete and correct.

## Resolved Dependencies

None. This task introduces no new dependencies. It uses only stdlib modules (`pkgutil`, `importlib`, `types`) already available in the Python 3.12 runtime. The external API shapes (`pkgutil.iter_modules()`, `importlib.import_module()`, `getattr()`) are all standard library and do not require MCP verification.

## Approach

**Step 1: Verify existing implementation correctness.**
Confirm that the existing `get_module_by_name` in `worker/nodes/arch/diffusion/__init__.py`:
- Accepts `arch: str` and returns `ModuleType | None`
- Constructs a minimal shim with only `.arch = arch`
- Delegates to `get_module(_Shim())`
- Does not modify `can_handle()` or `get_module()`
- Has a Google-style docstring with Args/Returns sections
- Is listed in `__all__`

This is a read-only verification — no source changes needed for this step.

**Step 2: Add tests to `worker/tests/test_arch_init.py`.**
Add a new test section (after the existing `can_handle` delegation tests) with three test functions:

1. `test_get_module_by_name_returns_zit_for_zit()` — Call `get_module_by_name("zit")`, assert the returned module is not None and `mod.__name__ == "worker.nodes.arch.diffusion.zit"`.

2. `test_get_module_by_name_returns_none_for_unknown_arch()` — Call `get_module_by_name("unknown")`, assert it returns `None`.

3. `test_get_module_by_name_does_not_construct_real_model()` — Call `get_module_by_name("zit")` and verify the shim pattern works: the function should succeed without any real model object being constructed (this is implicitly verified by the mock-mode test environment where torch/diffusers are not importable, but we add an explicit assertion that the shim class is a simple class without any model-like attributes).

Each test imports `get_module_by_name` from `worker.nodes.arch.diffusion` (not from `worker.nodes.arch`, since that module does not re-export it).

**Step 3: Update `docs/TESTS.md`.**
Add catalogue entries for the three new tests, following the format defined in ANVILML_DESIGN.md §16.1.

**Step 4: Run syntax check and tests.**
Run `python3 -m py_compile worker/nodes/arch/diffusion/__init__.py worker/tests/test_arch_init.py` to confirm no syntax errors. Then run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py -v` to confirm all tests pass.

## Public API Surface

One new public function already exists (no new additions needed):

```python
def get_module_by_name(arch: str) -> ModuleType | None:
    """Return the first loaded arch module whose ``can_handle()`` matches *arch*.
    
    Constructs a shim object carrying ``arch = arch`` and passes it to
    ``get_module()``.  This lets callers dispatch by a bare architecture
    string (e.g. ``"zit"``) instead of needing a full model descriptor
    object.
    
    Args:
        arch: The architecture identifier string (e.g. ``"zit"``).
    
    Returns:
        The matching architecture module, or ``None`` if no loaded arch
        module's ``can_handle()`` returns ``True`` for the shim object.
    """
```

Module path: `worker.nodes.arch.diffusion.get_module_by_name`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| VERIFY | `worker/nodes/arch/diffusion/__init__.py` | Verify existing `get_module_by_name` implementation (no changes) |
| MODIFY | `worker/tests/test_arch_init.py` | Add three test functions for `get_module_by_name` |
| MODIFY | `docs/TESTS.md` | Catalogue entries for the three new tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_init.py` | `test_get_module_by_name_returns_zit_for_zit` | `get_module_by_name("zit")` returns the zit arch module | `ANVILML_WORKER_MOCK=1`, zit module is importable | `get_module_by_name("zit")` | `mod.__name__ == "worker.nodes.arch.diffusion.zit"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_get_module_by_name_returns_zit_for_zit -v` exits 0 |
| `worker/tests/test_arch_init.py` | `test_get_module_by_name_returns_none_for_unknown_arch` | `get_module_by_name("unknown")` returns `None` | `ANVILML_WORKER_MOCK=1`, no unknown arch module exists | `get_module_by_name("unknown")` | `result is None` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_get_module_by_name_returns_none_for_unknown_arch -v` exits 0 |
| `worker/tests/test_arch_init.py` | `test_get_module_by_name_shim_pattern` | The shim object pattern works: a bare class with only `.arch` satisfies `can_handle()` | `ANVILML_WORKER_MOCK=1` | `get_module_by_name("zit")` | Returns a module (same as calling `get_module(_make_model("zit"))`), confirming shim equivalence | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_get_module_by_name_shim_pattern -v` exits 0 |

## CI Impact

No CI changes. The new tests run within the existing `pytest worker/tests/` invocation (mock mode). They do not touch CI config files, do not require new dependencies, and do not change any gate behavior.

## Platform Considerations

None identified. The `pkgutil.iter_modules()` + `importlib.import_module()` scan is cross-platform. The shim class pattern is pure Python with no platform-specific behavior. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The existing `_Shim` class defined inside `get_module_by_name` uses a class-level annotation `arch: str = arch` — in Python 3.12 this works correctly (the default value is evaluated at class definition time, capturing the `arch` parameter), but if the Python version were to change to one with different class-body evaluation semantics, the behavior could differ | Low | Low | Python 3.12 is the required version (ENVIRONMENT.md §1.1). The class-level default pattern is standard and well-documented in PEP 563 / PEP 649. No change needed. |
| Tests import `get_module_by_name` directly from `worker.nodes.arch.diffusion` rather than from `worker.nodes.arch` — if a future task adds it to the parent re-export, tests would still pass (direct import works regardless), but the plan should note this is not a re-export gap | Low | Low | The import path `from worker.nodes.arch.diffusion import get_module_by_name` is correct and matches how `loader.py` imports it. No re-export at the parent level is needed. |
| The shim pattern relies on `get_module()` iterating modules and calling `can_handle()` — if a future arch module's `can_handle()` reads attributes beyond `.arch`, the shim would fail to match even the correct architecture | Low | Low | This is a design constraint documented in the function's docstring. Any new arch module should only read `.arch` from the model object (as `zit.py` already does). The constraint is enforced by the codebase inspection that every `can_handle()` reads `getattr(model_obj, "arch", None)`. |

## Acceptance Criteria

- [ ] `python3 -m py_compile worker/nodes/arch/diffusion/__init__.py worker/tests/test_arch_init.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py -v` exits 0 (all existing + new tests pass)
- [ ] `python3 -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion import get_module_by_name; assert get_module_by_name('zit') is not None; assert get_module_by_name('zit').__name__.endswith('.zit')"` exits 0
- [ ] `python3 -c "import os; os.environ['ANVILML_WORKER_MOCK']='1'; from worker.nodes.arch.diffusion import get_module_by_name; assert get_module_by_name('unknown') is None"` exits 0
- [ ] `grep -n "get_module_by_name" docs/TESTS.md` shows at least three entries for the new tests
