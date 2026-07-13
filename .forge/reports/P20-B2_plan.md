# Plan Report: P20-B2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P20-B2                                            |
| Phase       | 20 — ZiT Diffusion Arch Module: Shape Inference & Construction |
| Description | worker/nodes/arch/diffusion/zit.py: can_handle() + dispatch registration |
| Depends on  | P20-B1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-13T15:35:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add `can_handle(key) -> bool` to `worker/nodes/arch/diffusion/zit.py` so the dispatcher built in Phase 10 can route diffusion architecture lookups to the ZiT module. Register `zit.py` as the first real entry in `arch/diffusion/__init__.py`'s `_REGISTERED_MODULES` list, making `get_module("zit")` return the zit module. This connects the shape-inference work from P20-B1 to the dispatch mechanism.

## Scope

### In Scope
- Implement `can_handle(key: str) -> bool` in `worker/nodes/arch/diffusion/zit.py` that compares `key` against the architecture string `"zit"`.
- Register `zit.py` into `worker/nodes/arch/diffusion/__init__.py`'s `_REGISTERED_MODULES` list via an `import zit` and `append(zit)`.
- Add 3 new tests in `worker/tests/test_arch_zit.py`: `test_can_handle_matches_zit`, `test_can_handle_rejects_unrelated_key`, and `test_get_module_returns_zit_for_matching_key`.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferrals. `load()`, `sample()`, and `compute_latent_shape()` remain unimplemented per the task's stated scope.

## Existing Codebase Assessment

**What exists:** P20-B1 already created `worker/nodes/arch/diffusion/zit.py` with `_infer_hyperparams()` and `_infer_hyperparams_inner()`, which reads safetensors headers and returns a dict containing `"arch": "zit"`. The docstring in `zit.py` explicitly marks `can_handle()` as "deferred to P20-B2." The `arch/diffusion/__init__.py` dispatcher exists with an empty `_REGISTERED_MODULES` list and a `get_module(key)` that iterates registered modules calling their `can_handle(key)`. Four tests for `_infer_hyperparams()` exist in `test_arch_zit.py`. The `test_arch_dispatch.py` file already tests the dispatcher's behaviour with mock modules (empty registry, can_handle=False skip) across all three arch families (diffusion, clip, vae).

**Established patterns:** Python worker code uses Google-style docstrings with Args/Returns/Raises sections. Module-level constants use UPPER_SNAKE_CASE. Test files follow the convention `test_<module_name>.py`. Tests use `pytest.raises` for exception assertions. The dispatcher pattern is: each arch module defines `can_handle(key)`; `__init__.py` imports the module and appends it to `_REGISTERED_MODULES`; `get_module(key)` scans the list.

**Gap between design doc and source:** The design doc §11.3's four-step contract lists `can_handle()` as step 2, and `zit.py`'s own docstring already references this. No gap — the source is ready for step 2.

## Resolved Dependencies

None. This task uses only Python standard library types (`str`, `bool`) and existing project modules (`worker.nodes.arch.diffusion.zit`, `worker.nodes.arch.diffusion`). No new external packages are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | — | — | — | — |

## Approach

1. **Implement `can_handle(key: str) -> bool` in `worker/nodes/arch/diffusion/zit.py`.**
   - Add a module-level constant `ARCH = "zit"` at the top of the file (after the imports, before the first function). This is the canonical architecture identifier that `can_handle` compares against. It mirrors the `"arch": "zit"` value that `_infer_hyperparams()` returns when it reads metadata or falls back to key-pattern inference.
   - Implement `can_handle(key)` as a simple equality check: `return key == ARCH`. This is the correct pattern — the dispatcher passes the `arch` string (from safetensors metadata or path fallback) as the key, and the module confirms it handles that architecture.
   - Add a Google-style docstring with Args and Returns sections.
   - Rationale: The `key` passed to `can_handle` by the dispatcher is already the architecture string (e.g. `"zit"`). A direct string comparison is O(1), deterministic, and requires no file I/O — `can_handle` must be cheap because it is called during dispatch lookup.

2. **Register `zit.py` in `worker/nodes/arch/diffusion/__init__.py`.**
   - Add `import zit` after the existing type imports (`from typing import Any`, `from types import ModuleType`).
   - Append `zit` to `_REGISTERED_MODULES`: `_REGISTERED_MODULES.append(zit)`.
   - Rationale: This is the first real entry in the list. The dispatcher was correctly empty since Phase 10; this is the expected moment for the first concrete registration.

3. **Add 3 new tests in `worker/tests/test_arch_zit.py`.**
   - `test_can_handle_matches_zit`: Import `can_handle` from `zit`, call `can_handle("zit")`, assert `True`. This verifies the primary match path.
   - `test_can_handle_rejects_unrelated_key`: Import `can_handle`, call `can_handle("flux2klein")`, assert `False`. This verifies the rejection path for an unrelated architecture string.
   - `test_get_module_returns_zit_for_matching_key`: Import `get_module` from `diffusion`, call `get_module("zit")`, assert the result is not `None` and is the `zit` module. This verifies the end-to-end dispatch: the registered zit module's `can_handle("zit")` returns True, so `get_module` finds it.
   - Rationale: These three tests cover the acceptance criteria exactly — match, rejection, and dispatcher integration. The total test count becomes 7 (4 existing + 3 new).

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `ARCH` | `worker/nodes/arch/diffusion/zit.py` | `ARCH: str = "zit"` (module-level constant) |
| `can_handle(key)` | `worker/nodes/arch/diffusion/zit.py` | `def can_handle(key: str) -> bool` |

No changes to any existing public item. No new re-exports.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `ARCH = "zit"` constant and `can_handle(key) -> bool` function |
| MODIFY | `worker/nodes/arch/diffusion/__init__.py` | Import `zit` and append to `_REGISTERED_MODULES` |
| MODIFY | `worker/tests/test_arch_zit.py` | Add 3 new tests: `test_can_handle_matches_zit`, `test_can_handle_rejects_unrelated_key`, `test_get_module_returns_zit_for_matching_key` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_can_handle_matches_zit` | `can_handle("zit")` returns `True` — the primary match path for the ZiT architecture string | zit.py has `can_handle()` implemented | `"zit"` | `True` | `python -m pytest worker/tests/test_arch_zit.py::test_can_handle_matches_zit -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_can_handle_rejects_unrelated_key` | `can_handle("flux2klein")` returns `False` — the module correctly rejects unrelated architecture keys | zit.py has `can_handle()` implemented | `"flux2klein"` | `False` | `python -m pytest worker/tests/test_arch_zit.py::test_can_handle_rejects_unrelated_key -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_get_module_returns_zit_for_matching_key` | `get_module("zit")` returns the `zit` module (not `None`) — end-to-end dispatch through the registered module | zit.py is imported and appended to `_REGISTERED_MODULES` in `__init__.py` | `"zit"` | `ModuleType` instance (the `zit` module) | `python -m pytest worker/tests/test_arch_zit.py::test_get_module_returns_zit_for_matching_key -v` exits 0 |

Total test count: 7 (4 existing from P20-B1 + 3 new).

## CI Impact

No CI changes required. The task only adds Python source code and tests. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) already run `pytest worker/tests/` which picks up all test files. No new file types, gates, or configurations are introduced.

## Platform Considerations

None identified. The `can_handle()` function is a pure Python string comparison with no platform-specific behaviour. The registration in `__init__.py` uses standard Python imports and list operations. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `get_module("zit")` returns the module but the test assertion `result is zit` fails because `import zit` in `__init__.py` creates a different module object than the one imported in the test. | Low | Medium | Use `result is not None` and `result.__name__ == "zit"` instead of identity comparison, or import `zit` inside the test and compare `result is zit`. The test_arch_dispatch.py pattern uses `Mock` objects where identity comparison works, but for real modules the safest approach is `result.__name__ == "zit"`. |
| Importing `zit` in `__init__.py` triggers side effects during dispatcher import (e.g. `_infer_hyperparams` being called at module level). | Low | High | The current `zit.py` has no module-level side effects — only function and constant definitions. The `ARCH = "zit"` constant is a literal string assignment, which is safe. No risk of premature execution. |
| Test isolation: `_REGISTERED_MODULES` is a module-level list that persists across test runs. If a prior test appends a module and doesn't clean up, subsequent tests see stale state. | Low | Medium | The `test_get_module_returns_zit_for_matching_key` test imports `zit` directly (no prior test modifies `_REGISTERED_MODULES`), so the list starts empty and only gets `zit` appended by `__init__.py`. No cleanup is needed. The existing `test_arch_dispatch.py` tests that modify `_REGISTERED_MODULES` use try/finally cleanup, but those are separate tests that don't interact with this new test. |

## Acceptance Criteria

- [ ] `python -m pytest worker/tests/test_arch_zit.py -v --collect-only | grep "test session starts" && python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with >=7 tests collected
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_can_handle_matches_zit -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_can_handle_rejects_unrelated_key -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_zit.py::test_get_module_returns_zit_for_matching_key -v` exits 0
- [ ] `grep -n "def can_handle" worker/nodes/arch/diffusion/zit.py` returns a match (function exists)
- [ ] `grep -n "import zit" worker/nodes/arch/diffusion/__init__.py` returns a match (module is imported)
- [ ] `grep -n "_REGISTERED_MODULES.append(zit)" worker/nodes/arch/diffusion/__init__.py` returns a match (module is registered)
