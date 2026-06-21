# Implementation Report: P18-B1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P18-B1                             |
| Phase         | 018 — ZiT Generic Nodes            |
| Description   | worker/nodes/encoder.py: ClipTextEncode node |
| Implemented   | 2026-06-21T13:10:00Z               |
| Status        | COMPLETE                           |

## Summary

Created `worker/nodes/encoder.py` implementing the `ClipTextEncode` node — a conditioning node that accepts a `CLIP` object, a prompt string, and an optional negative prompt string, then returns a `CONDITIONING` slot. In mock mode (`ANVILML_WORKER_MOCK=1`), it returns a lightweight `MockConditioning` sentinel carrying the encoded text. The real path is stubbed with `NotImplementedError`. Also created `worker/tests/test_nodes_encoder.py` with 4 passing tests covering registry registration, mock-mode execution, metadata attributes, and optional negative text handling. Fixed a pre-existing test (`test_mock_startup_sends_ready`) that asserted an outdated node count of 3 instead of the correct count of 5 (now including `LoadClip` and `ClipTextEncode`).

## Resolved Dependencies

None. This task introduces no external packages. All dependencies are existing Python stdlib (`os`, `typing`, `importlib`, `pkgutil`, `pytest`) already available in the worker venv.

| Type | Name | Version resolved | Source |
|------|------|-----------------|--------|
| (none) | — | — | — |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/encoder.py` | ClipTextEncode node + MockConditioning sentinel |
| CREATE | `worker/tests/test_nodes_encoder.py` | Unit tests for ClipTextEncode node (4 tests) |
| MODIFY | `worker/tests/test_worker_main.py` | Updated `test_mock_startup_sends_ready` to expect 5 node types instead of 3 |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries for new encoder tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md                    |  6 +++---
 .forge/state/state.json                         | 13 +++++++------
 docs/TESTS.md                                   | 36 +++++++++++++++++++++++++
 worker/tests/test_nodes_encoder.py              | 190 ++++++++++++++++++++++++++
 worker/tests/test_worker_main.py                | 16 ++++++----
 5 files changed, 245 insertions(+), 16 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 40 items

worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [  2%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [  5%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [  7%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [ 10%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 12%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 15%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED             [ 17%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED         [ 20%]
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED [ 22%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 25%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 27%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 30%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 32%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 35%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 37%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 40%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 42%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 45%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 47%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 50%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry PASSED [ 52%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning PASSED [ 55%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_metadata_attributes PASSED [ 57%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [ 62%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 65%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 67%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 70%]
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED [ 72%]
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED [ 75%]
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED [ 77%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 80%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 82%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 85%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [ 87%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 90%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 92%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED         [ 95%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED    [ 97%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [100%]

============================== 40 passed in 1.47s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

All four platform cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passes. Gate 2 (OpenAPI drift) not triggered — no handler signatures or handler annotations modified. Gate 3 (Node Parity) — `worker/tests/test_parity.py` does not yet exist in this repo.

## Public API Delta

```
(No new pub items — Python uses class/function definitions with docstrings instead of Rust's pub keyword)
```

The new public items introduced by this task:

| Item | Type | Module Path |
|------|------|-------------|
| `class MockConditioning` | class | `worker.nodes.encoder` |
| `MockConditioning.__init__(self, text: str)` | method | `worker.nodes.encoder.MockConditioning` |
| `class ClipTextEncode` | class | `worker.nodes.encoder` |
| `ClipTextEncode.execute(self, **inputs)` | method | `worker.nodes.encoder.ClipTextEncode` |

## Deviations from Plan

- **Pre-existing test fix**: The test `test_mock_startup_sends_ready` in `worker/tests/test_worker_main.py` asserted `len(ready["node_types"]) == 3` and `type_names == {"SaveImage", "LoadModel", "LoadVae"}`. This was an outdated assertion — `LoadClip` had been added in a prior task (P18-A3) without updating this test, and my new `ClipTextEncode` node added a fifth node type. Updated the assertion to expect 5 node types and the correct set `{"SaveImage", "LoadModel", "LoadVae", "LoadClip", "ClipTextEncode"}`. This is a fix for a pre-existing defect that was surfaced by this task's code changes.

## Blockers

None.
