# Implementation Report: P17-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P17-B1                                            |
| Phase       | 17 — Cancellation                                 |
| Description | worker/executor.py: topological sort of node graph |
| Implemented | 2026-07-11T12:00:00Z                             |
| Status      | COMPLETE                                          |

## Summary

Created `worker/executor.py` containing the `topo_sort(graph: dict) -> list[dict]` function, which performs a Kahn's-algorithm topological ordering of a job graph's nodes by their edge dependencies. Implemented 8 tests in `worker/tests/test_executor.py` covering single-node graphs, linear chains, parallel branches, cycle detection, edge-case handling (missing edges key, empty graph, missing nodes key), and torch-import isolation. All 8 tests pass. Updated `docs/TESTS.md` with catalogue entries for all 8 tests.

## Resolved Dependencies

None. `topo_sort()` uses only Python stdlib (`collections.defaultdict`, `collections.deque`). No new external packages or crates are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/executor.py` | New module with `topo_sort()` function (116 lines) |
| CREATE | `worker/tests/test_executor.py` | New test file with 8 tests (206 lines) |
| MODIFY | `docs/TESTS.md` | Added 8 test catalogue entries for new tests |

## Commit Log

```
 .forge/reports/P17-B1_plan.md | 136 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 +--
 docs/TESTS.md                 |  96 ++++++++++++++++++++
 worker/executor.py            | 116 ++++++++++++++++++++++++
 worker/tests/test_executor.py | 206 ++++++++++++++++++++++++++++++++++++++++++
 6 files changed, 564 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 8 items

worker/tests/test_executor.py::test_topo_sort_single_node PASSED         [ 12%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 25%]
worker/tests/test_executor.py::test_topo_sort_parallel_branches PASSED   [ 37%]
worker/tests/test_executor.py::test_topo_sort_cycle_detected PASSED      [ 50%]
worker/tests/test_executor.py::test_topo_sort_no_edges_key PASSED        [ 62%]
worker/tests/test_executor.py::test_topo_sort_empty_graph PASSED         [ 75%]
worker/tests/test_executor.py::test_topo_sort_missing_nodes_key PASSED   [ 87%]
worker/tests/test_executor.py::test_topo_sort_no_torch_import PASSED     [100%]

============================== 8 passed in 0.06s ===============================
```

## Format Gate

```
(No output — `cargo fmt --all -- --check` exited 0)
```

## Platform Cross-Check

```
(No Rust source files modified — no cross-check commands needed)
```

## Project Gates

None defined for Python-only changes. No config, OpenAPI, or node parity gates triggered.

## Public API Delta

No new `pub` items (Python does not use `pub`). The module exposes one public function:

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `topo_sort` | `def` | `worker.executor.topo_sort` | Kahn's-algorithm topological sort on a job graph dict |

## Deviations from Plan

- **Additional tests beyond the 4 minimum:** Implemented 8 tests (4 required + 4 additional edge-case tests: `test_topo_sort_no_edges_key`, `test_topo_sort_empty_graph`, `test_topo_sort_missing_nodes_key`, `test_topo_sort_no_torch_import`). These provide broader coverage of the graceful degradation paths (missing `edges` key, empty graph, missing `nodes` key, no torch import) that the plan's risk table identified.
- **`_position()` helper function:** Added a local helper function in the test file to find a node's position by its `"id"` field, because `list.index()` with a plain `{"id": "A"}` dict fails when the result contains dicts with additional keys (`"type"`, `"inputs"`). This is a test-only helper, not part of the production API.

## Blockers

None.
