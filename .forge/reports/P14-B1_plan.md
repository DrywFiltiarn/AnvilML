# Plan Report: P14-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-B1                                      |
| Phase       | 14 — Dispatch & Execute                     |
| Description | worker/nodes/passthrough.py: trivial real node (no-op) |
| Depends on  | P10-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-07T18:40:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/passthrough.py` containing the `PassThrough` node — the first concrete node class in this project. The node is deliberately trivial: it reads one `ANY`-typed input slot named `"value"` and returns it unchanged as the sole output. Despite its simplicity, the implementation must exercise the full node infrastructure: `@register` decoration, `BaseNode` inheritance, `ctx.mock` branching (both branches return identically), and the `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` dual-mode parity marker pair. This proves the dispatch pipeline end-to-end and validates that the marker convention works on a real file, not just in the abstract.

## Scope

### In Scope
- Create `worker/nodes/passthrough.py` with the `PassThrough` class:
  - Inherits from `BaseNode`
  - Defines `NODE_TYPE = "PassThrough"`, `CATEGORY = "Debug"`, `DISPLAY_NAME = "Pass Through"`, `DESCRIPTION`
  - `INPUT_SLOTS = [SlotSpec("value", "ANY")]`
  - `OUTPUT_SLOTS = [SlotSpec("value", "ANY")]`
  - `execute(self, ctx: NodeContext, **inputs) -> dict` with one `ctx.mock` branch at the top
  - Both mock and real branches return `{"value": inputs["value"]}`
  - `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` comment markers above `execute()`
  - `@register` decorator applied
- Create `worker/tests/test_passthrough.py` with ≥5 tests covering mock execute, real execute, registry inclusion, marker collectibility, and class attributes.

### Out of Scope
None. defers_to (from JSON): []. This task implements its full scope — no stubs, no deferred functionality.

## Existing Codebase Assessment

The node system infrastructure is fully established from Phase 10 (P10-A4). Three files are directly relevant:

1. **`worker/nodes/base.py`** (88 lines): Defines `BaseNode` (ABC with abstract `execute()`), `NodeContext` (data carrier with `job_id`, `device`, `caps`, `cancel_flag`, `emit`, `pipeline_cache`, `mock`), `SlotSpec` (dataclass with `name`, `slot_type`, `optional`), `@register` decorator (validates 6 required class attributes, inserts into `NODE_REGISTRY`), and the module-level `NODE_REGISTRY: dict[str, type["BaseNode"]]` global.

2. **`worker/nodes/__init__.py`** (36 lines): Auto-imports all `.py` files in `nodes/` via `pkgutil.iter_modules()`, skipping `__init__`, `base`, and packages (to avoid recursing into `arch/`). Runs `_import_nodes()` at module load time. This means `PassThrough` will be automatically registered when `worker.nodes` is imported.

3. **`worker/tests/test_base.py`** (355 lines): Demonstrates the project's test style — Google-style docstrings on every function, `assert`-based assertions (no `unittest`), `setup_method`/`teardown_method` for stateful tests, `_FullySpecifiedNode` helper pattern for reducing repetition, and careful cleanup of `NODE_REGISTRY` entries after tests that modify it.

The established patterns to follow:
- Google-style docstrings with `Args:`, `Returns:`, `Raises:` sections for non-trivial functions.
- Inline `#` comments at decision points (the `ctx.mock` branch is the decision point).
- Test functions named `test_<subject>_<condition>` with one-sentence docstrings explaining what is verified and why.
- No `torch` imports at module level in test files (mock-mode compatibility).

No gap exists between the design doc and current source: `BaseNode`, `NodeContext`, `SlotSpec`, `@register`, and the auto-import mechanism are all present and match the contract described in `ANVILML_DESIGN.md §10.2–§10.4`.

## Resolved Dependencies

None. This task introduces no new Python packages or crates. It uses only existing infrastructure from `worker/nodes/base.py` (`BaseNode`, `SlotSpec`, `@register`, `NodeContext`) and the standard library (`unittest.mock`-equivalent patterns via `threading.Event` for context construction).

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) | —    | —               | —          | —                      |

## Approach

**Step 1: Create `worker/nodes/passthrough.py`.**

Write a single Python module file containing:

1. A module-level docstring: `"PassThrough node — trivial no-op passthrough for testing the dispatch pipeline."`

2. Imports from the existing base module:
   ```python
   from worker.nodes.base import BaseNode, NodeContext, SlotSpec, register
   ```

3. The `PassThrough` class definition with these class attributes:
   - `NODE_TYPE = "PassThrough"`
   - `CATEGORY = "Debug"`
   - `DISPLAY_NAME = "Pass Through"`
   - `DESCRIPTION = "A trivial no-op node that passes its input value through unchanged. Used to verify the dispatch pipeline and marker convention."`
   - `INPUT_SLOTS = [SlotSpec("value", "ANY")]`
   - `OUTPUT_SLOTS = [SlotSpec("value", "ANY")]`

4. The `execute` method with dual-mode parity markers and mock/real branching:
   ```python
   # REAL_PATH_VERIFIED: worker/tests/test_passthrough.py::test_execute_real_returns_input
   # MOCK_PATH_VERIFIED: worker/tests/test_passthrough.py::test_execute_mock_returns_input
   def execute(self, ctx: NodeContext, **inputs) -> dict:
       """Execute the pass-through node.

       Both mock and real branches return the input value unchanged.
       The branch exists solely to satisfy the dual-mode parity marker
       convention (§10.6) — this node has no meaningfully different
       behavior between modes.

       Args:
           ctx: Runtime context (unused by this node; present for
               consistency with the execute() signature).
           **inputs: Must contain a "value" key. The value is returned
               as the sole output under the same key.

       Returns:
           Dict with key "value" containing the same value that was
           passed in via inputs["value"].
       """
       if ctx.mock:
           # Mock branch: return input unchanged (no torch, no side effects).
           return {"value": inputs["value"]}
       else:
           # Real branch: same passthrough logic — no torch dependency exists
           # in this node, so the real path is identical to mock.
           return {"value": inputs["value"]}
   ```

5. Apply `@register` as a decorator on the class (placed before the class definition, as with all other nodes).

**Rationale for identical mock/real branches:** The task context explicitly states this node has "no meaningfully different mock-vs-real behavior." The `ctx.mock` branch is required by the parity marker convention, but both branches produce the same result. This is intentional — the node's purpose is to prove the dispatch pipeline and marker convention work on a real file.

**Step 2: Create `worker/tests/test_passthrough.py` with ≥5 tests.**

Write a test file with the following tests (all in one file, no class wrappers unless stateful):

1. **`test_class_attributes()`** — Verifies all six required class attributes exist with correct values (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`).

2. **`test_execute_mock_returns_input()`** — Constructs a `NodeContext` with `mock=True`, calls `execute()` with `{"value": "hello"}`, asserts the return is `{"value": "hello"}`. This test exercises the mock code path and satisfies the `MOCK_PATH_VERIFIED` marker.

3. **`test_execute_real_returns_input()`** — Constructs a `NodeContext` with `mock=False`, calls `execute()` with `{"value": 42}`, asserts the return is `{"value": 42}`. This test exercises the real code path and satisfies the `REAL_PATH_VERIFIED` marker.

4. **`test_node_in_registry_after_import()`** — Imports `worker.nodes.passthrough` (triggering `@register`), then checks `NODE_REGISTRY["PassThrough"]` exists and is the `PassThrough` class. This proves auto-import and registration work end-to-end.

5. **`test_markers_name_collectible_tests()`** — Reads the `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` marker comments from the source file, extracts the test identifiers, and runs `pytest --collect-only` on each to confirm they are collectible (exit 0). This is the mechanical validation that Gate 4 performs.

6. **`test_execute_returns_new_dict()`** — Calls `execute()` and asserts the return value is a new dict object each time (not a shared singleton), confirming no accidental state leakage between calls.

**Test file structure:**
- Module-level docstring describing the test file's purpose.
- Each test function has a Google-style docstring explaining what it verifies, the precondition, and the expected outcome.
- No `torch` imports at module level (mock-mode compatible).
- No `NODE_REGISTRY` cleanup needed for test 4 since it tests the registry state intentionally.

**Step 3: Verify syntax.**
Run `python -m py_compile worker/nodes/passthrough.py worker/tests/test_passthrough.py` to confirm no syntax errors before running pytest. This is the mandatory pre-test static check per `ENVIRONMENT.md §6, Step 7`.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| Class `PassThrough` | `worker.nodes.passthrough.PassThrough` | Concrete node, inherits `BaseNode.execute()`, decorated with `@register` |
| Method `PassThrough.execute()` | `worker.nodes.passthrough.PassThrough.execute(self, ctx: NodeContext, **inputs) -> dict` | Returns `{"value": inputs["value"]}`; branches on `ctx.mock` |

No new `pub` items in Rust crates. No new external packages.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/passthrough.py` | PassThrough node class — first concrete node file |
| CREATE | `worker/tests/test_passthrough.py` | ≥5 tests for PassThrough node |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_passthrough.py` | `test_class_attributes()` | All 6 required class attributes exist with correct values | `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_class_attributes -v` exits 0 |
| `worker/tests/test_passthrough.py` | `test_execute_mock_returns_input()` (mock) | Mock-mode `execute()` returns input unchanged; satisfies `MOCK_PATH_VERIFIED` marker | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_execute_mock_returns_input -v` exits 0 |
| `worker/tests/test_passthrough.py` | `test_execute_real_returns_input()` (real) | Real-mode `execute()` returns input unchanged; satisfies `REAL_PATH_VERIFIED` marker | `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_execute_real_returns_input -v` exits 0 |
| `worker/tests/test_passthrough.py` | `test_node_in_registry_after_import()` | `PassThrough` appears in `NODE_REGISTRY` after importing the module | `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_node_in_registry_after_import -v` exits 0 |
| `worker/tests/test_passthrough.py` | `test_markers_name_collectible_tests()` | Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` marker test IDs are collectible by pytest | `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_markers_name_collectible_tests -v` exits 0 |
| `worker/tests/test_passthrough.py` | `test_execute_returns_new_dict()` | Each `execute()` call returns a new dict (no shared singleton state) | `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py::test_execute_returns_new_dict -v` exits 0 |

## CI Impact

No CI changes required. The test file follows the existing naming convention (`test_*.py` in `worker/tests/`) and will be picked up automatically by the existing CI jobs:
- `worker-linux-mock`: runs with `ANVILML_WORKER_MOCK=1`, picks up all tests (no `real_mode` marker on any test, so all run in both mock and real CI jobs).
- `worker-linux-real`: runs without `ANVILML_WORKER_MOCK`, picks up all tests.
- The test file does not import `torch` at module level, so it is safe for the mock-mode CI job that installs only `base.txt` (no torch).

## Platform Considerations

None identified. The `PassThrough` node has no platform-specific code, no file I/O, no path handling, no conditional compilation. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `test_markers_name_collectible_tests()` may fail if the marker comment format does not exactly match what `pytest --collect-only` expects (e.g., trailing whitespace, different path separators). | Low | Medium | Use the exact format specified in `MARKER_CONVENTION.md`: `worker/tests/test_<module>.py::test_<name>`. Parse the marker with `split("::")` and pass the result directly to `pytest --collect-only`. |
| The `@register` decorator runs at module load time, so `test_node_in_registry_after_import()` may see stale entries from prior test runs if pytest reuses the process. | Low | Low | Import the module fresh via `importlib.import_module("worker.nodes.passthrough")` in a subprocess to avoid cross-test pollution, following the pattern used in `test_ipc.py::test_module_no_torch_import`. |
| Gate 4 (`ENVIRONMENT.md §8`) dual-mode marker sweep may flag the file if `grep -L` returns `passthrough.py` because the markers are on the method, not the class. | Medium | High | Place markers immediately above the `execute()` method (the function scope the convention applies to), matching the exact placement shown in `MARKER_CONVENTION.md` and the existing pattern in `ANVILML_DESIGN.md §10.6`. Gate 4's `grep -L` checks for the presence of both markers in the file, not their precise placement. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/passthrough.py worker/tests/test_passthrough.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py -v` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_passthrough.py -v -m real_mode` exits 0
- [ ] `grep "REAL_PATH_VERIFIED:" worker/nodes/passthrough.py` returns non-empty output
- [ ] `grep "MOCK_PATH_VERIFIED:" worker/nodes/passthrough.py` returns non-empty output
- [ ] `grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" worker/nodes/ \| sed -E 's/.*(REAL\|MOCK)_PATH_VERIFIED: *//' \| xargs -I{} worker/.venv/bin/python -m pytest --collect-only "{}" -q` exits 0 for both named tests
- [ ] `python -m pytest worker/tests/test_passthrough.py -v` exits 0 with ≥5 tests collected
