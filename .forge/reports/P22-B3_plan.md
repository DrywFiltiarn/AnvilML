# Plan Report: P22-B3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P22-B3                                      |
| Phase       | 22 — Qwen3 CLIP Arch Module                 |
| Description | worker/nodes/arch/clip/qwen3.py: can_handle() + dispatch registration |
| Depends on  | P22-B2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-15T12:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Wire the Qwen3 CLIP arch module into the dispatcher that Phase 10 (P10-B2) built. Add a `can_handle(key) -> bool` function to `qwen3.py` that matches on the `"qwen3"` dispatch key string, and register the module into `arch/clip/__init__.py`'s `_REGISTERED_MODULES` list so that `get_module("qwen3")` now returns the qwen3 module instead of `None`. This gives the clip dispatcher its first real entry, transitioning from the zero-module stub state.

## Scope

### In Scope
- Add `can_handle(key: str) -> bool` function to `worker/nodes/arch/clip/qwen3.py` that returns `True` when `key == ARCH` (i.e., `"qwen3"`), `False` otherwise.
- Register `qwen3` module into `worker/nodes/arch/clip/__init__.py`'s `_REGISTERED_MODULES` list via an import and `.append()` call, following the exact pattern used in `arch/diffusion/__init__.py`.
- Add tests to `worker/tests/test_arch_clip_qwen3.py`:
  - `test_can_handle_matches_qwen3` — `can_handle("qwen3")` returns `True`.
  - `test_can_handle_rejects_other_keys` — `can_handle("zit")`, `can_handle("flux2klein")`, and `can_handle("unknown")` all return `False`.
  - `test_get_module_returns_qwen3_for_matching_key` — `clip.get_module("qwen3")` returns the qwen3 module.
- Update `docs/TESTS.md` with entries for all new tests (per FORGE_AGENT_RULES §5.10).

### Out of Scope
None. This task has `defers_to: []` (empty) — no scope is deferred. No stubs, no stubs masquerading as "verified at ACT time." The full `can_handle` implementation and dispatch registration are delivered here.

## Existing Codebase Assessment

**What already exists:**
- `qwen3.py` (303 lines) already implements `_infer_hyperparams()`, `_infer_hyperparams_inner()`, and `_safetensors_dtype_to_canonical()`. It defines `ARCH: str = "qwen3"` at module level (line 41). The module imports `logging`, `re`, `Any`, and `safe_open` from safetensors — no torch dependency.
- `arch/clip/__init__.py` (48 lines) implements `get_module(key)` which iterates `_REGISTERED_MODULES` calling `module.can_handle(key)` on each. Currently `_REGISTERED_MODULES` is an empty list `[]`. No modules are registered yet.
- `arch/diffusion/__init__.py` (49 lines) is the reference pattern: it imports `from worker.nodes.arch.diffusion import zit` and then calls `_REGISTERED_MODULES.append(zit)` at module level.
- `arch/diffusion/zit.py` (line 755) implements `can_handle(key: str) -> bool` as `return key == ARCH`.
- `test_arch_dispatch.py` (200 lines) tests the dispatcher for diffusion, clip, and vae families with mock doubles. The clip section tests `get_module` with empty registry and a fake module returning `False`.

**Established patterns:**
- `can_handle(key: str) -> bool` is a bare function at module level (not a method on a class), following the §10.4 fixed-method-name contract.
- Registration in `__init__.py` is done via import + `.append()` at module level, not via a function call.
- Tests for `can_handle` in the qwen3 test file use `pytest.raises` for error cases and direct assertions for boolean results.
- The existing `test_arch_clip_qwen3.py` has 3 tests: `test_infer_hyperparams_qwen3_fixture`, `test_infer_hyperparams_nonexistent_path_raises`, `test_infer_hyperparams_truncated_header_raises`.

**Gap between design doc and current source:**
- The design doc (§10.4) specifies `can_handle(key: Any) -> bool` — the parameter type is `Any`, not `str`. However, zit.py uses `key: str`. Both work since `str` is a subtype of `Any`. We'll match zit.py's `key: str` for consistency within the codebase.
- `can_handle()` does not have dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`). Per §10.6, these markers apply only to `load()`, `sample()`, `decode()`, and node `execute()`. `can_handle()` is a pure string comparison and is not in scope for the marker convention. This is correct — no markers needed.

## Resolved Dependencies

None. This task introduces no new external dependencies. It only adds a pure-Python function and a module-level import. All imports (`logging`, `types.ModuleType`) are from the Python standard library.

## Approach

1. **Add `can_handle()` to `qwen3.py`.** Append the following function to the end of `worker/nodes/arch/clip/qwen3.py` (after `_safetensors_dtype_to_canonical()`):

   ```python
   def can_handle(key: str) -> bool:
       """Confirm this module handles the given dispatch key.

       The dispatcher passes the ``clip_type`` string as *key*. This
       function returns ``True`` only when the key matches this module's
       canonical architecture identifier.

       Args:
           key: The clip_type string to check, e.g. ``"qwen3"``.

       Returns:
           ``True`` if *key* equals ``"qwen3"``, ``False`` otherwise.
       """
       return key == ARCH
   ```

   This follows the exact same pattern as `zit.py`'s `can_handle()` (line 755-769). The function is a pure, deterministic string comparison — no I/O, no torch dependency, importable in mock-mode without torch.

2. **Register qwen3 in `arch/clip/__init__.py`.** Modify the clip dispatcher to import and register qwen3, following the exact pattern from `arch/diffusion/__init__.py`:

   After the `_REGISTERED_MODULES: list[ModuleType] = []` line (line 21), add:
   ```python
   from worker.nodes.arch.clip import qwen3

   _REGISTERED_MODULES.append(qwen3)
   ```

   This is a module-level side effect executed at import time, identical to how `diffusion/__init__.py` registers zit.

3. **Add tests to `test_arch_clip_qwen3.py`.** Append three new test functions:

   a. `test_can_handle_matches_qwen3()` — imports `can_handle` from `qwen3`, calls `can_handle("qwen3")`, asserts `True`.

   b. `test_can_handle_rejects_other_keys()` — imports `can_handle`, calls it with `"zit"`, `"flux2klein"`, `"unknown"`, asserts all return `False`.

   c. `test_get_module_returns_qwen3_for_matching_key()` — imports `clip` dispatcher and `qwen3`, calls `clip.get_module("qwen3")`, asserts the returned module is the same as `qwen3` (identity check with `is`).

   These tests follow the style of existing tests in the file (Google-style docstrings, direct assertions, no fixtures needed).

4. **Update `docs/TESTS.md`.** Add entries for the three new tests using the format defined in ANVILML_DESIGN.md §17.1, with `Mode: mock` for all three (since `can_handle` and `get_module` with a registered module don't require torch).

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `can_handle` | `worker/nodes/arch/clip/qwen3.py` | `def can_handle(key: str) -> bool` |

No new types, no new module-level constants, no changes to existing pub items. The `ARCH` constant already exists at line 41 of qwen3.py.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/clip/qwen3.py` | Add `can_handle(key: str) -> bool` function at end of file |
| MODIFY | `worker/nodes/arch/clip/__init__.py` | Add import of `qwen3` and register it in `_REGISTERED_MODULES` |
| MODIFY | `worker/tests/test_arch_clip_qwen3.py` | Add 3 new test functions for can_handle and get_module dispatch |
| MODIFY | `docs/TESTS.md` | Add test catalogue entries for the 3 new tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_clip_qwen3.py` | `test_can_handle_matches_qwen3` | `can_handle("qwen3")` returns `True` | qwen3 module importable | `"qwen3"` | `True` | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_matches_qwen3 -v` |
| `worker/tests/test_arch_clip_qwen3.py` | `test_can_handle_rejects_other_keys` | `can_handle("zit")`, `can_handle("flux2klein")`, `can_handle("unknown")` all return `False` | qwen3 module importable | `"zit"`, `"flux2klein"`, `"unknown"` | All `False` | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_can_handle_rejects_other_keys -v` |
| `worker/tests/test_arch_clip_qwen3.py` | `test_get_module_returns_qwen3_for_matching_key` | `clip.get_module("qwen3")` returns the qwen3 module (identity match) | clip dispatcher importable with qwen3 registered | `"qwen3"` | `result is qwen3` (same object) | `python -m pytest worker/tests/test_arch_clip_qwen3.py::test_get_module_returns_qwen3_for_matching_key -v` |

Total test count in file after this task: 6 (3 existing + 3 new).

## CI Impact

No CI job changes required. The new tests are mock-compatible (no torch imports at module level, no torch usage in test bodies). They will be collected and run by both `worker-linux-mock` and `worker-linux-real` CI jobs. No new file types, no new gate triggers.

## Platform Considerations

None identified. The `can_handle()` function is a pure string comparison — no platform-specific code, no path handling, no line-ending concerns. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Importing qwen3 in `clip/__init__.py` could cause a circular import if qwen3 imports from clip at module level. | Low | High | qwen3.py currently has no imports from the `clip` package — it only imports from `logging`, `re`, `typing`, and `safetensors`. The import direction is one-way (clip → qwen3), matching the diffusion pattern which works without issues. |
| `get_module("qwen3")` returns `None` if the registration in `__init__.py` doesn't execute before the test calls `get_module`. | Low | Medium | The registration happens at module-level import time. Tests that import `clip` will get the registered module. This is the same pattern used by diffusion/__init__.py with zit, which is proven working. |
| Test count claim (≥6 total) fails because existing tests are removed or renamed. | Low | Medium | The existing 3 tests (`test_infer_hyperparams_qwen3_fixture`, `test_infer_hyperparams_nonexistent_path_raises`, `test_infer_hyperparams_truncated_header_raises`) are untouched. Only 3 new tests are added. |

## Acceptance Criteria

- [ ] `python -c "from worker.nodes.arch.clip.qwen3 import can_handle; assert can_handle('qwen3') == True"` exits 0
- [ ] `python -c "from worker.nodes.arch.clip.qwen3 import can_handle; assert can_handle('zit') == False; assert can_handle('unknown') == False"` exits 0
- [ ] `python -c "from worker.nodes.arch.clip import get_module, qwen3; assert get_module('qwen3') is qwen3"` exits 0
- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py -v` exits 0 with ≥6 tests collected
- [ ] `python -m pytest worker/tests/test_arch_clip_qwen3.py --collect-only -q | tail -1` shows exactly 6 tests
