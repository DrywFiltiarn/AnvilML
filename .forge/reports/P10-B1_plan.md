# Plan Report: P10-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-B1                                      |
| Phase       | 10 — Generic Node Groundwork                |
| Description | worker/nodes/arch/diffusion/__init__.py: can_handle/get_module dispatch |
| Depends on  | P10-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T08:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the shared dispatch mechanism for the diffusion architecture family — a single
`get_module(key)` function in `worker/nodes/arch/diffusion/__init__.py` that scans a
module-level `_REGISTERED_MODULES` list for the first module whose `can_handle(key)`
returns `True`. With zero registered modules (concrete arch modules like `zit.py` are
out of scope), `get_module` must return `None` for any key without raising. This
establishes the dispatch pattern that P10-B2 will replicate for the clip and vae
families, satisfying ANVILML_DESIGN.md §10.4's rule that all three families share one
scan implementation shape rather than three independently-written loops.

## Scope

### In Scope
- Create `worker/nodes/arch/` directory (parent `__init__.py` is not required — arch
  packages are imported by their family dispatcher, not auto-imported at the nodes
  level).
- Create `worker/nodes/arch/diffusion/__init__.py` with:
  - `_REGISTERED_MODULES: list[ModuleType] = []` — module-level list (currently empty).
  - `get_module(key: Any) -> ModuleType | None` — scans `_REGISTERED_MODULES` for the
    first module whose `can_handle(key)` returns `True`; returns `None` if no match
    or the list is empty.
- Create `worker/tests/test_arch_dispatch.py` with ≥3 tests verifying:
  - `get_module` returns `None` when `_REGISTERED_MODULES` is empty, for any key type.
  - `get_module` does not raise for `str`, `None`, and arbitrary object keys.
  - The dispatch loop correctly skips a module whose `can_handle` returns `False`
    (using a test double).

### Out of Scope
- Concrete arch modules (`zit.py`, `flux2klein.py`) — not in scope for this phase.
- The clip and vae family dispatch packages — implemented in P10-B2.
- The node auto-import mechanism (`worker/nodes/__init__.py` wiring) — implemented in P10-C1.
- The `worker_main.py` `_import_nodes()` wiring — implemented in P10-D1.

## Existing Codebase Assessment

The `worker/nodes/` directory exists with two files: `__init__.py` (empty, 0 lines)
and `base.py` (88 lines) containing `NODE_REGISTRY`, `register()`, `SlotSpec`,
`NodeContext`, and `BaseNode`. The established Python patterns are:

- Google-style docstrings on every class and non-trivial function (see `base.py`).
- Type annotations using `from __future__ import annotations` at module top.
- Tests live in `worker/tests/` with one test file per source module, importing from
  the package's public interface (e.g. `from worker.nodes import base`).
- Test functions use descriptive snake_case names and docstrings explaining what they
  verify and the expected outcome (see `test_base.py`).

No `worker/nodes/arch/` directory exists yet — this task creates the entire arch
directory structure from scratch. No `test_arch_dispatch.py` exists yet either.

The design doc (§10.4) specifies that `get_module` uses `pkgutil.iter_modules()` for
auto-importing concrete modules, but at this phase `_REGISTERED_MODULES` is empty and
the auto-import loop is not yet wired (that is P10-C1's job). The `get_module` function
itself is a simple linear scan — no auto-import logic is part of this task.

## Resolved Dependencies

None. This task uses only Python 3.12 standard library types (`typing.Any`,
`typing.ModuleType`) and no external packages.

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| stdlib | typing  | 3.12            | n/a        | n/a                    |

## Approach

1. **Create the `worker/nodes/arch/` directory.**
   - No `__init__.py` is needed here — arch packages are imported by their family
     dispatcher (this file), not by the top-level node auto-import loop.
   - Command: `mkdir -p worker/nodes/arch/diffusion`

2. **Create `worker/nodes/arch/diffusion/__init__.py`** with the following content:
   - Module docstring describing the diffusion arch family dispatcher.
   - Import `Any` from `typing` and `ModuleType` from `types`.
   - Define `_REGISTERED_MODULES: list[ModuleType] = []` — an empty list that will be
     populated by concrete arch modules in later phases.
   - Define `get_module(key: Any) -> ModuleType | None`:
     - Iterate over `_REGISTERED_MODULES`.
     - For each module, call `module.can_handle(key)`.
     - Return the first module whose `can_handle(key)` returns `True`.
     - If no match is found (or the list is empty), return `None`.
     - No logging is needed at this stage — `get_module` is a pure function called
       from node execute paths, and logging would be added when concrete modules
       are wired in later phases.
   - No dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) are
     applied to `get_module` — the §10.6 convention covers `execute()`, `load()`,
     `sample()`, `decode()`, and `compute_latent_shape()` methods; `get_module` is
     a shared dispatcher function, not one of those categories.

   Rationale: The design doc (§10.4) explicitly states that `get_module` is "the ONE
   shared dispatcher per family — never reimplemented per module." Each concrete module
   defines its own `can_handle(key)` independently; `get_module` only orchestrates the
   scan. With an empty registry, the function must return `None` silently — never raise.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `_REGISTERED_MODULES` | `worker.nodes.arch.diffusion` | `list[ModuleType] = []` (module-level, not pub) |
| `get_module` | `worker.nodes.arch.diffusion` | `def get_module(key: Any) -> ModuleType \| None` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/diffusion/__init__.py` | Diffusion arch family dispatcher: `_REGISTERED_MODULES` + `get_module()` |
| CREATE | `worker/tests/test_arch_dispatch.py` | Tests for `get_module()` with empty registry and test-double skip |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_arch_dispatch.py` | `test_get_module_returns_none_when_empty` | `get_module("zit")` returns `None` when `_REGISTERED_MODULES` is empty | `python -m pytest worker/tests/test_arch_dispatch.py::test_get_module_returns_none_when_empty -v` exits 0 |
| `worker/tests/test_arch_dispatch.py` | `test_get_module_does_not_raise_for_various_key_types` | `get_module()` does not raise for `str`, `None`, and arbitrary object keys when registry is empty | `python -m pytest worker/tests/test_arch_dispatch.py::test_get_module_does_not_raise_for_various_key_types -v` exits 0 |
| `worker/tests/test_arch_dispatch.py` | `test_get_module_skips_module_with_can_handle_false` | When a module's `can_handle` returns `False`, `get_module` continues scanning and returns `None` (or the first matching module) | `python -m pytest worker/tests/test_arch_dispatch.py::test_get_module_skips_module_with_can_handle_false -v` exits 0 |

## CI Impact

The new test file `worker/tests/test_arch_dispatch.py` is picked up automatically by
the existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`,
`worker-windows-real`) which run `python -m pytest worker/tests/ -v`. No CI workflow
changes are required. The test file contains no `torch` imports, so it collects cleanly
in both mock and real CI jobs.

## Platform Considerations

None identified. The dispatch logic is pure Python with no platform-specific code paths,
no file I/O, and no subprocess calls. The Windows cross-check in ENVIRONMENT.md §7 is
sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pkgutil.iter_modules()` auto-import is mentioned in §10.4 but not yet wired — the plan intentionally omits it since auto-import wiring is P10-C1's scope. If the ACT agent mistakenly adds auto-import logic, it would be scope creep. | Low | Low | The plan's Approach step 2 explicitly states no auto-import logic; the tests only verify `get_module` behavior against `_REGISTERED_MODULES` (not against filesystem scanning). |
| A test double's `can_handle` method is not callable if constructed incorrectly, causing `AttributeError` instead of `False` — this would make the skip test verify the wrong thing. | Low | Low | The test double uses `unittest.mock.Mock(can_handle=Mock(return_value=False))` which guarantees a callable `can_handle` attribute returning `False`. |
| `ModuleType` import path differs between Python versions — `types.ModuleType` is the standard import but a typo would cause `ImportError` at collection time. | Low | Low | Verified: `types.ModuleType` is the correct import for Python 3.12 (confirmed via stdlib docs). |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/arch/diffusion/__init__.py` exits 0
- [ ] `python -m py_compile worker/tests/test_arch_dispatch.py` exits 0
- [ ] `python -m pytest worker/tests/test_arch_dispatch.py -v` exits 0 (≥3 tests)
- [ ] `grep -c "def get_module" worker/nodes/arch/diffusion/__init__.py` outputs `1`
- [ ] `grep -c "_REGISTERED_MODULES" worker/nodes/arch/diffusion/__init__.py` outputs ≥1
