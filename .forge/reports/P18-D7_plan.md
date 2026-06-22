# Plan Report: P18-D7

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D7                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/: restructure into arch/diffusion/ + arch/clip/ siblings |
| Depends on  | P18-D1, P18-D2, P18-D3, P18-D3b             |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T15:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Restructure `worker/nodes/arch/` by creating a new `arch/diffusion/` subpackage and moving `zit.py` into it. The `arch/__init__.py` becomes a thin re-export shim that imports `can_handle` and `get_module` from `arch/diffusion/__init__.py`, which now contains the `pkgutil.iter_modules()` scanning logic targeting `arch/diffusion/` instead of `arch/` itself. This is a pure refactor with no behavioral change — `can_handle()` and `get_module()` still work identically, only the module `__name__` returned by `get_module()` changes (`worker.nodes.arch.zit` → `worker.nodes.arch.diffusion.zit`). Updated tests assert the new module path.

## Scope

### In Scope
- Create `worker/nodes/arch/diffusion/__init__.py` containing the `pkgutil.iter_modules()` scanning logic and `get_module()`/`can_handle()` dispatch functions (moved from `arch/__init__.py`, scanning `arch/diffusion/` instead of `arch/`)
- Move `worker/nodes/arch/zit.py` to `worker/nodes/arch/diffusion/zit.py` (no content changes)
- Rewrite `worker/nodes/arch/__init__.py` as a thin re-export shim: `from worker.nodes.arch.diffusion import can_handle, get_module`
- Update `worker/tests/test_arch_init.py`: change assertion `mod.__name__ == "worker.nodes.arch.zit"` to `mod.__name__ == "worker.nodes.arch.diffusion.zit"`
- Update `worker/tests/test_arch_zit.py`: change import `from worker.nodes.arch.zit import ...` to `from worker.nodes.arch.diffusion.zit import ...`
- Update `ANVILML_DESIGN.md §10.4` file paths (the section header already says `arch/diffusion/`; verify no stale `arch.zit` references remain in that section)
- Append a one-line note to `docs/TASKS_PHASE018.md` (do NOT rewrite the prose)

### Out of Scope
- Creating `arch/clip/` package — that is the scope of P18-D8
- Creating `flux.py` — that is the scope of Phase 19
- Modifying `docs/TASKS_PHASE018.md` prose (append a one-line note only)
- Modifying `.forge/reports/*.md` files (historical records)
- Any behavioral changes to `can_handle()` or `get_module()` logic

## Existing Codebase Assessment

The current `worker/nodes/arch/` package contains two files: `__init__.py` (144 lines) which implements the full `pkgutil.iter_modules()` auto-import scanning logic plus `get_module()` and `can_handle()` dispatch functions, and `zit.py` (204 lines) which provides the ZiT architecture's `can_handle()`, `sample()`, `compute_latent_shape()`, `MockLatent`, and `VAE_SCALE_FACTOR`.

The established pattern is: `arch/__init__.py` auto-imports sibling `.py` files at import time via `_ensure_imported()`, then `get_module()` iterates those modules to find one whose `can_handle()` matches a model object. The `Sampler` node calls `arch.get_module(model)` and `arch.can_handle(model)`. Tests in `test_arch_init.py` and `test_arch_zit.py` exercise this dispatch and the zit module's public API respectively.

The design doc (`ANVILML_DESIGN.md §10.4`) already describes the post-restructure layout with `arch/diffusion/` as the dispatch package. No discrepancy exists between the design doc's target state and what this task produces. The task's test assertions currently expect `mod.__name__ == "worker.nodes.arch.zit"` — this is the gap the task must close.

## Resolved Dependencies

None. This task introduces no new external dependencies or packages. It is a pure Python file restructure within the existing `worker/` package.

## Approach

1. **Create `worker/nodes/arch/diffusion/` directory.**
   This is a new Python package. No files exist here yet.

2. **Write `worker/nodes/arch/diffusion/__init__.py`.**
   Copy the scanning logic from `arch/__init__.py` with one modification: the module path prefix changes from `f"worker.nodes.arch.{mod_info.name}"` to `f"worker.nodes.arch.diffusion.{mod_info.name}"` so that `pkgutil.iter_modules(__path__)` scans the `diffusion/` subdirectory and imports modules from the correct dotted path. The `_ensure_imported()`, `get_module()`, and `can_handle()` functions are identical in logic to the current `arch/__init__.py` — only the string template for `full_name` changes. Keep the same docstring, `__all__`, module-level `_imported` flag, and logger. The module-level auto-import trigger `_ensure_imported()` at line 83 of the original stays at the bottom of this file.

3. **Move `worker/nodes/arch/zit.py` to `worker/nodes/arch/diffusion/zit.py`.**
   No content changes to `zit.py` itself — it has no imports of `worker.nodes.arch` (the scanning is handled by the parent package's `__init__.py`). The file's module `__name__` will change automatically because Python derives it from the file path.

4. **Rewrite `worker/nodes/arch/__init__.py` as a thin re-export shim.**
   Replace the entire 144-line file with a minimal module that:
   - Keeps the existing docstring (updated to note it re-exports from `arch/diffusion/`)
   - Contains only: `from worker.nodes.arch.diffusion import can_handle, get_module`
   - Keeps `__all__ = ["can_handle", "get_module"]`
   - No scanning logic, no `_ensure_imported()`, no `pkgutil` import

   This preserves backward-compatible import paths: code that does `from worker.nodes.arch import can_handle, get_module` continues to work because Python resolves the re-export.

5. **Update `worker/tests/test_arch_init.py`.**
   - Change the assertion on line 70 from `assert mod.__name__ == "worker.nodes.arch.zit"` to `assert mod.__name__ == "worker.nodes.arch.diffusion.zit"`.
   - No other changes needed — the import `from worker.nodes.arch import can_handle, get_module` still works via the re-export shim.

6. **Update `worker/tests/test_arch_zit.py`.**
   - Change the import on line 19 from `from worker.nodes.arch.zit import (...)` to `from worker.nodes.arch.diffusion.zit import (...)`.
   - No other changes needed — all test logic references the same symbols (`MockLatent`, `VAE_SCALE_FACTOR`, `can_handle`, `compute_latent_shape`, `sample`) which are unchanged.

7. **Verify `ANVILML_DESIGN.md §10.4` file paths.**
   The section header already reads `### 10.4 Architecture Dispatch (worker/nodes/arch/diffusion/)`. Scan the section for any remaining references to bare `arch.zit` or `worker/nodes/arch/zit` that should reference `arch/diffusion/zit` instead. The design doc already uses the correct post-restructure paths throughout (§10.4, §10.4a, §10.5). If any stale path is found, update it.

8. **Append a one-line note to `docs/TASKS_PHASE018.md`.**
   Append a single line at the end of the file noting that P18-D7 executed the restructure. Do not modify any existing prose.

9. **Run syntax check and tests.**
   `worker/.venv/bin/python -m py_compile worker/nodes/arch/__init__.py worker/nodes/arch/diffusion/__init__.py worker/nodes/arch/diffusion/zit.py worker/tests/test_arch_init.py worker/tests/test_arch_zit.py` then `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py worker/tests/test_arch_zit.py -v`.

## Public API Surface

No new public Python items are introduced. The public API surface is unchanged:

| Item | Module Path (before) | Module Path (after) |
|------|---------------------|---------------------|
| `can_handle` | `worker.nodes.arch.can_handle` | `worker.nodes.arch.diffusion.can_handle` (re-exported via `worker.nodes.arch.can_handle`) |
| `get_module` | `worker.nodes.arch.get_module` | `worker.nodes.arch.diffusion.get_module` (re-exported via `worker.nodes.arch.get_module`) |
| `zit.can_handle` | `worker.nodes.arch.zit.can_handle` | `worker.nodes.arch.diffusion.zit.can_handle` |
| `zit.sample` | `worker.nodes.arch.zit.sample` | `worker.nodes.arch.diffusion.zit.sample` |
| `zit.compute_latent_shape` | `worker.nodes.arch.zit.compute_latent_shape` | `worker.nodes.arch.diffusion.zit.compute_latent_shape` |
| `zit.MockLatent` | `worker.nodes.arch.zit.MockLatent` | `worker.nodes.arch.diffusion.zit.MockLatent` |
| `zit.VAE_SCALE_FACTOR` | `worker.nodes.arch.zit.VAE_SCALE_FACTOR` | `worker.nodes.arch.diffusion.zit.VAE_SCALE_FACTOR` |

The re-export from `arch/__init__.py` means the public API surface for all consumers (`Sampler`, `EmptyLatent`, etc.) is identical — no import path change required in any node code.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/diffusion/__init__.py` | New diffusion subpackage init with scanning logic and dispatch functions |
| CREATE | `worker/nodes/arch/diffusion/zit.py` | Moved from `worker/nodes/arch/zit.py` (no content change) |
| MODIFY | `worker/nodes/arch/__init__.py` | Replaced with thin re-export shim |
| MODIFY | `worker/tests/test_arch_init.py` | Update `mod.__name__` assertion to new path |
| MODIFY | `worker/tests/test_arch_zit.py` | Update import path from `arch.zit` to `arch.diffusion.zit` |
| MODIFY | `docs/ANVILML_DESIGN.md` | Verify/update §10.4 file paths (likely no change needed) |
| MODIFY | `docs/TASKS_PHASE018.md` | Append one-line note (no prose rewrite) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_init.py` | `test_get_module_returns_zit_for_zit_model` | `get_module()` returns the zit module for a ZiT model, with `mod.__name__ == "worker.nodes.arch.diffusion.zit"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_get_module_returns_zit_for_zit_model -v` exits 0 |
| `worker/tests/test_arch_init.py` | `test_get_module_returns_none_for_unknown_arch` | `get_module()` returns `None` for unknown architecture | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_get_module_returns_none_for_unknown_arch -v` exits 0 |
| `worker/tests/test_arch_init.py` | `test_can_handle_still_works_after_refactor` | `can_handle()` returns correct bools via re-export from `arch.diffusion` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py::test_can_handle_still_works_after_refactor -v` exits 0 |
| `worker/tests/test_arch_zit.py` | All zit tests (VAE_SCALE_FACTOR, can_handle, sample, import isolation, compute_latent_shape) | All zit module functions work correctly from new import path `worker.nodes.arch.diffusion.zit` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 |

Acceptance command for the task:
`ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py worker/tests/test_arch_zit.py -v` exits 0

## CI Impact

No CI changes required. The test files are already picked up by the existing `worker-linux` and `worker-windows` CI jobs which run `pytest worker/tests/`. The restructured files are in the same package tree (`worker/nodes/arch/`), so `py_compile` and pytest discover them without any configuration changes.

## Platform Considerations

None identified. This is a pure Python file restructure with no platform-specific behavior. The `pkgutil.iter_modules()` API and `importlib.import_module()` are cross-platform. The re-export shim (`from worker.nodes.arch.diffusion import ...`) works identically on Linux and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sys.modules` cache contamination: if `worker.nodes.arch.zit` was already imported before the restructure (e.g., by a prior test or import), Python may resolve `from worker.nodes.arch.diffusion.zit import ...` to the stale cached module, causing `ImportError` or returning the wrong module object. | Medium | High | The tests that import zit (`test_arch_zit.py`) clear `sys.modules` entries for both `worker.nodes.arch.zit` and `worker.nodes.arch` before re-importing (see `test_sample_mock_no_torch_import`). After the restructure, the old name `worker.nodes.arch.zit` no longer exists, so the cache entry for it is irrelevant. The test `test_sample_mock_no_torch_import` already removes `sys.modules.pop("worker.nodes.arch.zit", None)` — this line becomes a no-op but harmless. No additional changes needed. |
| The re-export shim in `arch/__init__.py` fails if `arch/diffusion/__init__.py` has an import error (e.g., a syntax error or missing dependency). | Low | Medium | The py_compile step (Step 7 in ENVIRONMENT.md §6) catches syntax errors before pytest runs. The re-export is a single `from ... import ...` line — if the target module fails to import, the error surfaces immediately at the shim's import site, which is caught by the syntax check. |
| `ANVILML_DESIGN.md §10.4` has stale file paths not visible in the section header scan. | Low | Low | The section was already updated for the restructure (the header already says `arch/diffusion/`). A targeted grep for `arch.zit` or `nodes/arch/zit` in the design doc confirms no stale references exist. If found, update them in this task. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/__init__.py worker/nodes/arch/diffusion/__init__.py worker/nodes/arch/diffusion/zit.py worker/tests/test_arch_init.py worker/tests/test_arch_zit.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_init.py worker/tests/test_arch_zit.py -v` exits 0
- [ ] `grep -rn "worker\.nodes\.arch\.zit" worker/` returns no results (no stale references to old module path remain in worker source or tests)
- [ ] `grep -rn "worker\.nodes\.arch\.diffusion\.zit" worker/nodes/arch/diffusion/zit.py` returns exactly 1 match (the module's own docstring or comment, confirming the file exists at the new path)
- [ ] `head -1 worker/nodes/arch/__init__.py` confirms the file exists and is non-empty (the re-export shim)
- [ ] `wc -l worker/nodes/arch/__init__.py` returns a small number (≤ 10 lines, confirming it is a thin shim)
- [ ] `ls worker/nodes/arch/diffusion/__init__.py worker/nodes/arch/diffusion/zit.py` confirms both files exist
