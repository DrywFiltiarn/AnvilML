# Plan Report: P17-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P17-B1                                            |
| Phase       | 17 — Cancellation                                 |
| Description | worker/executor.py: topological sort of node graph |
| Depends on  | P9-B1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-07-11T09:00:00Z                             |
| Attempt     | 1                                                 |

## Objective

Create `worker/executor.py` containing the `topo_sort(graph: dict) -> list[dict]` function, which performs a Kahn's-algorithm topological ordering of a job graph's nodes by their edge dependencies. This is the first real graph-execution module in the AnvilML Python worker — prior to Phase 17, `PassThrough` (Phase 14) was invoked directly without any topological sort, since a single-node graph needs no ordering. The function returns nodes in a valid topological order where every node appears after all nodes it depends on. Acceptance: ≥4 tests in `worker/tests/test_executor.py` covering a linear chain, a graph with parallel branches (valid ordering), a single-node graph, and a cycle detection case; `python -m pytest worker/tests/test_executor.py -v` exits 0.

## Scope

### In Scope
- Create `worker/executor.py` with a single public function `topo_sort(graph: dict) -> list[dict]`
- Implement Kahn's algorithm for topological ordering of the graph's `"nodes"` array, using the `"edges"` array (format: `"from": "node_id:slot_name"`, `"to": "node_id:slot_name"`) to build the adjacency list and compute in-degrees
- Return an empty list `[]` when the graph has no nodes (graceful handling of edge case)
- Raise `ValueError` when the graph contains a cycle (nodes remain after all zero-in-degree nodes are processed) — this is the same signal the Rust `dag.rs` uses for check 6
- Create `worker/tests/test_executor.py` with ≥4 tests: linear chain, parallel branches, single-node, and cycle detection
- Google-style docstrings on the public function

### Out of Scope
- The execution loop (`execute_graph`) that calls each node's `execute()` method — deferred to P17-B2 per its `defers_to` field
- `cancel_flag` checking between node steps — deferred to P17-B2
- Wiring `topo_sort()` into `worker_main.py`'s dispatch loop — deferred to P17-B3
- Graph validation (duplicate IDs, unknown types, dangling edges, slot-type mismatches) — already handled by Rust-side `validate_graph()` in `anvilml-scheduler` (Phase 12); `topo_sort()` only needs to work with a graph that has already been validated
- Any dependency on `NodeContext`, `NODE_REGISTRY`, or node classes — `topo_sort()` operates purely on the raw graph dict

## Existing Codebase Assessment

No prior source exists for `executor.py` — it is being created for the first time in this phase. The project's graph data format is already established: the Rust-side `anvilml-scheduler/src/dag.rs` (480 lines) implements the full validation pipeline including Kahn's algorithm (check 6, lines 361–473). The graph dict shape is `{"nodes": [{"id": str, "type": str, "inputs": dict}], "edges": [{"from": "node_id:slot_name", "to": "node_id:slot_name"}]}`.

The established Python patterns in `worker/tests/` are:
- One test file per source module (e.g. `test_ipc.py`, `test_passthrough.py`)
- Helper functions like `_make_ctx()` for constructing test fixtures
- Google-style docstrings on every test function describing the behaviour, preconditions, and expected outcome
- Tests that import from `worker.nodes.base` for `NodeContext` and `SlotSpec`
- `conftest.py` exists but is empty (no shared fixtures needed yet)
- `pyproject.toml` registers the `real_mode` marker

The test style uses plain `assert` statements without pytest fixtures. Test functions are plain `def` with no class wrappers. The `_make_ctx()` helper pattern (from `test_passthrough.py`) shows the convention for constructing minimal test inputs.

## Resolved Dependencies

None. `topo_sort()` uses only Python stdlib (`collections.defaultdict`, `collections.deque`). No new external packages or crates are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (none) | — | — | — | — |

## Approach

1. **Create `worker/executor.py`** with a module docstring and the `topo_sort(graph: dict) -> list[dict]` function.

   The function accepts a graph dict with the shape `{"nodes": [...], "edges": [...]}`. It builds a directed adjacency list from the `"edges"` array, computes in-degrees for all nodes, then runs Kahn's algorithm:
   - Parse every `"from"` and `"to"` edge string (format: `"node_id:slot_name"`) to extract source and destination node IDs
   - Build `adjacency: dict[str, list[str]]` and `in_degree: dict[str, int]`
   - Initialize the processing queue with all nodes having in-degree 0
   - Process the queue: pop a node, append to result, decrement in-degrees of neighbors, enqueue any reaching 0
   - After processing, if `len(result) < len(nodes)`, a cycle exists — raise `ValueError` with the remaining node IDs
   - Return the result list (nodes in valid topological order)

   The adjacency list construction only uses `"from"` and `"to"` fields from edges — it does not parse slot names (those are handled by prior validation in `dag.rs`). The function iterates `"edges"` only if the key exists (graceful handling of graphs without an `"edges"` array).

   Docstring follows Google style with `Args:`, `Returns:`, and `Raises:` sections.

2. **Create `worker/tests/test_executor.py`** with ≥4 tests:

   - `test_topo_sort_single_node`: A graph with one node and no edges returns that node in a list.
   - `test_topo_sort_linear_chain`: A→B→C chain returns `[A, B, C]` (deterministic order since each node has exactly one predecessor).
   - `test_topo_sort_parallel_branches`: A graph where A fans out to B and C (both depend only on A) returns a valid ordering where A comes before both B and C. Since Kahn's algorithm processes nodes in insertion order when multiple have in-degree 0, the test asserts `result.index("A") < result.index("B")` and `result.index("A") < result.index("C")` rather than a specific order between B and C.
   - `test_topo_sort_cycle_detected`: A graph with a cycle (A→B→C→A) raises `ValueError`. The test asserts the exception is raised and that the error message contains the cycle node IDs.

   Each test function has a Google-style docstring. Tests use minimal node dicts with only `"id"`, `"type"`, and `"inputs"` keys — no `NodeContext` or registry needed since `topo_sort()` operates on raw graph data.

3. **Verify syntax** by running `python -m py_compile worker/executor.py worker/tests/test_executor.py` before running tests (per ENVIRONMENT.md §7, mandatory pre-test check for Python files).

4. **Run acceptance test**: `python -m pytest worker/tests/test_executor.py -v` — must exit 0 with ≥4 tests passing.

## Public API Surface

| Module | Item | Signature | Description |
|--------|------|-----------|-------------|
| `worker.executor` | `topo_sort` | `def topo_sort(graph: dict) -> list[dict]` | Perform Kahn's-algorithm topological sort on a job graph dict. Returns nodes in dependency order. Raises `ValueError` on cycle. |

No `pub` items in the Python sense — this is a module-level function. No class, no re-export needed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/executor.py` | New module with `topo_sort()` function |
| CREATE | `worker/tests/test_executor.py` | New test file with ≥4 tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_executor.py` | `test_topo_sort_single_node` | A graph with one node and no edges returns `[node]` | `python -m pytest worker/tests/test_executor.py::test_topo_sort_single_node -v` exits 0 |
| `worker/tests/test_executor.py` | `test_topo_sort_linear_chain` | A→B→C chain returns nodes in correct dependency order | `python -m pytest worker/tests/test_executor.py::test_topo_sort_linear_chain -v` exits 0 |
| `worker/tests/test_executor.py` | `test_topo_sort_parallel_branches` | A graph with parallel branches produces a valid topological order (A before B and C) | `python -m pytest worker/tests/test_executor.py::test_topo_sort_parallel_branches -v` exits 0 |
| `worker/tests/test_executor.py` | `test_topo_sort_cycle_detected` | A cyclic graph raises `ValueError` with cycle node IDs in the message | `python -m pytest worker/tests/test_executor.py::test_topo_sort_cycle_detected -v` exits 0 |

## CI Impact

No CI changes required. The new test file lives under `worker/tests/` which is already covered by the existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`). No new file types, no new gate, no new CI configuration needed.

## Platform Considerations

None identified. The topological sort is a pure data transformation with no platform-specific code, no file I/O, no path handling, and no subprocess spawning. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Edge format ambiguity — the `"from"` field is `"node_id:slot_name"` (e.g. `"load_model_0:MODEL"`) but `topo_sort()` only needs node IDs. A naive split on `:` would fail if a node ID contains a colon. | Low | Medium | Use `splitn(2, ':')` (same pattern as Rust `dag.rs` line 178) to split on the first colon only, extracting the node ID before the colon and discarding the slot name after. |
| Graph without `"edges"` key — a single-node graph or a graph that was validated but has no edges will not have an `"edges"` key. | Low | Low | Check `if "edges" in graph and isinstance(graph["edges"], list)` before building the adjacency list. Nodes with no incoming edges simply start with in-degree 0. |
| Graph without `"nodes"` key — malformed input could lack the `"nodes"` array entirely. | Low | Low | Return `[]` early if `"nodes"` is missing or not a list. This is a graceful degradation; the Rust validator should have caught this before the graph reaches the worker. |
| Test ordering non-determinism — when multiple nodes have in-degree 0 simultaneously, the order depends on dict iteration order (Python 3.7+ guarantees insertion order, but the order of which zero-in-degree node is enqueued first depends on how the adjacency list was built). | Low | Low | The parallel-branches test asserts a partial order (`A before B` and `A before C`) rather than a total order, matching the fact that any valid topological ordering is acceptable. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/executor.py` exits 0
- [ ] `python -m py_compile worker/tests/test_executor.py` exits 0
- [ ] `python -m pytest worker/tests/test_executor.py -v` exits 0 with ≥4 tests passing
- [ ] `test_topo_sort_single_node` passes: single-node graph returns `[node]`
- [ ] `test_topo_sort_linear_chain` passes: A→B→C returns `[A, B, C]`
- [ ] `test_topo_sort_parallel_branches` passes: A→{B,C} returns A before both B and C
- [ ] `test_topo_sort_cycle_detected` passes: cyclic graph raises `ValueError`
- [ ] `worker/executor.py` contains a module-level docstring
- [ ] `topo_sort()` has a Google-style docstring with Args, Returns, and Raises sections
