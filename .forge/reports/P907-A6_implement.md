# Implementation Report: P907-A6

| Field | Value |
|-------|-------|
| Task ID | P907-A6 |
| Phase | 907 — ZeroMQ IPC Transport |
| Description | worker/tests: update test_ipc.py and test_worker_main.py for ZeroMQ transport |
| Implemented | 2026-06-13T18:45:00Z |
| Status | COMPLETE |

## Summary

Rewrote `worker/tests/test_worker_main.py` from stdin/stdout length-prefixed framing to ZeroMQ DEALER socket communication. All 14 previously-skipped tests are now unskipped and passing. Removed obsolete `struct` import, `_make_frame()`, and `_parse_frames()` helpers. Added `_ZmqTransport` helper class and `_spawn_worker()` that binds a ZMQ DEALER socket on an ephemeral port and passes the port via `ANVILML_IPC_PORT`. Confirmed `test_ipc.py` already uses ZMQ PAIR sockets and requires no changes.

## Resolved Dependencies

Not applicable — no new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/tests/test_worker_main.py` | Rewrite all 14 tests for ZMQ DEALER transport |
| Read only | `worker/tests/test_ipc.py` | Already ZMQ PAIR — no changes needed |

## Commit Log

```
 .forge/reports/P907-A6_plan.md   | 198 +++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +-
 worker/tests/test_worker_main.py | 688 ++++++++++++++++++---------------------
 4 files changed, 527 insertions(+), 378 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 54 items

worker/tests/test_defaults.py::test_zit_defaults_fields PASSED           [  1%]
worker/tests/test_defaults.py::test_sdxl_defaults_fields PASSED           [  3%]
worker/tests/test_defaults.py::test_model_defaults_is_dataclass PASSED    [  5%]
worker/tests/test_executor.py::TestValidGraph::test_progress_completed_and_edge_resolution PASSED [  7%]
worker/tests/test_executor.py::TestCycleDetected::test_cycle_emits_failed PASSED [  9%]
worker/tests/test_executor.py::TestNodeException::test_exception_emits_failed PASSED [ 11%]
worker/tests/test_executor.py::TestCancelDuringExecution::test_cancel_emits_cancelled_and_skips_remaining PASSED [ 12%]
worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED [ 14%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED [ 16%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED [ 18%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_socketpair_roundtrip PASSED [ 20%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_full_bidirectional_roundtrip PASSED [ 22%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_read_frame_eof PASSED [ 24%]
worker/tests/test_nodes_base.py::TestRegisterPopulatesRegistry::test_register_populates_registry PASSED [ 25%]
worker/tests/test_nodes_base.py::TestMissingExecuteRaisesTypeError::test_missing_execute_raises_typeerror PASSED [ 27%]
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_output_slots_match_declaration PASSED [ 29%]
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_returns_conditioning_key PASSED [ 31%]
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_registered_in_registry PASSED [ 33%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_output_slots_match_declaration PASSED [ 35%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_returns_latents_and_seed PASSED [ 37%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_seed_resolution PASSED [ 39%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_seed_passthrough PASSED [ 41%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_registered_in_registry PASSED [ 43%]
worker/tests/test_nodes_zit.py::TestZitDecode::test_output_slots_match_declaration PASSED [ 45%]
worker/tests/test_nodes_zit.py::TestZitDecode::test_returns_image_key PASSED [ 47%]
worker/tests/test_nodes_zit.py::TestZitDecode::test_registered_in_registry PASSED [ 49%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_output_slots_empty PASSED [ 51%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_returns_empty_dict PASSED [ 53%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_emits_imageready_with_correct_fields PASSED [ 55%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_seed_resolved_when_negative PASSED [ 57%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_registered_in_registry PASSED [ 59%]
worker/tests/test_parity.py::test_node_parity PASSED                     [ 61%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheHit::test_cache_hit_returns_cached PASSED [ 63%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheMiss::test_cache_miss_invokes_loader PASSED [ 65%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheEviction::test_eviction_on_vram_pressure PASSED [ 67%]
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_emits_failed PASSED [ 69%]
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_skipped_in_mock PASSED [ 71%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 73%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 75%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED  [ 77%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 79%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 81%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [ 83%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED [ 85%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED [ 87%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED [ 89%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED [ 91%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED [ 93%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED [ 95%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED [ 97%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_node_delay_ms PASSED [100%]

============================== 54 passed in 5.02s ==============================
```

## Format Gate

Not applicable — task modified only Python test files; no Rust source files changed.

## Platform Cross-Check

Not required — no Rust source files modified.

## Project Gates

- **Gate 1 (Config Surface Sync):** Not applicable — no `ServerConfig` fields added/removed/renamed.
- **Gate 2 (OpenAPI Drift):** Not applicable — no handler signatures, `ToSchema` types, or `utoipa` annotations modified.

## Deviations from Plan

- Added `proc.wait(timeout=5)` before `assert proc.returncode == 0` in 5 tests (`test_shutdown_dying_exit`, `test_double_init_exits`, `test_execute_progress_completed`, `test_execute_saveimage_imageready`, `test_cancel_before_execute`) to account for the asynchronous process termination after `sys.exit(0)`. This was needed because `Dying` is sent via ZMQ before the process actually exits, so `proc.returncode` is `None` until `proc.wait()` is called.

## Blockers

None.
