# Plan Report: P18-D8

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D8                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/clip/__init__.py: clip dispatcher mirroring diffusion's can_handle()/get_module() |
| Depends on  | P18-D7                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T16:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/arch/clip/__init__.py` — a CLIP architecture dispatch registry that mirrors `arch/diffusion/__init__.py`'s `can_handle()` / `get_module()` contract exactly, but dispatches on the `clip_type` **string** directly (not a loaded model object). This is the structural foundation that `LoadClip` (P18-D12) will call via `arch.clip.get_module(clip_type)` to select the correct loader module. The task includes writing the dispatcher itself plus ≥2 tests against a temporary dummy module that proves `get_module()` and `can_handle()` iterate over sibling `.py` files correctly, then removing the dummy before commit.

## Scope

### In Scope
- Create `worker/nodes/arch/clip/` directory as a Python package (with `__init__.py`).
- Implement `can_handle(clip_type: str) -> bool` — iterates loaded clip arch modules, returns `True` if any `can_handle(clip_type)` returns `True`.
- Implement `get_module(clip_type: str) -> ModuleType | None` — iterates loaded clip arch modules, returns the first matching module or `None`.
- Auto-import sibling `.py` files via `pkgutil.iter_modules(__path__)` using the same idempotency pattern as `arch/diffusion/__init__.py`.
- Create `worker/tests/test_arch_clip_init.py` with ≥2 tests.
- Create a temporary dummy clip module (e.g., `worker/nodes/arch/clip/_test_dummy.py`) for testing, remove it before commit.

### Out of Scope
- Real clip arch modules (`qwen3.py`, `clip_l.py`, `t5.py`) — handled by P18-D9, P18-D10, P18-D11 respectively. These are named in the downstream tasks' `defers_to: ["P18-D8"]` entries, confirming they depend on this dispatcher.
- Updating `LoadClip` to use `arch.clip.get_module()` — handled by P18-D12.
- Modifying `arch/__init__.py` re-export shim — no change needed; `arch.clip` is a separate subpackage imported independently.

## Existing Codebase Assessment

**What exists:** `arch/diffusion/__init__.py` (144 lines) is the established pattern for architecture dispatch. It uses `pkgutil.iter_modules(__path__)` to scan sibling `.py` files, `importlib.import_module()` to load each one, and exposes `can_handle(model_obj)` and `get_module(model_obj)` functions. The diffusion dispatch iterates modules on every call (no caching), which is acceptable because the set of loaded arch modules is small (currently just `zit.py`). The `_ensure_imported()` function provides idempotent auto-import at module load time. Error handling catches `ImportError` (logged as warning) and generic `Exception` from `can_handle()` (silently skipped).

**Established patterns:**
- Google-style docstrings with `Args:`, `Returns:`, and `.. versionadded::` directive.
- `from __future__ import annotations` at the top of every module.
- `logging.getLogger(__name__)` for module-level logging.
- `ModuleType` from `types` for return type annotations.
- `Any` from `typing` for the model object parameter in diffusion dispatch.
- Tests use `_make_model()` helper functions, Google-style docstrings with `Preconditions:`, `Tests:`, `Expected output:` sections.
- `conftest.py` provides an `autouse=True` fixture that sets `ANVILML_WORKER_MOCK=1`.

**Gap between design doc and source:** The design doc (§10.4a) specifies the clip dispatch interface, but the `arch/clip/` directory does not exist yet. The diffusion dispatch uses `Any` for the model object parameter; the clip dispatch will use `str` instead. The diffusion dispatch iterates modules on every `get_module()` call (no caching); the same pattern should be replicated for clip dispatch for consistency.

## Resolved Dependencies

None. This task uses only Python standard library modules: `importlib`, `pkgutil`, `logging`, `types.ModuleType`. No external crates or packages are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| stdlib | importlib | Python 3.12 built-in | N/A | n/a |
| stdlib | pkgutil | Python 3.12 built-in | N/A | n/a |
| stdlib | logging | Python 3.12 built-in | N/A | n/a |
| stdlib | types.ModuleType | Python 3.12 built-in | N/A | n/a |

## Approach

1. **Create the `arch/clip/` package directory** with an `__init__.py` file. The directory path is `worker/nodes/arch/clip/`. This creates the new Python package alongside the existing `arch/diffusion/` package.

2. **Write `worker/nodes/arch/clip/__init__.py`** mirroring `arch/diffusion/__init__.py`'s structure with these differences:
   - The module docstring describes CLIP architecture dispatch (not diffusion).
   - `_ensure_imported()` scans `__path__` for sibling `.py` files and imports each via `importlib.import_module(f"worker.nodes.arch.clip.{mod_info.name}")`. Same idempotency guard (`_imported` flag), same `pkgutil.iter_modules(__path__)` scan, same `ImportError` catch-and-log pattern.
   - `_ensure_imported()` is called at module load time (line 83 of the diffusion version).
   - `get_module(clip_type: str) -> ModuleType | None` — iterates modules via `pkgutil.iter_modules(__path__)`, skips packages (`mod_info.ispkg`), imports each, looks for `can_handle` attribute, calls `handler(clip_type)` (passing the string directly), returns the matching module or `None`. Same exception handling as diffusion (`except Exception: continue`).
   - `can_handle(clip_type: str) -> bool` — delegates to `get_module(clip_type) is not None`, avoiding duplication of iteration logic.
   - `__all__ = ["can_handle", "get_module"]` exported.
   - Same Google-style docstrings with `Args:`, `Returns:` sections.
   - Same `.. versionadded:: 0.1.0` directive.

3. **Create a temporary dummy clip module** for testing: `worker/nodes/arch/clip/_test_dummy.py`. This module exposes `can_handle(clip_type: str) -> bool` that returns `True` when `clip_type == "_dummy"`. This allows the tests to verify that `get_module()` actually iterates over sibling modules and finds a match. The module name is prefixed with `_` to follow Python convention for test-only internal modules, and it will be deleted before commit.

4. **Write `worker/tests/test_arch_clip_init.py`** with ≥2 tests:
   - `test_get_module_returns_dummy_for_dummy_clip_type`: Create a dummy clip type string (`"_dummy"`), call `get_module("_dummy")`, assert the returned module's `__name__` is `"worker.nodes.arch.clip._test_dummy"`. This proves `get_module()` iterates modules, imports them, calls `can_handle()`, and returns the correct module.
   - `test_get_module_returns_none_for_unknown_clip_type`: Call `get_module("nonexistent")`, assert `None` is returned. This proves the function correctly returns `None` when no module matches.
   - `test_can_handle_returns_bools_correctly`: Call `can_handle("_dummy")` (expect `True`) and `can_handle("nonexistent")` (expect `False`). This proves the delegation to `get_module()` works.
   - Each test follows the existing pattern: Google-style docstring with `Preconditions:`, `Tests:`, `Expected output:` sections.

5. **Remove the dummy module** (`worker/nodes/arch/clip/_test_dummy.py`) before committing. The tests pass because the dummy module is present during test execution and removed afterward. The dispatcher itself is fully functional without the dummy — it simply returns `None` for any clip type until a real clip arch module (P18-D9/10/11) is added.

## Public API Surface

```
# worker/nodes/arch/clip/__init__.py

def can_handle(clip_type: str) -> bool:
    """Check whether any loaded clip architecture module handles the given clip_type.

    Args:
        clip_type: The clip type string (e.g. "qwen3", "clip_l", "t5").

    Returns:
        True if at least one loaded clip arch module's can_handle()
        returns True for the clip_type; False otherwise.
    """

def get_module(clip_type: str) -> ModuleType | None:
    """Return the first loaded clip arch module whose can_handle() matches.

    Args:
        clip_type: The clip type string to match against.

    Returns:
        The matching architecture module, or None if no
        loaded clip arch module claims this clip_type.
    """
```

No new types are introduced. The return type `ModuleType` is from `types.ModuleType`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/clip/__init__.py` | CLIP architecture dispatch registry with `can_handle()` and `get_module()` |
| CREATE | `worker/tests/test_arch_clip_init.py` | Unit tests for the clip dispatcher |
| CREATE | `worker/nodes/arch/clip/_test_dummy.py` | Temporary dummy module for testing (removed before commit) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_clip_init.py` | `test_get_module_returns_dummy_for_dummy_clip_type` | `get_module("_dummy")` returns the `_test_dummy` module, proving iteration and matching works | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_init.py::test_get_module_returns_dummy_for_dummy_clip_type -v` exits 0 |
| `worker/tests/test_arch_clip_init.py` | `test_get_module_returns_none_for_unknown_clip_type` | `get_module("nonexistent")` returns `None`, proving no-match case works | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_init.py::test_get_module_returns_none_for_unknown_clip_type -v` exits 0 |
| `worker/tests/test_arch_clip_init.py` | `test_can_handle_returns_bools_correctly` | `can_handle("_dummy")` returns `True`, `can_handle("nonexistent")` returns `False` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_init.py::test_can_handle_returns_bools_correctly -v` exits 0 |

Acceptance command (all tests): `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_init.py -v` exits 0.

## CI Impact

No CI changes required. The new test file is under `worker/tests/` which is already covered by the `worker-linux` and `worker-windows` CI jobs (`ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v`). The new Python source file is covered by the `py_compile` step in those same CI jobs. No new file types, gates, or test modules are introduced that would require CI configuration changes.

## Platform Considerations

None identified. The dispatcher uses only Python stdlib (`importlib`, `pkgutil`, `logging`, `types`), which are platform-neutral. Path handling for `__path__` is done by `pkgutil` internally, which abstracts away platform-specific path separators. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The temporary dummy module (`_test_dummy.py`) may be accidentally committed if the removal step is forgotten. | Medium | High | The dummy module is named with a `_test_` prefix to make it visually obvious as a test artifact. The ACT agent's task description explicitly calls out the removal step. The plan's "Out of Scope" section confirms the dummy is temporary. |
| Importing the dummy module during tests may leave it cached in `sys.modules`, causing subsequent test runs to find stale state. | Low | Medium | The test imports the module naturally via `pkgutil`/`importlib` (which caches in `sys.modules`). Since the dummy is present at test time and tests only check return values (not side effects), stale cache is harmless. After removal, the dispatcher correctly returns `None` for all clip types. |
| The clip dispatch module's `__path__` may not resolve correctly if the package is imported from a non-standard location. | Low | Low | `pkgutil.iter_modules(__path__)` uses the package's own `__path__` attribute, which Python sets automatically for packages. This is the same pattern used successfully in `arch/diffusion/__init__.py`. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/clip/__init__.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_arch_clip_init.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_clip_init.py -v` exits 0 with ≥ 3 tests
- [ ] `worker/nodes/arch/clip/_test_dummy.py` does not exist after implementation (dummy removed)
- [ ] `ls worker/nodes/arch/clip/` contains only `__init__.py` (and `__pycache__/`)
