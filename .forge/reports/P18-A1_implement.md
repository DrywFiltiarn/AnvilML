# Implementation Report: P18-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P18-A1                             |
| Phase         | 018 — ZiT Generic Nodes            |
| Description   | worker/nodes/loader.py: LoadModel generic node |
| Implemented   | 2026-06-21T12:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented the `LoadModel` node in `worker/nodes/loader.py` — the first of three loader nodes for Phase 018. The node accepts a `model_id` STRING input and outputs a `MODEL` slot. In mock mode (`ANVILML_WORKER_MOCK=1`), it returns a lightweight `MockModel` sentinel with `arch="zit"`. In real mode, it raises `NotImplementedError` (stubbed for future P18-D1 pipeline_cache integration). Created 4 unit tests in `worker/tests/test_nodes_loader.py` covering registry registration, mock-mode execution, missing-input handling, and metadata attribute verification. Updated `docs/TESTS.md` with entries for all 4 new tests. Also updated `worker/tests/test_worker_main.py::test_mock_startup_sends_ready` to expect 2 registered node types (SaveImage + LoadModel) instead of 1.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | safetensors | 0.4+ (in base.txt) | pypi-query MCP |

No new external dependencies introduced. The real loading path will use `safetensors.safe_open()` and `pipeline_cache.get_or_load()` — both are existing or planned modules. The mock path uses only Python stdlib (`os`, `typing`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/loader.py` | LoadModel node with MockModel sentinel class |
| CREATE | `worker/tests/test_nodes_loader.py` | 4 unit tests for LoadModel node |
| MODIFY | `worker/tests/test_worker_main.py` | Updated `test_mock_startup_sends_ready` to expect 2 node types (SaveImage + LoadModel) |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries for new tests |

## Commit Log

```
 .forge/reports/P18-A1_plan.md     | 215 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +--
 docs/TESTS.md                     |  32 ++++++
 worker/nodes/loader.py            | 118 +++++++++++++++++++++
 worker/tests/test_nodes_loader.py | 200 +++++++++++++++++++++++++++++++++++
 worker/tests/test_worker_main.py  |  10 +-
 7 files changed, 581 insertions(+), 13 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 29 items

worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [  3%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [  6%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [ 10%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [ 13%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 17%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 20%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED             [ 24%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED         [ 27%]
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED [ 31%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 34%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 37%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 41%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 44%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 48%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 51%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 55%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 58%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 62%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 65%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 68%]
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [ 72%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 75%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 79%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 82%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 86%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 89%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED          [ 93%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED     [ 96%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [100%]

============================== 29 passed in 1.44s ==============================
```

## Format Gate

```
EXIT: 0
```

(No output — `cargo fmt --all -- --check` exited 0 with no formatting drift.)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.56s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

All four cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:** Not triggered — this task modifies only Python worker files, not Rust handler signatures, utoipa annotations, or AppState fields.

**Gate 3 — Node Parity:** `worker/tests/test_parity.py` does not exist yet in this codebase. Not applicable.

## Public API Delta

No new `pub` items in Rust. Python public API items introduced:

| Item | Type | Module Path |
|------|------|-------------|
| `class MockModel` | class | `worker.nodes.loader` |
| `MockModel.__init__` | method | `worker.nodes.loader` |
| `class LoadModel` | class | `worker.nodes.loader` |
| `LoadModel.execute` | method | `worker.nodes.loader` |

## Deviations from Plan

- Updated `worker/tests/test_worker_main.py::test_mock_startup_sends_ready` to expect 2 registered node types (`SaveImage` + `LoadModel`) instead of 1. This was necessary because the new `LoadModel` node is correctly registered via the auto-import mechanism, and the existing test was asserting a hardcoded count of registered nodes. This is a required fix to prevent a test regression, not a deviation from the plan's scope.

## Blockers

None.
