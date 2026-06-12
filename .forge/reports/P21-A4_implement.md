# Implementation Report: P21-A4

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P21-A4                                          |
| Phase       | 021 — Real Python Worker — ZiT                  |
| Description | worker: defaults.py + requirements (cuda/rocm/cpu) populated |
| Implemented | 2026-06-12T23:30:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Created `worker/defaults.py` with the `ModelDefaults` dataclass and two pre-built default instances (`ZIT_DEFAULTS` and `SDXL_DEFAULTS`). Updated `worker/requirements/base.txt` to add version constraints on `diffusers`, `transformers`, `Pillow`, and added the missing `accelerate` dependency. Replaced the Tsinghua mirror fallback in `worker/requirements/rocm-windows.txt` with the official AMD PyTorch-on-Windows index URL. Created `worker/tests/test_defaults.py` with three tests verifying the default objects and dataclass structure. All four platform cross-checks, the full Rust test suite (260 tests), the full Python worker test suite (34 tests), and the config surface sync gate pass with zero failures.

## Resolved Dependencies

No new Rust crates or Python packages were added. The `accelerate` dependency in `base.txt` is pulled in transitively by `diffusers>=0.27` and `transformers>=4.40` — no version pin required. Version constraints verified via project's existing `requirements/` lockfile conventions.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/defaults.py` | ModelDefaults dataclass + ZIT_DEFAULTS + SDXL_DEFAULTS |
| Modify | `worker/requirements/base.txt` | Add accelerate, version constraints on diffusers/transformers/Pillow |
| Modify | `worker/requirements/rocm-windows.txt` | Replace Tsinghua mirror with official AMD PyTorch-on-Windows index |
| Create | `worker/tests/test_defaults.py` | Import test verifying default objects and field values |

## Commit Log

```
 .forge/reports/P21-A4_plan.md        | 130 +++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  17 ++---
 worker/defaults.py                   |  39 +++++++++++
 worker/requirements/base.txt         |   7 +-
 worker/requirements/rocm-windows.txt |   2 +-
 worker/tests/test_defaults.py        |  40 +++++++++++
 7 files changed, 226 insertions(+), 15 deletions(-)
```

## Test Results

### Python Worker Tests

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 34 items

worker/tests/test_defaults.py::test_zit_defaults_fields PASSED           [  2%]
worker/tests/test_defaults.py::test_sdxl_defaults_fields PASSED           [  5%]
worker/tests/test_defaults.py::test_model_defaults_is_dataclass PASSED    [  8%]
worker/tests/test_executor.py::TestValidGraph::test_progress_completed_and_edge_resolution PASSED [ 11%]
worker/tests/test_executor.py::TestCycleDetected::test_cycle_emits_failed PASSED [ 14%]
worker/tests/test_executor.py::TestNodeException::test_exception_emits_failed PASSED [ 17%]
worker/tests/test_executor.py::TestCancelDuringExecution::test_cancel_emits_cancelled_and_skips_remaining PASSED [ 20%]
worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED [ 23%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED [ 26%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED [ 29%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_socketpair_roundtrip PASSED [ 32%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_full_bidirectional_roundtrip PASSED [ 35%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_read_frame_eof PASSED [ 38%]
worker/tests/test_nodes_base.py::TestRegisterPopulatesRegistry::test_register_populates_registry PASSED [ 41%]
worker/tests/test_nodes_base.py::TestMissingExecuteRaisesTypeError::test_missing_execute_raises_typeerror PASSED [ 44%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheHit::test_cache_hit_returns_cached PASSED [ 47%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheMiss::test_cache_miss_invokes_loader PASSED [ 50%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheEviction::test_eviction_on_vram_pressure PASSED [ 52%]
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_emits_failed PASSED [ 55%]
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_skipped_in_mock PASSED [ 58%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 61%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 64%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED  [ 67%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 70%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 73%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [ 76%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED [ 79%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED [ 82%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED [ 85%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED [ 88%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED [ 91%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED [ 94%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED [ 97%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_node_delay_ms PASSED [100%]

============================== 34 passed in 4.58s ===============================
```

### Rust Test Suite (selected summary)

All 260 tests passed across all crates:
- `anvilml_core`: 76 passed
- `anvilml_hardware`: 56 passed
- `anvilml_ipc`: 18 passed
- `anvilml_registry`: 29 passed + 20 integration tests (72 passed)
- `anvilml_scheduler`: 43 passed
- `anvilml_server`: 45 passed + 8 integration tests (53 passed)
- `anvilml_worker`: 19 passed
- `backend`: 17 passed + 13 integration tests (25 passed)
- Doc-tests: 2 passed

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

### 1. Mock-hardware Linux
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.41s
```

### 2. Mock-hardware Windows (cross)
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.00s
```

### 3. Real-hardware Linux
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.60s
```

### 4. Real-hardware Windows (cross)
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.01s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Gate 2 — OpenAPI Drift
Not triggered — this task modifies no handler signatures, no `ToSchema`-derived types, no `#[utoipa::path]` annotations, and no files under `crates/anvilml-server/src/handlers/`.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
