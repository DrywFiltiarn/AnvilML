# Plan Report: P17-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-B2                                      |
| Phase       | 17 — Cancellation                           |
| Description | worker/executor.py: execute_graph loop with cancel_flag check |
| Depends on  | P17-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-11T12:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `worker/executor.py` by adding `execute_graph(graph: dict, ctx_factory) -> dict`, which uses `topo_sort()` (from P17-B1) to order nodes, then executes each node in topological order while checking `ctx.cancel_flag.is_set()` before every node's `execute()` call. On cancellation, the function returns early with a result dict indicating `{"cancelled": True}`. This enables the cooperative cancellation chain: the scheduler sends `CancelJob` via IPC, which sets the `cancel_flag` on the `NodeContext`, and the executor checks it between steps.

## Scope

### In Scope
- Add `execute_graph(graph: dict, ctx_factory) -> dict` to `worker/executor.py`.
- The function accepts a graph dict (same shape as `topo_sort()`'s input) and a callable `ctx_factory` that takes `(job_id, emit)` and returns a `NodeContext`.
- Uses `topo_sort()` to produce the execution order.
- Before executing each node, checks `ctx.cancel_flag.is_set()`. If set, stops immediately and returns `{"cancelled": True}`.
- On normal completion (all nodes executed without cancellation), returns `{"cancelled": False}`.
- Each node is instantiated via `NODE_REGISTRY[node["type"]]`, called with the context and the node's `inputs`.
- The function captures node outputs in a `results` dict keyed by node ID.
- Add >=5 tests in `worker/tests/test_executor.py` covering: (1) cancel_flag set before any node runs skips all execution, (2) cancel_flag set after the first node's execute() but before the second stops there, (3) a graph with no cancel_flag set runs to completion, (4) execution order matches `topo_sort()`, (5) outputs dict is populated correctly.
- Total test count in `test_executor.py` reaches >=9.

### Out of Scope
None. This task's `defers_to` is `[]` (empty). No scope is deferred.

## Existing Codebase Assessment

The codebase already has `worker/executor.py` with a complete `topo_sort()` implementation (P17-B1), using Kahn's algorithm. It also has `worker/nodes/base.py` defining `NodeContext` (with `cancel_flag` as a `threading.Event`), `BaseNode` ABC, `@register` decorator, and `NODE_REGISTRY`. The `PassThrough` node exists in `worker/nodes/passthrough.py` as a simple no-op that returns its input value.

The existing test file `worker/tests/test_executor.py` has 8 tests covering `topo_sort()` alone. The tests follow a consistent pattern: helper function `_make_graph()` for graph construction, assertions on result ordering, and a subprocess-based import isolation test. The project uses Google-style docstrings for Python functions.

The `NodeContext` class already carries `cancel_flag` as a `threading.Event` attribute — this was established in Phase 10 (P10-A3). The `cancel_flag` semantics are documented in `ANVILML_DESIGN.md §14.5`: it is checked cooperatively between node steps, never interrupting a node mid-`execute()`.

No new external dependencies are needed. The function uses only `topo_sort` (stdlib `collections`), `NODE_REGISTRY` (from `worker.nodes.base`), and standard Python primitives.

## Resolved Dependencies

None. This task uses only Python stdlib (`threading.Event` is already used by `NodeContext` in `base.py` and the interim `_execute_job()` in `worker_main.py`). No new crates or packages are introduced.

## Approach

1. **Add `execute_graph()` function to `worker/executor.py`.**

   Import `NODE_REGISTRY` from `worker.nodes.base` and `NodeContext` inside the function body (not at module level, following the established pattern in `worker_main.py` where `worker.ipc` is also imported inside functions to avoid transitive torch dependencies).

   Function signature: `def execute_graph(graph: dict, ctx_factory) -> dict:`

   Implementation steps:
   a. Call `topo_sort(graph)` to get nodes in dependency order. Log at DEBUG level the number of nodes being executed.
   b. Initialize an empty `results = {}` dict to accumulate node outputs keyed by node ID.
   c. Create the context by calling `ctx_factory` — this factory (produced by `P17-B3`'s caller) will construct a `NodeContext` with a `threading.Event()` as the `cancel_flag`.
   d. Iterate over each node in the sorted list:
      - **Before** calling `node_instance.execute()`, check `ctx.cancel_flag.is_set()`. If set, return `{"cancelled": True}` immediately. This is the cooperative cancellation checkpoint.
      - Instantiate the node class from `NODE_REGISTRY[node["type"]]`.
      - Call `node_instance.execute(ctx, **node.get("inputs", {}))`.
      - Store the result: `results[node["id"]] = node_output`.
   e. After the loop completes normally (no cancellation), return `{"cancelled": False}`.

2. **Add tests to `worker/tests/test_executor.py`.**

   Create a mock `NodeContext` for testing that does not import `torch` or any node module. The mock context needs:
   - `cancel_flag`: a `threading.Event()`
   - `emit`: a simple lambda that stores calls (no IPC needed)
   - Other fields (`job_id`, `device`, `caps`, `pipeline_cache`, `mock`) with dummy values

   Create a mock node class that:
   - Implements `execute(ctx, **inputs)` and returns `{"output": inputs.get("value", None)}`
   - Can be registered in a local test-time `NODE_REGISTRY` or the test can use a minimal approach where the node class is a simple callable

   Test cases (5 minimum):
   a. `test_execute_graph_cancel_before_first`: cancel_flag is set before the loop starts. Asserts `execute_graph()` returns `{"cancelled": True}` and no node's `execute()` was called.
   b. `test_execute_graph_cancel_after_first`: Two-node graph. cancel_flag is set inside the first node's `execute()` (via a mutable flag shared with the test). Asserts the first node ran, the second did not, and result is `{"cancelled": True}`.
   c. `test_execute_graph_no_cancel_completes`: No cancel_flag set. Asserts all nodes execute in order, outputs are collected, and result is `{"cancelled": False}`.
   d. `test_execute_graph_execution_order_matches_topo_sort`: Graph with a known topological order. Asserts the actual execution order (tracked via a list) matches the `topo_sort()` output order.
   e. `test_execute_graph_results_dict`: Asserts the returned dict contains outputs keyed by node ID for all executed nodes.

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| `execute_graph` | `worker/executor.py` | `def execute_graph(graph: dict, ctx_factory) -> dict:` |

The function is `pub` in the Python module sense: it is importable by other modules in `worker/` (specifically `worker_main.py` in P17-B3). No new types or classes are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/executor.py` | Add `execute_graph()` function with cancel-flag checkpoint loop |
| MODIFY | `worker/tests/test_executor.py` | Add >=5 tests for `execute_graph()` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_executor.py` | `test_execute_graph_cancel_before_first` | cancel_flag set before any node runs → returns `{"cancelled": True}`, no nodes executed | `python -m pytest worker/tests/test_executor.py::test_execute_graph_cancel_before_first -v` |
| `worker/tests/test_executor.py` | `test_execute_graph_cancel_after_first` | cancel_flag set mid-execution → first node runs, second skipped, returns `{"cancelled": True}` | `python -m pytest worker/tests/test_executor.py::test_execute_graph_cancel_after_first -v` |
| `worker/tests/test_executor.py` | `test_execute_graph_no_cancel_completes` | No cancel_flag → all nodes execute, returns `{"cancelled": False}` | `python -m pytest worker/tests/test_executor.py::test_execute_graph_no_cancel_completes -v` |
| `worker/tests/test_executor.py` | `test_execute_graph_execution_order_matches_topo_sort` | Execution order matches `topo_sort()` output | `python -m pytest worker/tests/test_executor.py::test_execute_graph_execution_order_matches_topo_sort -v` |
| `worker/tests/test_executor.py` | `test_execute_graph_results_dict` | Output dict is populated correctly with node results keyed by ID | `python -m pytest worker/tests/test_executor.py::test_execute_graph_results_dict -v` |

Acceptance command: `python -m pytest worker/tests/test_executor.py -v` — exits 0 with >=9 total tests (8 existing + 5 new = 13, but only 9 are required).

## CI Impact

No CI changes required. The new tests are in the existing `worker/tests/test_executor.py` file, which is already picked up by the mock-mode test command (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v -m "not real_mode"`) and the real-mode test command (`python -m pytest worker/tests/ -v -m real_mode`). Since `execute_graph()` does not import `torch` at module level (imports are inside the function body), it will be collected by both mock and real CI jobs without issue.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The function uses only Python stdlib (`threading.Event`, `dict`, `list`) — no platform-specific APIs, no path handling, no line-ending concerns.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `NODE_REGISTRY` import from `worker.nodes.base` may transitively import `torch` at collection time in mock-mode CI. | Low | Medium | Import `NODE_REGISTRY` inside the function body (not at module level), following the established pattern in `worker_main.py`. The test will also import nodes inside test functions, not at module level. |
| The mock node used in tests may conflict with real nodes registered in `NODE_REGISTRY` (e.g., `PassThrough`). | Low | Low | Use a separate, isolated registry for tests or use a mock class that doesn't register itself. The test helper function can create a minimal dict-based registry per-test. |
| `ctx_factory` signature mismatch with what P17-B3 will pass. | Medium | High | The plan specifies `ctx_factory` as a callable that returns a `NodeContext`. The ACT agent should confirm the exact signature from P17-B3's context before writing, but the function accepts any callable — no hard dependency on a specific signature. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/executor.py` exits 0
- [ ] `python -m py_compile worker/tests/test_executor.py` exits 0
- [ ] `python -m pytest worker/tests/test_executor.py -v` exits 0 with >=9 total tests
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_executor.py -v -m "not real_mode"` exits 0
- [ ] `python -m pytest worker/tests/test_executor.py -v -m real_mode` exits 0
