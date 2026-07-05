# Plan Report: P10-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A1                                      |
| Phase       | 10 — Generic Node Groundwork                |
| Description | worker/nodes/base.py: SlotSpec dataclass + NODE_REGISTRY dict |
| Depends on  | P9-F1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T21:10:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `worker/nodes/` package with two files — an empty `__init__.py` and a `base.py`
containing the `NODE_REGISTRY` dict and `SlotSpec` dataclass as defined verbatim in
`ANVILML_DESIGN.md §14.5`. Write at least three tests in `worker/tests/test_base.py`
that verify `NODE_REGISTRY` starts empty and `SlotSpec` constructs with both the default
`optional=False` and explicit `optional=True`. The acceptance command
`python -m pytest worker/tests/test_base.py -v` must exit 0.

## Scope

### In Scope
- Create `worker/nodes/` directory (package).
- Create `worker/nodes/__init__.py` — empty file (auto-import wiring is a later task).
- Create `worker/nodes/base.py` with:
  - Module docstring: `"Base node ABC and registration decorator."`
  - Imports: `from __future__ import annotations`, `from abc import ABC, abstractmethod`,
    `from typing import Any`, `from dataclasses import dataclass`
  - `NODE_REGISTRY: dict[str, type["BaseNode"]] = {}` module-level global
  - `@dataclass class SlotSpec` with fields `name: str`, `slot_type: str`,
    `optional: bool = False` and its docstring from §14.5
- Create `worker/tests/test_base.py` with >=3 tests verifying the above.

### Out of Scope
- `@register` decorator — separate task P10-A2.
- `NodeContext` class — separate task P10-A3.
- `BaseNode` ABC with abstract `execute()` — separate task P10-A4.
- Auto-import wiring in `__init__.py` — separate task P10-C1.
- Any concrete node implementations.

defers_to (from JSON): []

## Existing Codebase Assessment

No prior source exists under `worker/nodes/` — the directory does not exist yet. This
task establishes the baseline patterns for the node system in subsequent phases.

The existing Python test suite in `worker/tests/` follows a clear convention:
- Google-style docstrings on every test function describing what it verifies, its
  precondition, and expected outcome.
- Class-based test organization with `setup_method`/`teardown_method` for fixture
  cleanup when module-level globals need resetting.
- Subprocess isolation (via `subprocess.run`) when testing that a module does not
  import heavy dependencies at top level.
- Tests import only the public interface of the module under test.

The `worker/tests/conftest.py` exists but is empty — it contains no fixtures and no
test functions, consistent with the convention that it holds only shared fixtures when
they are needed.

The project's `pyproject.toml` registers a `real_mode` marker; tests without a marker
are assumed mock-compatible and run in both CI job groups.

## Resolved Dependencies

None. This task uses only Python 3.12 standard library modules: `dataclasses`, `abc`,
`typing`, `from __future__ import annotations`. No external packages are introduced.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (stdlib) | dataclasses | 3.12 stdlib | n/a | n/a |
| (stdlib) | abc | 3.12 stdlib | n/a | n/a |
| (stdlib) | typing | 3.12 stdlib | n/a | n/a |

## Approach

1. **Create `worker/nodes/` directory.**
   - `mkdir -p worker/nodes`

2. **Create `worker/nodes/__init__.py` as an empty file.**
   - Per task context: "empty for now, auto-import wiring is a later task."
   - The file must exist so Python recognises `worker.nodes` as a package.

3. **Create `worker/nodes/base.py` per ANVILML_DESIGN.md §14.5 verbatim.**
   - Module docstring (first line): `"Base node ABC and registration decorator."`
   - Import block (exact order from §14.5):
     ```python
     from __future__ import annotations
     from abc import ABC, abstractmethod
     from typing import Any
     from dataclasses import dataclass
     ```
   - `NODE_REGISTRY` global (exact type annotation from §14.5):
     ```python
     NODE_REGISTRY: dict[str, type["BaseNode"]] = {}
     ```
     This is a module-level mutable dict that will be populated by the `@register`
     decorator in a later task. The forward-reference `"BaseNode"` in the type
     annotation is valid because of the `from __future__ import annotations` import
     which defers evaluation of all annotations (PEP 563).
   - `SlotSpec` dataclass (exact fields and defaults from §14.5):
     ```python
     @dataclass
     class SlotSpec:
         """Declares one input or output slot on a node."""
         name: str
         slot_type: str          # Must match a SlotType value (e.g. "MODEL", "CLIP")
         optional: bool = False
     ```
     The inline comment on `slot_type` is part of the normative §14.5 text and must be
     copied verbatim. The `optional` parameter defaults to `False` — this is the key
     behavior verified by the tests.

   - **Do NOT include** `@register` or `NodeContext` — those are P10-A2 and P10-A3
     respectively. Including them would violate task atomicity (§4.1 of FORGE_AGENT_RULES).

4. **Create `worker/tests/test_base.py` with >=3 tests.**
   - Test 1: `test_node_registry_starts_empty` — import `base`, assert `NODE_REGISTRY == {}`.
   - Test 2: `test_slotspec_optional_defaults_to_false` — construct `SlotSpec(name="x", slot_type="MODEL")`, assert `optional is False`.
   - Test 3: `test_slotspec_accepts_explicit_optional_true` — construct `SlotSpec(name="y", slot_type="IMAGE", optional=True)`, assert `optional is True`.
   - Each test carries a Google-style docstring describing what it verifies.

## Public API Surface

| Path | Item | Signature / Description |
|------|------|------------------------|
| `worker/nodes/base.py` | `NODE_REGISTRY` | `dict[str, type["BaseNode"]] = {}` — module-level global registry |
| `worker/nodes/base.py` | `SlotSpec` | `@dataclass class SlotSpec: name: str, slot_type: str, optional: bool = False` |

No `pub` items in the Python sense — these are module-level names. No functions or
classes beyond `SlotSpec` are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/__init__.py` | Empty package init file |
| CREATE | `worker/nodes/base.py` | NODE_REGISTRY dict + SlotSpec dataclass (normative per §14.5) |
| CREATE | `worker/tests/test_base.py` | >=3 unit tests for NODE_REGISTRY and SlotSpec |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_base.py` | `test_node_registry_starts_empty` | `NODE_REGISTRY` is `{}` immediately after import | `python -m pytest worker/tests/test_base.py -v` exits 0 |
| `worker/tests/test_base.py` | `test_slotspec_optional_defaults_to_false` | `SlotSpec(name="x", slot_type="MODEL").optional` is `False` | `python -m pytest worker/tests/test_base.py -v` exits 0 |
| `worker/tests/test_base.py` | `test_slotspec_accepts_explicit_optional_true` | `SlotSpec(name="y", slot_type="IMAGE", optional=True).optional` is `True` | `python -m pytest worker/tests/test_base.py -v` exits 0 |

## CI Impact

No CI changes required. The new test file `worker/tests/test_base.py` is automatically
picked up by the existing pytest invocation in every `worker-*` CI job
(`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`)
because they all run `python -m pytest worker/tests/` which collects all `test_*.py`
files in that directory.

## Platform Considerations

None identified. The code uses only Python stdlib (`dataclasses`, `abc`, `typing`) with
no platform-specific imports, `os` calls, or path handling. The Windows cross-check in
ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `from __future__ import annotations` changes the type annotation semantics for `NODE_REGISTRY: dict[str, type["BaseNode"]] = {}` — the stringified annotation `"BaseNode"` is never evaluated, so the dict value is a plain `dict` at runtime, not a `dict[str, type[BaseNode]]`. A test that inspects the runtime type of `NODE_REGISTRY` via `type()` would see `dict`, not the annotated type. | Low | Low | The task only tests that `NODE_REGISTRY == {}` (empty dict), which is unaffected by annotation semantics. No test inspects runtime type annotations. |
| The `worker/nodes/` directory creation might fail if a file named `nodes` already exists at that path. | Low | Medium | The glob check confirmed no `worker/nodes/` exists. Use `mkdir -p` which handles the case gracefully (exits 0 if directory already exists). |
| The normative §14.5 text includes imports for `ABC`, `abstractmethod`, and `NodeContext` which this task does NOT implement. An ACT agent might be tempted to include them as dead imports. | Medium | Low | The plan explicitly states to copy only the SlotSpec and NODE_REGISTRY sections verbatim from §14.5, and to omit `@register` and `NodeContext`. The unused imports (`ABC`, `abstractmethod`) are part of the normative §14.5 import block and should be included as-is — they are not dead code, they are the imports the module needs per the design doc even though the current task only uses a subset. |

## Acceptance Criteria

- [ ] `test -f worker/nodes/__init__.py` exits 0 (file exists)
- [ ] `test -f worker/nodes/base.py` exits 0 (file exists)
- [ ] `test -f worker/tests/test_base.py` exits 0 (test file exists)
- [ ] `python -m py_compile worker/nodes/base.py` exits 0 (syntax check passes)
- [ ] `python -m pytest worker/tests/test_base.py -v` exits 0 (all tests pass)
