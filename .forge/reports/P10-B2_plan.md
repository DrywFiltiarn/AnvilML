# Plan Report: P10-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-B2                                      |
| Phase       | 10 — Generic Node Groundwork                |
| Description | worker/nodes/arch/clip/__init__.py and arch/vae/__init__.py: same dispatch |
| Depends on  | P10-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T09:05:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `get_module()` dispatch mechanism for the CLIP and VAE architecture families,
mirroring the exact structure already implemented for the diffusion family in P10-B1.
Both new `__init__.py` files start with an empty `_REGISTERED_MODULES` list, so
`get_module()` returns `None` for any key without raising. This completes the three-family
dispatch scaffold (diffusion, clip, vae) that later phases will populate with concrete
arch modules.

## Scope

### In Scope
- Create `worker/nodes/arch/clip/__init__.py` with the same `get_module()` structure as
  `diffusion/__init__.py` (empty `_REGISTERED_MODULES`, scan loop calling `can_handle()`).
- Create `worker/nodes/arch/vae/__init__.py` with the identical `get_module()` structure.
- Add 6 new test functions to `worker/tests/test_arch_dispatch.py`: 3 for the clip
  dispatcher, 3 for the vae dispatcher — mirroring the existing 3 diffusion tests.

### Out of Scope

defers_to (from JSON): []

No scope is deferred. The empty `defers_to` field forbids deferral outright.

- Concrete arch modules (`qwen3.py`, `zit_vae.py`, `flux2_vae.py`) — out of this phase's
  scope entirely, reserved for later phases.
- `can_handle()` implementations — each concrete module defines its own in a later phase.
- Registration of modules into `_REGISTERED_MODULES` — no modules exist yet to register.

## Existing Codebase Assessment

Phase 10 Group A (P10-A1 through P10-A4) has already established `worker/nodes/base.py`
with `SlotSpec`, `NODE_REGISTRY`, `@register`, `NodeContext`, and `BaseNode(ABC)`.

Phase 10 Group B (P10-B1) has already created `worker/nodes/arch/diffusion/__init__.py`
containing the shared dispatch pattern: a module-level `_REGISTERED_MODULES: list[ModuleType]`
list and a `get_module(key: Any) -> ModuleType | None` function that iterates the list
calling `module.can_handle(key)` on each entry, returning the first match or `None`.

The test file `worker/tests/test_arch_dispatch.py` contains 3 tests exercising the
diffusion dispatcher: `test_get_module_returns_none_when_empty`,
`test_get_module_does_not_raise_for_various_key_types`, and
`test_get_module_skips_module_with_can_handle_false` (using a `Mock`).

The `worker/nodes/arch/clip/` and `worker/nodes/arch/vae/` directories do not exist yet.
No test files for clip or vae dispatch exist.

The established patterns to follow:
- Module docstring referencing ANVILML_DESIGN.md §10.4
- Google-style docstrings on `get_module()` with `Args:` and `Returns:` sections
- `from __future__ import annotations` at the top of every new module
- Tests import via `from worker.nodes.arch import <family>` (not absolute imports)
- Mock-based testing with `unittest.mock.Mock` and `types.ModuleType` spec

## Resolved Dependencies

None. This task introduces no new external dependencies. It only creates Python files
within the existing worker package, using only the standard library (`types.ModuleType`,
`typing.Any`).

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) |      |                 |            |                        |

## Approach

### Step 1: Create `worker/nodes/arch/clip/__init__.py`

Copy the exact structure from `worker/nodes/arch/diffusion/__init__.py` with three
differences: (a) the module docstring references "CLIP architecture family" and
"text-encoder" instead of "diffusion architecture family", (b) the `_REGISTERED_MODULES`
list is empty (no modules registered yet), and (c) the `can_handle()` inline comment
references "clip_type string" as the key type (per ANVILML_DESIGN.md §10.4's `can_handle`
docstring which says "clip: key is the clip_type string").

The file will contain exactly:
- Module docstring (Google-style, referencing ANVILML_DESIGN.md §10.4)
- `from __future__ import annotations`
- `from typing import Any`
- `from types import ModuleType`
- `_REGISTERED_MODULES: list[ModuleType] = []`
- `get_module(key: Any) -> ModuleType | None` function with full docstring and scan loop

The `get_module()` function body is **verbatim identical** to the diffusion version —
this is the "one shared implementation" per ANVILML_DESIGN.md §10.4. The only variation
between families is the docstring text and the registered module contents (which are
empty at this phase).

### Step 2: Create `worker/nodes/arch/vae/__init__.py`

Identical to Step 1, with the module docstring referencing "VAE architecture family"
and "VAE" instead of "CLIP" or "diffusion". The key type for VAE is an arch string
read from safetensors metadata or a path-derived fallback (per §10.4's `can_handle`
docstring).

### Step 3: Add clip tests to `worker/tests/test_arch_dispatch.py`

Append three new test functions after the existing diffusion tests:

**`test_clip_get_module_returns_none_when_empty()`** — imports `from worker.nodes.arch import clip`, calls `clip.get_module("qwen3")`, asserts `None`. Same pattern as the diffusion test.

**`test_clip_get_module_does_not_raise_for_various_key_types()`** — calls `clip.get_module("qwen3")`, `clip.get_module(None)`, `clip.get_module(object())`, asserts all return `None`. Same pattern as the diffusion test.

**`test_clip_get_module_skips_module_with_can_handle_false()`** — creates a `Mock(spec=ModuleType)` with `can_handle` returning `False`, appends to `clip._REGISTERED_MODULES`, calls `clip.get_module("qwen3")`, asserts `None` and that `can_handle` was called once, then removes the mock in a `finally` block. Same pattern as the diffusion test.

### Step 4: Add vae tests to `worker/tests/test_arch_dispatch.py`

Append three new test functions with the same structure as the clip tests, but
importing `from worker.nodes.arch import vae` and using `"zit_vae"` as the test key.

**`test_vae_get_module_returns_none_when_empty()`**
**`test_vae_get_module_does_not_raise_for_various_key_types()`**
**`test_vae_get_module_skips_module_with_can_handle_false()`**

### Step 5: Verify

Run `python -m py_compile` on the two new `__init__.py` files and the modified
`test_arch_dispatch.py` to confirm syntax correctness. Then run the full test suite
for the file to confirm all 9 tests pass.

### Dual-mode parity marker check

This task creates only `get_module()` dispatcher functions, not `execute()`, `load()`,
`sample()`, or `decode()` functions. The dual-mode parity marker convention
(ANVILML_DESIGN.md §10.6) applies exclusively to those four function categories.
Therefore, no `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers are needed for this task.

### Phase-closing task check

P10-B2 is NOT the last task in `tasks_phase010.json` (P10-E1 is). Therefore,
the §9a end-of-phase deliverable audit does not apply to this task.

## Public API Surface

| Item | Module Path | Description |
|------|-------------|-------------|
| `get_module(key: Any) -> ModuleType \| None` | `worker.nodes.arch.clip` | CLIP family dispatcher — returns first registered module whose `can_handle(key)` is True |
| `_REGISTERED_MODULES: list[ModuleType]` | `worker.nodes.arch.clip` | Empty registry list (private, no `pub` equivalent in Python) |
| `get_module(key: Any) -> ModuleType \| None` | `worker.nodes.arch.vae` | VAE family dispatcher — same signature as CLIP |
| `_REGISTERED_MODULES: list[ModuleType]` | `worker.nodes.arch.vae` | Empty registry list (private) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/clip/__init__.py` | CLIP architecture family dispatch module |
| CREATE | `worker/nodes/arch/vae/__init__.py` | VAE architecture family dispatch module |
| MODIFY | `worker/tests/test_arch_dispatch.py` | Add 6 new test functions (3 clip + 3 vae) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_dispatch.py` | `test_clip_get_module_returns_none_when_empty` | `clip.get_module("qwen3")` returns `None` with empty registry | `clip` module freshly imported | `"qwen3"` | `None` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_clip_get_module_returns_none_when_empty -v` exits 0 |
| `worker/tests/test_arch_dispatch.py` | `test_clip_get_module_does_not_raise_for_various_key_types` | `clip.get_module()` does not raise for `str`, `None`, or `object()` keys | `clip` module freshly imported | `"qwen3"`, `None`, `object()` | All return `None` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_clip_get_module_does_not_raise_for_various_key_types -v` exits 0 |
| `worker/tests/test_arch_dispatch.py` | `test_clip_get_module_skips_module_with_can_handle_false` | Dispatcher skips a module whose `can_handle` returns `False` | `clip._REGISTERED_MODULES` is empty before test | `"qwen3"` with mock returning `False` | `None`, mock called once | `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_clip_get_module_skips_module_with_can_handle_false -v` exits 0 |
| `worker/tests/test_arch_dispatch.py` | `test_vae_get_module_returns_none_when_empty` | `vae.get_module("zit_vae")` returns `None` with empty registry | `vae` module freshly imported | `"zit_vae"` | `None` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_vae_get_module_returns_none_when_empty -v` exits 0 |
| `worker/tests/test_arch_dispatch.py` | `test_vae_get_module_does_not_raise_for_various_key_types` | `vae.get_module()` does not raise for `str`, `None`, or `object()` keys | `vae` module freshly imported | `"zit_vae"`, `None`, `object()` | All return `None` | `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_vae_get_module_does_not_raise_for_various_key_types -v` exits 0 |
| `worker/tests/test_arch_dispatch.py` | `test_vae_get_module_skips_module_with_can_handle_false` | Dispatcher skips a module whose `can_handle` returns `False` | `vae._REGISTERED_MODULES` is empty before test | `"zit_vae"` with mock returning `False` | `None`, mock called once | `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py::test_vae_get_module_skips_module_with_can_handle_false -v` exits 0 |

## CI Impact

No CI changes required. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`,
`worker-windows-mock`, `worker-windows-real`) all run `pytest worker/tests/` which
will pick up the new test functions automatically. No new markers, file patterns, or
configuration changes are needed.

## Platform Considerations

None identified. The code uses only standard library imports (`types.ModuleType`,
`typing.Any`) and `unittest.mock.Mock` — all cross-platform. No `os.path` vs
`pathlib` distinctions, no line-ending handling, no platform-specific branches.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The clip and vae `__init__.py` files are near-duplicates of diffusion's, risking subtle divergence (e.g. different docstrings, different comment text) that violates ANVILML_DESIGN.md §10.4's "do not write three separate iteration implementations" rule. | Medium | Medium | The plan specifies that the `get_module()` function body is **verbatim identical** across all three families. Only the docstring text differs. The ACT agent should copy the diffusion file and perform only the targeted substitutions (family name, key type description). |
| Tests in `test_arch_dispatch.py` share the global `_REGISTERED_MODULES` list across families — if a test for one family fails to clean up its mock in the `finally` block, subsequent tests for that same family could observe stale state. | Low | Low | Each family has its own `_REGISTERED_MODULES` list (`clip._REGISTERED_MODULES`, `vae._REGISTERED_MODULES`). The mock-cleanup tests use `try/finally` to remove the fake module, matching the diffusion test pattern exactly. |
| The test file grows beyond a reasonable size with 9 tests total. | Low | Low | 9 tests across 3 families is well within the 350-line Python file guideline. The file will be approximately 150-180 lines — far below threshold. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/clip/__init__.py worker/nodes/arch/vae/__init__.py worker/tests/test_arch_dispatch.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_arch_dispatch.py -v` exits 0 with 9 tests (3 diffusion + 3 clip + 3 vae)
- [ ] `head -1 .forge/reports/P10-B2_plan.md` prints `# Plan Report: P10-B2`
- [ ] `grep "^## " .forge/reports/P10-B2_plan.md` shows all 12 section headings
- [ ] `wc -l .forge/reports/P10-B2_plan.md` reports > 40 lines
