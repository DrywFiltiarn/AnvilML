# Implementation Report: P14-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-A2                                        |
| Phase       | 014 — Dispatch & Mock Execute               |
| Description | anvilml-worker: mock execute in worker_main.py and executor.py |
| Implemented | 2026-06-20T12:00:00Z                         |
| Status      | COMPLETE                                      |

## Summary

Implemented the Python-side job execution pipeline for the AnvilML worker. Created `worker/executor.py` with a `run_graph()` function that topologically sorts nodes using Kahn's algorithm and executes them in dependency order. Created `worker/nodes/image.py` with a `SaveImage` node that generates a 64×64 black PNG via stdlib and emits an `ImageReady` event. Modified `worker/worker_main.py` to handle the `Execute` message type, building a `NodeContext` and calling `run_graph()` with proper error handling that sends `Completed` or `Failed` events back to the Rust supervisor. Created 8 tests in `worker/tests/test_executor.py` covering topological ordering, SaveImage PNG generation, successful execution, and node error handling.

## Resolved Dependencies

No new external dependencies were added. The implementation uses only Python stdlib (`struct`, `base64`, `zlib`, `io`, `collections`, `time`) and existing project dependencies (`msgpack`, `pyzmq`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/executor.py` | Topological-sort node executor with `run_graph()` function |
| CREATE | `worker/nodes/image.py` | `SaveImage` node with mock PNG generation |
| MODIFY | `worker/worker_main.py` | Add `time` import; add `Execute` message handler in dispatch loop |
| CREATE | `worker/tests/test_executor.py` | 8 tests for executor and SaveImage |
| MODIFY | `worker/tests/test_worker_main.py` | Update assertion for `node_types` (now contains SaveImage) |
| MODIFY | `docs/TESTS.md` | Add 8 test entries per ANVILML_DESIGN.md §16.1 |

## Commit Log

```
 .forge/reports/P14-A2_plan.md    | 151 +++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +-
 docs/TESTS.md                    |  72 +++++++
 worker/executor.py               | 215 +++++++++++++++++++
 worker/nodes/image.py            | 146 +++++++++++++
 worker/tests/test_executor.py    | 445 +++++++++++++++++++++++++++++++++++++++
 worker/tests/test_worker_main.py |   6 +-
 worker/worker_main.py            |  65 ++++++
 9 files changed, 1109 insertions(+), 10 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 24 items

worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [  4%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [  8%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [ 12%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [ 16%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 20%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 25%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED             [ 29%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED         [ 33%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 37%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 41%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 45%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 50%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 54%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 58%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 62%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 66%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 70%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 75%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 79%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 83%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 87%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED          [ 91%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED     [ 95%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [100%]

============================== 24 passed in 1.42s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows (cross-check)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
```

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
(no diff — openapi.json is up to date)
```

### Gate 3 — Node Parity
Not applicable — `worker/tests/test_parity.py` does not yet exist in the repository (planned for a later phase).

## Public API Delta

New Python public items introduced by this task:

| Item | Type | Module Path |
|------|------|-------------|
| `run_graph` | function | `worker.executor.run_graph` |
| `_topo_sort` | function | `worker.executor._topo_sort` (module-private) |
| `_resolve_input_value` | function | `worker.executor._resolve_input_value` (module-private) |
| `SaveImage` | class | `worker.nodes.image.SaveImage` |
| `_generate_black_png` | function | `worker.nodes.image._generate_black_png` (module-private) |

## Deviations from Plan

1. **Pre-existing test fix**: `test_mock_startup_sends_ready` in `worker/tests/test_worker_main.py` expected `node_types == []` but now SaveImage is registered. Updated the assertion to verify `node_types` contains exactly one entry (SaveImage). This was a necessary fix because the existing test was written before SaveImage was registered.

2. **Test helper pattern**: The `_make_node_class` helper stores the execute function in a closure-scoped variable (`_fn`) rather than as a class attribute. This avoids Python's descriptor protocol turning it into a bound method that would receive `self` as the first argument.

3. **SaveImage re-import in test**: The `test_saveimage_emits_image_ready` test uses `importlib.reload(worker.nodes.image)` after `registry_clean` to re-register SaveImage. This is necessary because the `registry_clean` fixture clears the global registry before each test.

4. **Additional tests**: Added 4 extra tests beyond the plan's 4: `test_topo_sort_cycle_detection`, `test_topo_sort_linear_chain`, `test_topo_sort_diamond`, and `test_run_graph_empty_graph`. These test the `_topo_sort` helper function and edge cases.

## Blockers

None.
