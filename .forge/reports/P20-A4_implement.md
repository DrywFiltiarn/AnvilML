# Implementation Report: P20-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P20-A4                                            |
| Phase       | 020 — OpenAPI & Launcher Polish                   |
| Description | CI openapi-diff gate + python-worker pytest job   |
| Implemented | 2026-06-12T11:30:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Added an OpenAPI spec diff gate to the `rust` CI job (ubuntu-latest only) that regenerates `backend/openapi.json` via `cargo run -p anvilml-openapi` and fails if the committed file is stale. Updated the `python-worker` CI job to install dependencies from `worker/requirements/base.txt` instead of the previous inline `pip install msgpack pillow pytest`. All local verifications, format gates, lint, cross-checks, tests, and project gates pass with zero failures.

## Resolved Dependencies

Not applicable — this task modifies only a CI workflow file (`.github/workflows/ci.yml`), adding no new dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `.github/workflows/ci.yml` | Add openapi-diff steps to rust-linux path; update python-worker deps to use `worker/requirements/base.txt` |

## Commit Log

```
 .forge/reports/P20-A4_plan.md | 108 ++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +--
 .forge/state/state.json       |  13 ++---
 .github/workflows/ci.yml      |  10 +++-
 4 files changed, 127 insertions(+), 10 deletions(-)
```

## Test Results

### Local openapi-diff verification
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.72s
     Running `target/debug/anvilml-openapi`
Generated OpenAPI spec: /home/dryw/AnvilML/backend/openapi.json
EXIT_CODE=0
```

### Local pytest verification
```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 20 items

worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED [  5%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED [ 10%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED [ 15%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_socketpair_roundtrip PASSED [ 20%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_full_bidirectional_roundtrip PASSED [ 25%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_read_frame_eof PASSED [ 30%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 35%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 40%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED  [ 45%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 50%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 55%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [ 60%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED [ 65%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED [ 70%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED [ 75%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED [ 80%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED [ 85%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED [ 90%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED [ 95%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_node_delay_ms PASSED [100%]

============================== 20 passed in 4.37s ==============================
EXIT_CODE=0
```

### Rust full test suite
```
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-core)
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-hardware)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-ipc)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-openapi)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-registry)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml_registry_db)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (device_store)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (rescan)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (scanner)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (seed_loader)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (store_get)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (store_list)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-scheduler)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-server)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_artifact_save)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_artifact_serve)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_models)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_ws_events)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml-worker)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (anvilml binary)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_cancel)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_delete)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (api_ws_lifecycle)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (config_reference)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (preflight_check)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out (doc-tests anvilml-hardware)
EXIT_CODE=0
```

## Format Gate

```
EXIT_CODE=0
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s
EXIT_CODE=0

# 2. Mock-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.58s
EXIT_CODE=0

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s
EXIT_CODE=0

# 4. Real-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
EXIT_CODE=0
```

## Project Gates

### Gate 1 — Config Surface Sync
```
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.29s
     Running unittests src/main.rs (target/debug/deps/anvilml-03b4d2703be5ae7e)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out

     Running tests/api_cancel.rs (target/debug/deps/api_cancel-1becac64088b72d6)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out

     Running tests/api_delete.rs (target/debug/deps/api_delete-5cdd7b68a0f48f87)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out

     Running tests/api_ws_lifecycle.rs (target/debug/deps/api_ws_lifecycle-59892c70)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-48e50093)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out

     Running tests/preflight_check.rs (target/debug/deps/preflight_check-7f289abae170f164)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
EXIT_CODE=0
```

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
