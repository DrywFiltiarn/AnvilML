# Plan Report: P10-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A4                                      |
| Phase       | 10 — Generic Node Groundwork                |
| Description | worker/nodes/base.py: BaseNode ABC abstract execute() |
| Depends on  | P10-A3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T00:14:52Z                        |
| Attempt     | 1                                           |

## Objective

Add the `BaseNode` abstract base class with an abstract `execute()` method to `worker/nodes/base.py`, completing the Group A base contract. This gives every future concrete node a common superclass that Python's ABC machinery enforces — a subclass missing `execute()` cannot be instantiated. Three new tests in `worker/tests/test_base.py` confirm the ABC semantics, bringing the file to >=15 tests total.

## Scope

### In Scope
- Add `class BaseNode(ABC)` to `worker/nodes/base.py` with abstract method `execute(self, ctx: NodeContext, **inputs) -> dict` decorated with `@abstractmethod`.
- Add three tests to `worker/tests/test_base.py`:
  1. `test_base_node_cannot_be_instantiated` — `BaseNode()` raises `TypeError`.
  2. `test_concrete_subclass_instantiates` — a minimal subclass implementing `execute()` instantiates without error.
  3. `test_execute_calls_subclass_impl` — calling `execute()` on the concrete subclass invokes the subclass's implementation, not a base no-op.

### Out of Scope
None. `defers_to (from JSON): []` — this task may not defer any scope. No stubs, no stubs for later phases, no "implement at ACT time" placeholders.

## Existing Codebase Assessment

`worker/nodes/base.py` (59 lines) already contains the outputs of P10-A1 through P10-A3: `NODE_REGISTRY` (module-level dict), `register()` decorator (validates six required class attributes), `SlotSpec` dataclass, and `NodeContext` class (seven attributes assigned directly in `__init__`). The module docstring and all type signatures match `ANVILML_DESIGN.md §14.5` verbatim.

`worker/tests/test_base.py` (301 lines) contains 12 tests covering `SlotSpec` defaults, `@register` success and six missing-attribute cases, `@register` return identity, and `NodeContext` attribute assignment with `mock=True`, `mock=False`, and arbitrary `caps`. Tests follow a consistent style: Google-style docstrings describing what is verified and why, `base` module imports, no fixtures from conftest (conftest.py is empty), and manual cleanup of `NODE_REGISTRY` entries after registration tests.

The gap between design doc and current source is minimal: `ANVILML_DESIGN.md §14.5` shows the module docstring and all existing types but does not explicitly render the `BaseNode` class definition — the task context fills this gap with the exact signature. No discrepancy exists that would affect the approach.

## Resolved Dependencies

None. This task introduces no external crates or packages. It uses only Python's standard library `abc` module, which is already imported at line 4 of `base.py`.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) | —    | —               | —          | —                      |

## Approach

1. **Add `BaseNode` class to `worker/nodes/base.py`** (after the existing `NodeContext` class, before the file's implicit end). The class inherits from `ABC` and declares one abstract method:

   ```python
   class BaseNode(ABC):
       """Abstract base class for all node types.

       Subclasses must implement execute(). Direct instantiation is
       prevented by Python's ABC machinery.
       """

       @abstractmethod
       def execute(self, ctx: NodeContext, **inputs) -> dict:
           """Execute this node's computation.

           Subclasses override this method to perform inference or
           data transformation. The base class provides no default
           implementation — a subclass missing this method cannot be
           instantiated.

           Args:
               ctx: Runtime context carrying job_id, device, caps,
                   cancel_flag, emit, pipeline_cache, and mock flag.
               **inputs: Named input values keyed by slot name,
                   matching the node's INPUT_SLOTS.

           Returns:
               Dict of output values keyed by slot name,
               matching the node's OUTPUT_SLOTS.
           """
           ...
   ```

   Rationale: The method body is `...` (Ellipsis), which is the standard Python convention for abstract method stubs. Unlike `raise NotImplementedError`, it does not produce a runtime error if accidentally called — it simply signals "this is abstract" at the type level. ABC semantics prevent instantiation of incomplete subclasses regardless.

   Rationale for docstring placement: Per `FORGE_AGENT_RULES.md §12.1`, every non-trivial function must have a docstring. The abstract method's docstring documents the contract that all concrete implementations must honour, which is valuable for both human readers and type checkers.

2. **Add three tests to `worker/tests/test_base.py`** (appended after the existing 12 tests, following the established style):

   a. **`test_base_node_cannot_be_instantiated`** — Asserts that `base.BaseNode()` raises `TypeError`. This is the simplest ABC test: no subclass, no mocking, just direct instantiation attempt. Docstring explains that Python's ABC machinery enforces this, not custom code.

   b. **`test_concrete_subclass_instantiates`** — Defines a minimal concrete subclass of `BaseNode` with a trivial `execute()` that returns `{}`, asserts that instantiation succeeds without error. Docstring explains that this confirms the abstract method requirement is satisfied by providing `execute()`.

   c. **`test_execute_calls_subclass_impl`** — Defines a concrete subclass with an `execute()` method that sets a module-level or instance-level flag (e.g., `self.called = True`), instantiates it, calls `execute()`, and asserts the flag is True. This proves the subclass's implementation runs, not a base no-op. Docstring explains this guards against a future regression where a base no-op is accidentally called.

   All three tests follow the existing pattern: Google-style docstrings, `from worker.nodes import base` import (already present at module level), no fixtures, self-contained assertions.

3. **Verify** by running `python -m pytest worker/tests/test_base.py -v` — must exit 0 with >=15 tests total (12 existing + 3 new).

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `class BaseNode` | `worker/nodes/base.py` | `class BaseNode(ABC)` |
| `BaseNode.execute` | `worker/nodes/base.py` | `@abstractmethod def execute(self, ctx: NodeContext, **inputs) -> dict` |

No new `pub` (Python `def` at module level) items are added — `BaseNode` is a class, not a function. The `execute` method is not `pub` in the Python sense (no leading underscore convention change); it's an instance method on the ABC.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/base.py` | Append `BaseNode(ABC)` class with abstract `execute()` method (~25 lines) |
| MODIFY | `worker/tests/test_base.py` | Append 3 new test functions (~45 lines) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_base.py` | `test_base_node_cannot_be_instantiated` | `BaseNode()` raises `TypeError` per ABC semantics — confirms the abstract base class cannot be directly instantiated | `python -m pytest worker/tests/test_base.py -v` exits 0 |
| `worker/tests/test_base.py` | `test_concrete_subclass_instantiates` | A minimal concrete subclass implementing `execute()` instantiates without error — confirms the abstract method requirement is satisfied | `python -m pytest worker/tests/test_base.py -v` exits 0 |
| `worker/tests/test_base.py` | `test_execute_calls_subclass_impl` | Calling `execute()` on a concrete subclass invokes the subclass's own implementation, not a base no-op — guards against future regression | `python -m pytest worker/tests/test_base.py -v` exits 0 |

## CI Impact

No CI changes required. The new tests are in an existing test file (`test_base.py`) that is already collected by `pytest worker/tests/`. The mock-mode and real-mode CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) all run `pytest worker/tests/` and will pick up the new tests automatically. No new markers, file types, or test modules are introduced.

## Platform Considerations

None identified. `ABC` and `abstractmethod` are standard library modules (`abc`) that behave identically on Linux, Windows, and all Python 3.12.x platforms. No `#[cfg(...)]` guards, path separators, or line-ending handling are relevant. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `@abstractmethod` decorator on `execute` could conflict with the `@register` decorator if a future task decorates a subclass that inherits from `BaseNode` — Python's MRO would resolve `@abstractmethod` before `@register` sees the class, and `hasattr(cls, "execute")` in `@register` would return True even if the subclass's `execute` is still abstract. | Low | Medium | This is a future-task concern, not this task's concern. The current task only adds the ABC; no subclass exists yet to be decorated. Document this as a known interaction in the docstring. |
| The new tests could fail if the existing test runner environment does not have Python 3.12.x or has a non-standard `abc` module. | Very Low | High | The project requires Python 3.12.x (ENVIRONMENT.md §1). The `abc` module is standard library. No mitigation needed — a failure here indicates an environment defect, not a code defect. |
| Adding tests increases the test file from 301 to ~346 lines, approaching the 350-line Python source threshold in ARCHITECTURE.md §11. | Low | Low | 346 < 350. Even with generous formatting, the file stays under threshold. If it approaches the limit in a future task, extract BaseNode-specific tests to a new `test_base_node.py` file. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/base.py` exits 0
- [ ] `python -m py_compile worker/tests/test_base.py` exits 0
- [ ] `python -m pytest worker/tests/test_base.py -v` exits 0 with >=15 tests collected
- [ ] `grep -c "class BaseNode" worker/nodes/base.py` returns exactly 1
- [ ] `grep -c "abstractmethod" worker/nodes/base.py` returns >=1
- [ ] `grep -c "def execute" worker/nodes/base.py` returns >=1
