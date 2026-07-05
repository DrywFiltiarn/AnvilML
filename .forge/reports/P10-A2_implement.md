# Implementation Report: P10-A2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P10-A2                          |
| Phase         | 10 — Generic Node Groundwork    |
| Description   | worker/nodes/base.py: @register decorator with required-attr validation |
| Implemented   | 2026-07-05T22:45:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the `@register` decorator in `worker/nodes/base.py` per `ANVILML_DESIGN.md §14.5`. The decorator validates six required class attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`) and inserts the class into `NODE_REGISTRY` keyed by `cls.NODE_TYPE`. Returns the original class unchanged (identity preservation). Wrote 8 tests in `worker/tests/test_base.py` covering success, individual missing-attr failures for all 6 attributes, and identity preservation. All 11 tests in the file pass (3 existing from P10-A1 + 8 new).

## Resolved Dependencies

None. This task introduces no new external crates or packages. All functionality uses Python stdlib (`abc`, `dataclasses`, `typing`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/base.py` | Added `register(cls: type) -> type` decorator function per §14.5 |
| MODIFY | `worker/tests/test_base.py` | Added `_FullySpecifiedNode` helper class and 8 new tests for `register()` |
| MODIFY | `docs/TESTS.md` | Added 8 test catalogue entries for the new tests |

## Commit Log

```
 .forge/reports/P10-A2_plan.md | 134 +++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 +--
 docs/TESTS.md                 |  94 ++++++++++++++++++++++
 worker/nodes/base.py          |  18 +++++
 worker/tests/test_base.py     | 179 +++++++++++++++++++++++++++++++++++++++++-
 6 files changed, 434 insertions(+), 10 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 11 items

worker/tests/test_base.py::test_node_registry_starts_empty PASSED        [  9%]
worker/tests/test_base.py::test_slotspec_optional_defaults_to_false PASSED [ 18%]
worker/tests/test_base.py::test_slotspec_accepts_explicit_optional_true PASSED [ 27%]
worker/tests/test_base.py::test_register_success PASSED                  [ 36%]
worker/tests/test_base.py::test_register_missing_NODE_TYPE PASSED        [ 45%]
worker/tests/test_base.py::test_register_missing_CATEGORY PASSED         [ 54%]
worker/tests/test_base.py::test_register_missing_DISPLAY_NAME PASSED     [ 63%]
worker/tests/test_base.py::test_register_missing_DESCRIPTION PASSED      [ 72%]
worker/tests/test_base.py::test_register_missing_INPUT_SLOTS PASSED      [ 81%]
worker/tests/test_base.py::test_register_missing_OUTPUT_SLOTS PASSED     [ 90%]
worker/tests/test_base.py::test_register_returns_class_identity PASSED   [100%]

============================== 11 passed in 0.05s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.19s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.63s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.04s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.15s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Public API Delta

```
+def register(cls: type) -> type:
```

One new module-level function `register` in `worker.nodes.base`, matching the plan's Public API Surface table exactly. No new `pub` items in any Rust crate (no Rust source files were modified).

## Deviations from Plan

- **Test approach for missing-attr tests:** The plan suggested using `delattr` on a subclass of `_FullySpecifiedNode`. During implementation, `del BadNode.NODE_TYPE` on a child class fails with `AttributeError` because inherited class attributes cannot be deleted from the child — they remain on the parent. Fixed by using `type()` to dynamically create a class with exactly the 5 desired attributes, without the missing one. This is a more robust approach and avoids the inheritance/deletion pitfall. The test semantics are identical: each test creates a class missing exactly one required attribute and asserts `TypeError` with the correct attribute name in the message.

## Blockers

None.
