# Plan Report: P13-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-A4                                            |
| Phase       | 013 — Dispatch & Execute                          |
| Description | worker: mock executor returning Completed (no image yet) |
| Depends on  | P13-A3                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-09T12:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add handling of the `Execute` IPC message in `worker/worker_main.py` under mock mode. When the worker receives `Execute{job_id, graph, settings, device_index}`, it parses the `graph['nodes']` list and iterates over each node in the given order, emitting a `Progress{node_index, node_total, node_type}` event for each node. After all nodes are processed, it emits `Completed{job_id, elapsed_ms}`. No image or artifact emission (deferred to phase 14).

## Scope

### In Scope
- Add `Execute` message handler in the main message loop of `worker/worker_main.py`
- Parse `graph['nodes']` from the Execute payload
- Iterate nodes in the order given (no topological sort in mock — the DAG is already validated by the server, P13-A6 verifies correctness)
- Emit `Progress{job_id, node_index, node_total, node_type}` for each node
- Record start time, compute elapsed_ms, emit `Completed{job_id, elapsed_ms}` after the last node
- Add a test in `worker/tests/test_worker_main.py` that sends an Execute frame and verifies Progress + Completed events

### Out of Scope
- Topological sort of nodes (deferred; server validates DAG already)
- Image/artifact emission (`ImageReady`) — phase 14
- Cancel handling during execution — phase 14/22
- Real hardware execution path (non-mock) — phase 21
- Node-level execution logic (actual ML inference) — phase 21
- `executor.py` creation — out of scope for this task (mock mode skips it)

## Approach

1. **Add `_execute_mock` function** in `worker/worker_main.py` (after `_probe_hardware`, before the message loop):
   - Accepts `job_id` (str/Uuid), `graph` (dict with `nodes` key), `settings` (dict), `device_index` (int)
   - Records `start_time = time.monotonic()`
   - Iterates `graph['nodes']` with `enumerate`:
     - For each node at index `i` with total `len(graph['nodes'])`:
       - Extract `node_type = node.get('type', 'unknown')`
       - Emit `Progress{job_id, node_index=i, node_total=len(nodes), node_type=node_type}`
     - No sleep or delay needed (mock is instant)
   - Computes `elapsed_ms = int((time.monotonic() - start_time) * 1000)`
   - Emits `Completed{job_id, elapsed_ms=elapsed_ms}`

2. **Add Execute handler** in the main message loop (after `MemoryQuery`, before `Shutdown`):
   - Match `_type == "Execute"`
   - Extract `job_id`, `graph`, `settings`, `device_index` from the message
   - Call `_execute_mock(job_id, graph, settings, device_index)`
   - Continue to next iteration

3. **Add test** in `worker/tests/test_worker_main.py`:
   - `test_execute_progress_completed`: spawn worker, send `InitializeHardware`, send `Execute{job_id, graph, settings, device_index}`, send `Shutdown`
   - Parse stdout frames; assert:
     - Exactly one `Ready` event with correct worker_id
     - `N` `Progress` events (where N = number of nodes in graph), each with correct `node_index`, `node_total`, `node_type`
     - Exactly one `Completed` event with matching `job_id` and `elapsed_ms >= 0`
     - Exactly one `Dying` event, process exit code 0

4. **Verify**: Run `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` and confirm all tests pass.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Add `_execute_mock()` function and Execute handler in message loop |
| Modify | `worker/tests/test_worker_main.py` | Add `test_execute_progress_completed` test |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_worker_main.py` | `test_execute_progress_completed` | Execute → N Progress events → Completed; correct indices, totals, types, elapsed_ms |

## CI Impact

The Python worker test suite (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`) is already part of CI (ARCHITECTURE.md §9, ENVIRONMENT.md §6). Adding a new test to `test_worker_main.py` extends this suite — no CI workflow changes are required. The new test follows the existing pattern of spawning the worker subprocess and reading framed msgpack from stdout.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `graph['nodes']` key missing from Execute payload | Low | Worker crashes on KeyError | Guard with `.get('nodes', [])`; emit Completed immediately with zero Progress events |
| `node_type` field missing from a node dict | Low | Crash on `.get('type')` | Use `node.get('type', 'unknown')` — defensive default |
| Test timeout (worker hangs after Execute) | Low | Test fails | Ensure `_execute_mock` always emits Completed before the loop returns; no blocking calls |
| `elapsed_ms` is 0 due to fast mock execution | Low | Test assertion issue | Assert `elapsed_ms >= 0` rather than `> 0` |

## Acceptance Criteria

- [ ] `worker/worker_main.py` handles `Execute{_type}` in the message loop under mock mode
- [ ] For a graph with N nodes, exactly N `Progress` events are emitted with correct `node_index` (0..N-1), `node_total` (N), and `node_type`
- [ ] A single `Completed{job_id, elapsed_ms}` event is emitted after all Progress events
- [ ] New test `test_execute_progress_completed` passes under `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v`
- [ ] All existing tests in `test_worker_main.py` continue to pass (no regression)
