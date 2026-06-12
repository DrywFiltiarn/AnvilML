# Implementation Report: P21-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A1                                      |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: nodes/base.py BaseNode + NodeContext + NODE_REGISTRY + @register |
| Implemented | 2026-06-12T20:15:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Created the foundational node infrastructure for the AnvilML Python worker: `worker/nodes/base.py` with `NodeContext` dataclass, `BaseNode` abstract base class, `NODE_REGISTRY` dict, and `@register` decorator; updated `worker/nodes/__init__.py` with auto-discovery imports via `pkgutil`; and created `worker/tests/test_nodes_base.py` with two tests verifying registration and abstract method enforcement. All 258 Rust tests, 22 Python worker tests, format gates, and platform cross-checks pass.

## Resolved Dependencies

Not applicable — no new dependencies added. Only stdlib imports (`abc`, `dataclasses`, `threading`, `typing`, `pkgutil`, `importlib`, `os`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/nodes/base.py` | NodeContext dataclass, BaseNode ABC, NODE_REGISTRY, @register decorator |
| Modify | `worker/nodes/__init__.py` | Auto-import node modules to populate registry |
| Create | `worker/tests/test_nodes_base.py` | Tests for @register and missing execute |

## Commit Log

```
 worker/nodes/__init__.py        | 26 +++++++++++++
 worker/nodes/base.py            | 83 +++++++++++++++++++++++++++++++++++++++++
 worker/tests/test_nodes_base.py | 54 +++++++++++++++++++++++++++
 3 files changed, 163 insertions(+)
```

## Test Results

### Python worker tests (worker/tests/test_nodes_base.py)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 2 items

worker/tests/test_nodes_base.py::TestRegisterPopulatesRegistry::test_register_populates_registry PASSED [ 50%]
worker/tests/test_nodes_base.py::TestMissingExecuteRaisesTypeError::test_missing_execute_raises_typeerror PASSED [100%]

============================== 2 passed in 0.04s ===============================
```

### Full Python worker test suite

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 22 items

worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED [  4%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED [  9%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED [ 13%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_socketpair_roundtrip PASSED [ 18%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_full_bidirectional_roundtrip PASSED [ 22%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_read_frame_eof PASSED [ 27%]
worker/tests/test_nodes_base.py::TestRegisterPopulatesRegistry::test_register_populates_registry PASSED [ 31%]
worker/tests/test_nodes_base.py::TestMissingExecuteRaisesTypeError::test_missing_execute_raises_typeerror PASSED [ 36%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 40%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 45%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED  [ 50%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 54%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 59%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [ 63%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED [ 68%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED [ 72%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED [ 77%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED [ 81%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED [ 86%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED [ 90%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED [ 95%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_node_delay_ms PASSED [100%]

============================== 22 passed in 4.38s ===============================
```

### Rust test suite (full workspace)

```
test result: ok. 76 passed; 0 failed; 0 ignored (anvilml-core)
test result: ok. 56 passed; 0 failed; 0 ignored (anvilml-hardware)
test result: ok. 18 passed; 0 failed; 0 ignored (anvilml-ipc)
test result: ok. 28 passed; 0 failed; 0 ignored (anvilml-registry)
test result: ok. 1 passed; 0 failed; 0 ignored (anvilml-registry integration)
test result: ok. 4 passed; 0 failed; 0 ignored (device_store)
test result: ok. 4 passed; 0 failed; 0 ignored (patch_meta)
test result: ok. 2 passed; 0 failed; 0 ignored (rescan)
test result: ok. 1 passed; 0 failed; 0 ignored (rescan_stale)
test result: ok. 1 passed; 0 failed; 0 ignored (safetensors_header)
test result: ok. 1 passed; 0 failed; 0 ignored (scanner)
test result: ok. 7 passed; 0 failed; 0 ignored (seed_loader)
test result: ok. 2 passed; 0 failed; 0 ignored (store_get)
test result: ok. 3 passed; 0 failed; 0 ignored (store_list)
test result: ok. 43 passed; 0 failed; 0 ignored (anvilml-scheduler)
test result: ok. 45 passed; 0 failed; 0 ignored (anvilml-server)
test result: ok. 1 passed; 0 failed; 0 ignored (api_artifact_save)
test result: ok. 3 passed; 0 failed; 0 ignored (api_artifact_serve)
test result: ok. 3 passed; 0 failed; 0 ignored (api_models)
test result: ok. 1 passed; 0 failed; 0 ignored (api_ws_events)
test result: ok. 19 passed; 0 failed; 0 ignored (anvilml-worker)
test result: ok. 17 passed; 0 failed; 0 ignored (anvilml binary)
test result: ok. 2 passed; 0 failed; 0 ignored (api_cancel)
test result: ok. 5 passed; 0 failed; 0 ignored (api_delete)
test result: ok. 1 passed; 0 failed; 0 ignored (api_ws_lifecycle)
test result: ok. 1 passed; 0 failed; 0 ignored (config_reference)
test result: ok. 4 passed; 0 failed; 0 ignored (preflight_check)
test result: ok. 2 passed; 0 failed; 0 ignored (doc-tests anvilml-hardware)
```

## Format Gate

```
# cargo fmt --all -- --check
# (exits 0, no output — no formatting drift)
```

## Platform Cross-Check

```
# 1. cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s

# 2. cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s

# 3. cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s

# 4. cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s
```

## Project Gates

Gate 1 (config surface sync): Not applicable — no `ServerConfig` or nested config struct modified.

Gate 2 (OpenAPI drift): Not applicable — no handler signatures, `ToSchema` types, `utoipa` annotations, or `anvilml-openapi/src/main.rs` modified.

## Deviations from Plan

None.

## Blockers

None.
