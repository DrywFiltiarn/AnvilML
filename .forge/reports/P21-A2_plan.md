# Plan Report: P21-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A2                                        |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: executor.py run_graph (topo-sort, cancel, exceptions) |
| Depends on  | P21-A1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-12T18:22:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `worker/executor.py` implementing `run_graph()`: a Kahn topological-sort + node-execution loop that resolves edge references, dispatches nodes via `NODE_REGISTRY`, handles cancellation cooperatively, and emits `Progress` / `Completed` / `Failed` / `Cancelled` IPC events. Replace the inline mock loop in `worker_main.py` with a call to `run_graph`. Provide `test_executor.py` with four scenarios: valid execution, cycle detection, node exception → `Failed`, and cancellation → `Cancelled`.

## Scope

### In Scope
- `worker/executor.py`: `run_graph(graph, settings, device_str, cancel_flag, emit_fn, pipeline_cache, job_id)`
  - Kahn topological sort with cycle detection
  - Per-node: cancel check, input resolution (literals + edge refs), `NODE_REGISTRY[type]` lookup/instantiate/execute, output storage, `Progress` emit
  - Exception handling: `CancelledError` → `Cancelled`, other `Exception` → `Failed{error, traceback}`
  - Completion → `Completed{elapsed_ms}`
  - Logging per §11 conventions (INFO at lifecycle points, DEBUG for internal state)
- `worker/worker_main.py`: replace `_execute_mock` call path with `run_graph`, wiring `threading.Event` as cancel flag
- `worker/tests/test_executor.py`: four test scenarios using mock nodes

### Out of Scope
- `pipeline_cache.py` (P21-A3)
- ZiT node implementations (P21-A5)
- `defaults.py` (P21-A4)
- Rust-side changes
- CI workflow modifications

## Approach

1. **Create `worker/executor.py`** with the following structure:
   - Import `NODE_REGISTRY` from `worker.nodes.base`
   - `run_graph(graph, settings, device_str, cancel_flag, emit_fn, pipeline_cache, job_id) -> dict`:
     a. Extract `nodes` from `graph["nodes"]` (default `[]`)
     b. **Kahn topological sort**: build adjacency from edge refs in inputs; compute in-degree; BFS queue → sorted order; if `len(sorted) < len(nodes)`, emit `Failed{error:"cycle_detected"}` and return
     c. Record `start = time.monotonic()`
     d. For each node in sorted order:
        i. If `cancel_flag.is_set()`: emit `Cancelled{job_id}`, return `{"status":"cancelled"}`
        ii. Resolve inputs: for each slot value, if it's a dict with `"node_id"` and `"output_slot"`, look up `node_outputs[ref["node_id"]][ref["output_slot"]]`; otherwise pass literal through
        iii. Look up `cls = NODE_REGISTRY[node_type]`; instantiate `node = cls(ctx)` where `ctx` is a `NodeContext` with `pipeline_cache, device_str, emit_fn, cancel_flag, job_id`
        iv. Call `outputs = node.execute(**resolved_inputs)`
        v. Store `node_outputs[node_id] = outputs`
        vi. Emit `Progress{job_id, node_index, node_total, node_type}`
     e. On success: `elapsed_ms = int((time.monotonic() - start) * 1000)`; emit `Completed{job_id, elapsed_ms}`; return `{"status":"completed","elapsed_ms":elapsed_ms}`
     f. On `CancelledError`: emit `Cancelled{job_id}`; return `{"status":"cancelled"}`
     g. On other `Exception`: emit `Failed{job_id, error:str(e), traceback:traceback.format_exc()}`; return `{"status":"failed","error":..., "traceback":...}`

2. **Update `worker/worker_main.py`**:
   - Import `run_graph` from `worker.executor`
   - Replace the `_execute_mock(job_id, graph, settings, device_index, cancel_flag)` call with:
     ```python
     cancel_event = threading.Event()
     cancel_flags[job_id] = cancel_event  # reader thread sets this on CancelJob
     result = run_graph(graph, settings, device_str, cancel_event, emit_fn, pipeline_cache, job_id)
     cancel_flags.pop(job_id, None)
     ```
   - The `_message_reader_thread` already sets `cancel_flag[job_id] = True` on `CancelJob`; update it to set `cancel_event.set()` instead
   - Remove `_execute_mock` and `_generate_black_png` (mock loop functions are replaced)
   - `emit_fn` is a closure around `ipc.write_frame`

3. **Create `worker/tests/test_executor.py`** with four test classes:
   - `test_valid_graph`: mock nodes that return fixed output dicts; verify `Progress` emitted per node in topo order, then `Completed` with `elapsed_ms >= 0`, edge refs resolved correctly
   - `test_cycle_detected`: graph with circular dependency (A→B→A); verify `Failed{error:"cycle_detected"}` emitted, no `Completed`
   - `test_node_exception`: one mock node raises `RuntimeError("boom")`; verify `Failed{error:"boom", traceback}` emitted
   - `test_cancel_during_execution`: cancel flag set mid-execution; verify `Cancelled` emitted, no `Completed`, nodes after cancel not executed

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/executor.py` | `run_graph()` with Kahn topo-sort, node dispatch, cancel/exception handling |
| Modify | `worker/worker_main.py` | Replace `_execute_mock` call with `run_graph`; update cancel flag to `threading.Event` |
| Create | `worker/tests/test_executor.py` | Four test scenarios: valid, cycle, exception, cancel |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_executor.py` | `test_valid_graph` | Kahn sort order, edge ref resolution, Progress per node, Completed with elapsed_ms |
| `worker/tests/test_executor.py` | `test_cycle_detected` | Cycle detection emits Failed{error:"cycle_detected"}, no Completed |
| `worker/tests/test_executor.py` | `test_node_exception` | Node exception emits Failed{error, traceback}, no Completed |
| `worker/tests/test_executor.py` | `test_cancel_during_execution` | Cancel flag mid-execution emits Cancelled, no Completed, subsequent nodes skipped |

## CI Impact

No CI workflow file changes. The new test file is picked up automatically by `pytest worker/tests/`. The existing CI gate `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` will include the new tests. No Rust crate version bump required (Python-only changes). No config drift or OpenAPI drift.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `worker_main.py` cancel flag wiring breaks existing cancel tests | Medium | High | The cancel flag change (dict → threading.Event) is internal to worker_main; test_worker_main.py tests the full IPC loop and will catch any regression. Update the reader thread to set `cancel_event.set()` instead of `cancel_flag[job_id] = True`. |
| Edge reference resolution fails on malformed graph | Low | Medium | Kahn sort validates structure; edge refs resolved against `node_outputs` dict — missing key raises `KeyError` which is caught by the outer exception handler → `Failed`. |
| Mock nodes in tests conflict with real NODE_REGISTRY | Low | Medium | Tests use `_clear_registry` fixture (copied from test_nodes_base.py) to ensure a clean registry per test. Mock nodes are registered dynamically within each test. |
| Topo-sort cycle detection misses edge cases | Low | Low | Kahn's algorithm is deterministic: if in-degree queue empties before all nodes processed, a cycle exists. Test covers explicit 2-node cycle. |

## Acceptance Criteria

- [ ] `worker/executor.py` exists with `run_graph()` implementing Kahn topo-sort, input resolution, node dispatch, and event emission
- [ ] Cycle detection: graph with circular edge refs emits `Failed{error:"cycle_detected"}`
- [ ] Exception handling: node raising `RuntimeError` emits `Failed{error, traceback}`
- [ ] Cancellation: `cancel_flag` set mid-execution emits `Cancelled`, skips remaining nodes
- [ ] `worker/worker_main.py` calls `run_graph` instead of `_execute_mock`; cancel path uses `threading.Event`
- [ ] `worker/tests/test_executor.py` has 4 tests, all pass with `pytest worker/tests/test_executor.py -v`
- [ ] Full pytest suite passes: `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0
