# Implementation Report: P10-A4

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P10-A4                                      |
| Phase         | 10 — Generic Node Groundwork                |
| Description   | worker/nodes/base.py: BaseNode ABC abstract execute() |
| Implemented   | 2026-07-06T01:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Added the `BaseNode` abstract base class with an abstract `execute()` method to `worker/nodes/base.py`, completing the Group A base contract. Three new tests in `worker/tests/test_base.py` confirm the ABC semantics: direct instantiation raises `TypeError`, a minimal concrete subclass with `execute()` instantiates successfully, and calling `execute()` on the concrete subclass invokes the subclass's implementation. The `docs/TESTS.md` catalogue was updated with entries for all three new tests. All 18 tests in `test_base.py` pass (12 existing + 3 new + 3 additional from the test suite).

## Resolved Dependencies

None. This task introduces no external crates or packages. It uses only Python's standard library `abc` module (`ABC`, `abstractmethod`), which is already imported at line 4 of `base.py`.

| Type   | Name | Version resolved | Source         |
|--------|------|-----------------|----------------|
| (none) | —    | —               | —              |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/base.py` | Appended `BaseNode(ABC)` class with abstract `execute()` method (~25 lines) |
| MODIFY | `worker/tests/test_base.py` | Appended 3 new test functions: `test_base_node_cannot_be_instantiated`, `test_concrete_subclass_instantiates`, `test_execute_calls_subclass_impl` (~54 lines) |
| MODIFY | `docs/TESTS.md` | Added 3 new test catalogue entries for the new tests (~36 lines) |

## Commit Log

```
 .forge/reports/P10-A4_plan.md | 142 ++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 ++--
 docs/TESTS.md                 |  36 +++++++++++
 worker/nodes/base.py          |  29 +++++++++
 worker/tests/test_base.py     |  54 ++++++++++++++++
 6 files changed, 271 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 18 items

worker/tests/test_base.py::test_node_registry_starts_empty PASSED        [  5%]
worker/tests/test_base.py::test_slotspec_optional_defaults_to_false PASSED [ 11%]
worker/tests/test_base.py::test_slotspec_accepts_explicit_optional_true PASSED [ 16%]
worker/tests/test_base.py::test_register_success PASSED                  [ 22%]
worker/tests/test_base.py::test_register_missing_NODE_TYPE PASSED        [ 27%]
worker/tests/test_base.py::test_register_missing_CATEGORY PASSED         [ 33%]
worker/tests/test_base.py::test_register_missing_DISPLAY_NAME PASSED         [ 38%]
worker/tests/test_base.py::test_register_missing_DESCRIPTION PASSED        [ 44%]
worker/tests/test_base.py::test_register_missing_INPUT_SLOTS PASSED        [ 50%]
worker/tests/test_base.py::test_register_missing_OUTPUT_SLOTS PASSED       [ 55%]
worker/tests/test_base.py::test_register_returns_class_identity PASSED   [ 61%]
worker/tests/test_base.py::test_node_context_assigns_all_attrs PASSED    [ 66%]
worker/tests/test_base.py::test_node_context_mock_true PASSED            [ 72%]
worker/tests/test_base.py::test_node_context_mock_false PASSED           [ 77%]
worker/tests/test_base.py::test_node_context_caps_accepts_arbitrary_dict PASSED [ 83%]
worker/tests/test_base.py::test_base_node_cannot_be_instantiated PASSED  [ 88%]
worker/tests/test_base.py::test_concrete_subclass_instantiates PASSED    [ 94%]
worker/tests/test_base.py::test_execute_calls_subclass_impl PASSED       [100%]

============================== 18 passed in 0.06s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.37s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.71s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.06s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.08s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
     Running tests/config_reference.rs (target/debug/deps/config_reference-55d41adf63520aa7)
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
+class BaseNode(ABC):
+    def execute(self, ctx: NodeContext, **inputs) -> dict:
```

New items:
- `class BaseNode` — Python class in `worker.nodes.base` module. Inherits from `ABC`. Provides the abstract base for all node types.
- `BaseNode.execute(self, ctx: NodeContext, **inputs) -> dict` — `@abstractmethod` in `worker.nodes.base.BaseNode`. Abstract method that all concrete node subclasses must override.

## Deviations from Plan

None. Implementation follows the approved plan exactly.

## Blockers

None.
