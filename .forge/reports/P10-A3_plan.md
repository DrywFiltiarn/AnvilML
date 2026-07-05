# Plan Report: P10-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-A3                                            |
| Phase       | 10 — Generic Node Groundwork                      |
| Description | worker/nodes/base.py: NodeContext runtime context class |
| Depends on  | P10-A2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-05T22:58:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the `NodeContext` runtime context class to `worker/nodes/base.py` per the normative
specification in `ANVILML_DESIGN.md §14.5`. This class carries everything a future node's
`execute()` method needs — job ID, device string, capability dict, cancellation flag,
emit callback, pipeline cache, and mock-mode flag — without requiring the node to reach
into global state. The acceptance criterion is >=4 new tests in
`worker/tests/test_base.py` (>=12 total tests in the file, exits 0).

## Scope

### In Scope
- Add the `NodeContext` class to `worker/nodes/base.py` with the exact docstring and
  `__init__` signature from `ANVILML_DESIGN.md §14.5` (copy verbatim).
- Add >=4 unit tests to `worker/tests/test_base.py` covering:
  - Constructor assigns all 7 attributes correctly.
  - `mock=True` constructs cleanly.
  - `mock=False` constructs cleanly.
  - `caps` accepts an arbitrary dict (any shape).

### Out of Scope
None. This task's `defers_to` field is `[]` — no scope may be deferred.

## Existing Codebase Assessment

The `worker/nodes/base.py` file already contains the module-level `NODE_REGISTRY` dict,
the `@register` decorator (validated in P10-A2), and the `SlotSpec` dataclass (from
P10-A1). The file is 34 lines and follows the project's established patterns:
Google-style docstrings with `Raises:` sections, `from __future__ import annotations`,
and no trailing whitespace. The existing test file `worker/tests/test_base.py` has 11
tests covering `NODE_REGISTRY` initialization, `SlotSpec` defaults, and all 6 missing-attr
`TypeError` cases for `@register`. Test style uses a `_FullySpecifiedNode` helper class
with `del` cleanup to avoid polluting the global registry. The project uses Python 3.12.x
with no external runtime dependencies for worker node code — `NodeContext` is a plain
class with no imports beyond what `base.py` already has.

The design doc (§14.5) specifies `NodeContext` as a plain class (not a dataclass), with
a docstring listing all 7 attributes and an `__init__` that assigns each parameter
directly to `self` with no validation or transformation. The existing codebase has no
`NodeContext` yet, so this task establishes the baseline for all future node `execute()`
methods.

## Resolved Dependencies

None. This task introduces no new Python packages or external dependencies. All imports
are from the Python standard library (`abc`, `dataclass`, `typing`, `from __future__`),
which `base.py` already uses.

## Approach

1. **Append `NodeContext` class to `worker/nodes/base.py`.**
   Copy the class definition from `ANVILML_DESIGN.md §14.5` verbatim — the docstring,
   attribute descriptions, and `__init__` signature. The class is placed after `SlotSpec`
   (the last existing item). No new imports are needed: the class is plain Python with
   no annotations beyond the docstring, so no additional `from typing import ...` line
   is required. The docstring uses `Args:` / `Returns:` style implicitly through its
   `Attributes:` section, matching the Google-style convention established by the existing
   `register()` docstring in this same file.

   Exact content to append (after the `SlotSpec` class):
   ```python

   class NodeContext:
       """Runtime context passed to every node's execute() method.

       Attributes:
           job_id: The UUID string of the currently executing job.
           device: The torch device string (e.g. "cuda:0", "cpu"). Unused in mock mode.
           caps: The worker's own InferenceCaps dict from capability.probe_capabilities()
               (or the mock equivalent). Arch modules read dtype decisions from this —
               never from a Rust-side hint — per §6.6/§11.5.
           cancel_flag: threading.Event; set when the job is cancelled.
           emit: Callable for emitting WorkerEvent dicts back to the supervisor.
           pipeline_cache: The shared LRU model/pipeline cache.
           mock: bool — True if ANVILML_WORKER_MOCK=1. Nodes branch on this exactly
               once, at the top of execute(), never deeper inside arch dispatch.
       """
       def __init__(self, job_id, device, caps, cancel_flag, emit, pipeline_cache, mock):
           self.job_id = job_id
           self.device = device
           self.caps = caps
           self.cancel_flag = cancel_flag
           self.emit = emit
           self.pipeline_cache = pipeline_cache
           self.mock = mock
   ```

2. **Add >=4 tests to `worker/tests/test_base.py`.**
   The tests import `from worker.nodes import base` (same pattern as existing tests) and
   construct `NodeContext` with minimal but valid values for each parameter. No cleanup
   is needed because `NodeContext` is a simple instance with no global state mutations.

   - `test_node_context_assigns_all_attrs`: constructs with concrete values for all 7
     params and asserts `self.job_id == "test-job"`, `self.device == "cpu"`,
     `self.caps == {"bf16": True, "fp8": False}`, `self.mock is True`, etc.
   - `test_node_context_mock_true`: constructs with `mock=True` and asserts
     `ctx.mock is True`.
   - `test_node_context_mock_false`: constructs with `mock=False` and asserts
     `ctx.mock is False`.
   - `test_node_context_caps_accepts_arbitrary_dict`: constructs with an arbitrary
     dict (`{"some_key": "some_value", "numeric": 42}`) and asserts
     `ctx.caps == {"some_key": "some_value", "numeric": 42}`.

3. **Verify the test count.**
   After adding 4 tests, `test_base.py` will have 15 tests total (11 existing + 4 new),
   exceeding the >=12 acceptance threshold.

## Public API Surface

| Item | Location | Description |
|------|----------|-------------|
| `class NodeContext` | `worker/nodes/base.py` | Runtime context class with 7 public attributes: `job_id`, `device`, `caps`, `cancel_flag`, `emit`, `pipeline_cache`, `mock`. No new `pub`/`def` beyond `__init__`. |

No new module-level exports or re-exports are added. `NodeContext` is accessible via
`from worker.nodes.base import NodeContext` (the module is already importable).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/base.py` | Append `NodeContext` class (verbatim from §14.5) |
| MODIFY | `worker/tests/test_base.py` | Add >=4 tests for `NodeContext` constructor |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_base.py` | `test_node_context_assigns_all_attrs` | `NodeContext` constructor assigns all 7 attributes to matching `self` attributes | `python -m pytest worker/tests/test_base.py::test_node_context_assigns_all_attrs -v` exits 0 |
| `worker/tests/test_base.py` | `test_node_context_mock_true` | `NodeContext` constructs cleanly with `mock=True` | `python -m pytest worker/tests/test_base.py::test_node_context_mock_true -v` exits 0 |
| `worker/tests/test_base.py` | `test_node_context_mock_false` | `NodeContext` constructs cleanly with `mock=False` | `python -m pytest worker/tests/test_base.py::test_node_context_mock_false -v` exits 0 |
| `worker/tests/test_base.py` | `test_node_context_caps_accepts_arbitrary_dict` | `caps` accepts any arbitrary dict without validation | `python -m pytest worker/tests/test_base.py::test_node_context_caps_accepts_arbitrary_dict -v` exits 0 |

## CI Impact

No CI changes required. The task only adds a Python class and unit tests to an existing
test file. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`,
`worker-windows-mock`, `worker-windows-real`) already run `pytest worker/tests/` which
will pick up the new tests automatically. No new test markers, fixtures, or CI gates
are introduced.

## Platform Considerations

None identified. The `NodeContext` class is a pure Python class with no platform-specific
code, no `os`/`sys` imports, no path handling, and no conditional imports. The Windows
cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Docstring copy from §14.5 drifts from the design doc due to formatting differences (e.g. trailing whitespace, line wrapping) | Low | Medium | Copy the docstring character-for-character from the design doc source; verify with `diff` after writing. |
| Adding tests increases total test count beyond what existing tests expect (e.g. if a fixture or conftest assumes a fixed test count) | Low | Low | The existing test file has no such assumption — no conftest fixture counts tests, and no test iterates over `test_base.py`'s test functions. Verify by running the full suite. |
| `NodeContext` class docstring attributes section doesn't follow Google-style convention (project expects `Args:`/`Returns:` but §14.5 uses `Attributes:`) | Low | Medium | The design doc's docstring is normative per §14.5 — copy it verbatim. If the project convention conflicts, the design doc takes precedence for this class. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/base.py` exits 0
- [ ] `python -m py_compile worker/tests/test_base.py` exits 0
- [ ] `python -m pytest worker/tests/test_base.py -v` exits 0 with >=12 tests collected
- [ ] `grep -c "^def test_" worker/tests/test_base.py` outputs a number >= 15
- [ ] `grep "class NodeContext:" worker/nodes/base.py` returns a match (class exists)
- [ ] `grep "self.job_id = job_id" worker/nodes/base.py && grep "self.device = device" worker/nodes/base.py && grep "self.caps = caps" worker/nodes/base.py && grep "self.cancel_flag = cancel_flag" worker/nodes/base.py && grep "self.emit = emit" worker/nodes/base.py && grep "self.pipeline_cache = pipeline_cache" worker/nodes/base.py && grep "self.mock = mock" worker/nodes/base.py` — all 7 assignments present
