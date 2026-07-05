# Plan Report: P10-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A2                                      |
| Phase       | 10 — Generic Node Groundwork                |
| Description | worker/nodes/base.py: @register decorator with required-attr validation |
| Depends on  | P10-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T20:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the `@register` decorator in `worker/nodes/base.py` that validates six required class attributes on a node class and inserts it into `NODE_REGISTRY` keyed by `cls.NODE_TYPE`. This closes the gap between P10-A1's `SlotSpec`/`NODE_REGISTRY` scaffolding and a fully functional registration mechanism, so future node classes can register themselves at import time with compile-time-like validation (fail-fast via `TypeError` at class-definition time rather than at first use).

## Scope

### In Scope
- Add the `register(cls: type) -> type` decorator function to `worker/nodes/base.py`, implemented EXACTLY per `ANVILML_DESIGN.md §14.5`.
- Six `hasattr` checks for `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`, raising `TypeError` naming the specific missing attribute.
- Insertion into `NODE_REGISTRY[cls.NODE_TYPE] = cls` on success.
- Return of the original class (identity preserved, no wrapper/proxy).
- Write tests in `worker/tests/test_base.py`: >=5 new tests covering success, individual missing-attr failures for all 6 attributes, and identity preservation. >=8 total tests in file (3 existing from P10-A1 + new tests).

### Out of Scope
None. This task's `defers_to` is `[]` (empty). No scope is deferred. The decorator is implemented fully, not as a stub.

## Existing Codebase Assessment

P10-A1 already created `worker/nodes/base.py` with `NODE_REGISTRY` (an empty `dict[str, type["BaseNode"]]`) and the `SlotSpec` dataclass. The file is 16 lines, ending at line 16 with the `SlotSpec` definition. There is no `@register` function, no `BaseNode` ABC, and no `NodeContext` class yet — those are P10-A2, P10-A3, and P10-A4 respectively.

`worker/tests/test_base.py` has 3 tests from P10-A1: `test_node_registry_starts_empty`, `test_slotspec_optional_defaults_to_false`, and `test_slotspec_accepts_explicit_optional_true`. The test style uses plain `pytest` functions with Google-style docstrings describing what is verified, preconditions, and expected outcomes. Tests import via `from worker.nodes import base`.

The design doc (`ANVILML_DESIGN.md §14.5`) provides the **exact** implementation of `register()` — it is normative and must be copied verbatim (aside from adding docstrings per §10 of ENVIRONMENT.md). The existing codebase has no conflicting types or patterns for this decorator; it is a clean addition to the existing module.

## Resolved Dependencies

None. This task introduces no new external crates or packages. All functionality uses Python stdlib (`abc`, `dataclasses`, `typing`).

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | — | — | — | — |

## Approach

1. **Add the `register` decorator to `worker/nodes/base.py`** — insert the function between the `NODE_REGISTRY` declaration and the `@dataclass` `SlotSpec` class (or after `SlotSpec`; placement is stylistic, but the function must be module-level and importable). Implement exactly per `ANVILML_DESIGN.md §14.5`:

   ```python
   def register(cls: type) -> type:
       """Register a node class in NODE_REGISTRY.

       The class must define NODE_TYPE (str), CATEGORY (str), DISPLAY_NAME (str),
       DESCRIPTION (str), INPUT_SLOTS (list[SlotSpec]), and OUTPUT_SLOTS (list[SlotSpec]).

       Raises:
           TypeError: If any required attribute is missing.
       """
       required = ("NODE_TYPE", "CATEGORY", "DISPLAY_NAME", "DESCRIPTION",
                   "INPUT_SLOTS", "OUTPUT_SLOTS")
       for attr in required:
           if not hasattr(cls, attr):
               raise TypeError(f"@register: {cls.__name__} missing {attr}")
       NODE_REGISTRY[cls.NODE_TYPE] = cls
       return cls
   ```

   Rationale: This is a decorator that registers, not wraps. Returning `cls` unchanged (not a wrapper) preserves the class's identity, MRO, and method resolution — critical because `execute()` must be callable directly on the original class.

2. **Write tests in `worker/tests/test_base.py`** — append 8 new test functions after the 3 existing ones. Each test follows the established pattern: a Google-style docstring, then assertions. Use a helper class `_FullySpecifiedNode` defined at module level in the test file to avoid repeating six attributes across every test.

   Test 1: `test_register_success` — Define a class with all 6 required attrs, decorate it, assert it is in `NODE_REGISTRY` under its `NODE_TYPE`, then remove it from `NODE_REGISTRY` to avoid polluting subsequent tests.

   Test 2–7: `test_register_missing_NODE_TYPE`, `test_register_missing_CATEGORY`, `test_register_missing_DISPLAY_NAME`, `test_register_missing_DESCRIPTION`, `test_register_missing_INPUT_SLOTS`, `test_register_missing_OUTPUT_SLOTS` — Each defines a class missing exactly one attribute, calls `@register`, and asserts `pytest.raises(TypeError, match="<ATTR_NAME>")`. The `match` parameter ensures the error message names the specific missing attribute.

   Test 8: `test_register_returns_class_identity` — Decorate a fully-specified class, assert the return value is the exact same object (`is` comparison), confirming identity preservation.

   After all tests, `NODE_REGISTRY` may contain entries from successful decorations. Each success test cleans up by removing its entry from `NODE_REGISTRY` to maintain test isolation (same pattern as P10-A1's empty-registry precondition).

3. **Run the acceptance command** — `python -m pytest worker/tests/test_base.py -v` must exit 0 with >=8 tests collected and passing.

## Public API Surface

| Item | Module Path | Signature / Type |
|------|-------------|-----------------|
| `register` | `worker.nodes.base` | `def register(cls: type) -> type` |
| `NODE_REGISTRY` (existing, unchanged) | `worker.nodes.base` | `dict[str, type["BaseNode"]] = {}` |
| `SlotSpec` (existing, unchanged) | `worker.nodes.base` | `@dataclass class SlotSpec(name: str, slot_type: str, optional: bool = False)` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/base.py` | Add `register()` decorator function per §14.5 |
| MODIFY | `worker/tests/test_base.py` | Add >=5 new tests for `register()`, total >=8 tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_base.py` | `test_register_success` | Decorating a fully-specified class succeeds, inserts into `NODE_REGISTRY` keyed by `NODE_TYPE` | `python -m pytest worker/tests/test_base.py::test_register_success -v` exits 0 |
| `worker/tests/test_base.py` | `test_register_missing_NODE_TYPE` | Missing `NODE_TYPE` raises `TypeError` with "NODE_TYPE" in the message | `python -m pytest worker/tests/test_base.py::test_register_missing_NODE_TYPE -v` exits 0 |
| `worker/tests/test_base.py` | `test_register_missing_CATEGORY` | Missing `CATEGORY` raises `TypeError` with "CATEGORY" in the message | `python -m pytest worker/tests/test_base.py::test_register_missing_CATEGORY -v` exits 0 |
| `worker/tests/test_base.py` | `test_register_missing_DISPLAY_NAME` | Missing `DISPLAY_NAME` raises `TypeError` with "DISPLAY_NAME" in the message | `python -m pytest worker/tests/test_base.py::test_register_missing_DISPLAY_NAME -v` exits 0 |
| `worker/tests/test_base.py` | `test_register_missing_DESCRIPTION` | Missing `DESCRIPTION` raises `TypeError` with "DESCRIPTION" in the message | `python -m pytest worker/tests/test_base.py::test_register_missing_DESCRIPTION -v` exits 0 |
| `worker/tests/test_base.py` | `test_register_missing_INPUT_SLOTS` | Missing `INPUT_SLOTS` raises `TypeError` with "INPUT_SLOTS" in the message | `python -m pytest worker/tests/test_base.py::test_register_missing_INPUT_SLOTS -v` exits 0 |
| `worker/tests/test_base.py` | `test_register_missing_OUTPUT_SLOTS` | Missing `OUTPUT_SLOTS` raises `TypeError` with "OUTPUT_SLOTS" in the message | `python -m pytest worker/tests/test_base.py::test_register_missing_OUTPUT_SLOTS -v` exits 0 |
| `worker/tests/test_base.py` | `test_register_returns_class_identity` | Decorator returns the exact same class object (`is` identity check) | `python -m pytest worker/tests/test_base.py::test_register_returns_class_identity -v` exits 0 |

## CI Impact

No CI changes required. The tests are added to an existing test file (`worker/tests/test_base.py`) that is already collected by the mock-mode and real-mode pytest runs in CI (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`). No new test markers, file patterns, or CI gates are introduced.

## Platform Considerations

None identified. The `@register` decorator is pure Python with no platform-specific code, no `os`/`sys` calls, no path handling, and no subprocess spawning. It operates entirely at class-definition time in the Python process. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `NODE_REGISTRY` entries from success tests leak into subsequent tests, causing false positives when tests expect an empty registry | Medium | High | Each success test removes its entry from `NODE_REGISTRY` after asserting. The `test_node_registry_starts_empty` test (P10-A1) runs first by alphabetical order and verifies the module-level registry starts empty; if a prior test leaks, this test fails. |
| `pytest.raises(TypeError, match="...")` uses regex matching — a missing word in the pattern causes a false negative | Low | Medium | Use exact attribute names from the design doc (§14.5). The `match` parameter matches against the exception message string, which the design doc specifies as `f"@register: {cls.__name__} missing {attr}"`. No special regex characters in attribute names. |
| Test file grows beyond Python 350-line threshold | Low | Low | Adding 8 tests (~200 lines including docstrings) to a 40-line file keeps it well under 350 lines. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/base.py` exits 0
- [ ] `python -m py_compile worker/tests/test_base.py` exits 0
- [ ] `python -m pytest worker/tests/test_base.py -v` exits 0 with >=8 tests collected and passing
- [ ] `NODE_REGISTRY` is empty before import (verified by `test_node_registry_starts_empty`, runs first)
- [ ] Decorating a fully-specified class adds it to `NODE_REGISTRY` under its `NODE_TYPE` key (verified by `test_register_success`)
- [ ] Missing any of the 6 required attributes raises `TypeError` naming that attribute (verified by `test_register_missing_*` tests 2–7)
- [ ] Decorator returns the original class object unchanged (verified by `test_register_returns_class_identity`)
