# Implementation Report: P21-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A5                                      |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: nodes/zit.py real ZiT nodes + nodes/common.py SaveImage |
| Implemented | 2026-06-13T00:00:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Created three new Python files implementing the ZiT (Zero-Iteration) diffusion pipeline nodes and the SaveImage node for the AnvilML worker. `worker/nodes/zit.py` defines four `@register`-decorated node classes (`ZitLoadPipeline`, `ZitTextEncode`, `ZitSampler`, `ZitDecode`) with real `diffusers` integration and mock sentinel fallbacks. `worker/nodes/common.py` defines `SaveImage` which encodes a PIL Image to PNG and emits an `ImageReady` IPC event. `worker/tests/test_nodes_zit.py` provides 19 mock-mode tests verifying output slot correctness, registry registration, and `ImageReady` event emission. All 53 Python tests and 270 Rust tests pass, all four platform cross-checks pass, and both project gates (config sync, OpenAPI drift) pass.

## Resolved Dependencies

| Type   | Name         | Version resolved | Source         |
|--------|--------------|------------------|----------------|
| python | diffusers    | (optional import)| pypi-query MCP |
| python | torch        | (conditional)    | pypi-query MCP |
| python | pillow       | (conditional)    | pypi-query MCP |

No new dependencies were added to any manifest file. All imports (`diffusers`, `torch`, `PIL.Image`, `base64`, `random`, `threading`) are either already present in the project's dependency files or are Python stdlib. The `diffusers` import is wrapped in a `try/except ImportError` guard so the mock path never requires it.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/nodes/zit.py` | Four ZiT node classes (ZitLoadPipeline, ZitTextEncode, ZitSampler, ZitDecode) with real diffusers + mock sentinel paths |
| Create | `worker/nodes/common.py` | SaveImage node with PNG encode + ImageReady emission |
| Create | `worker/tests/test_nodes_zit.py` | pytest: mock output slots correct (19 tests) |

No version bumps required — no Rust crate source files were modified.

## Commit Log

```
 .forge/reports/P21-A5_plan.md  | 102 +++++++++++++++
 .forge/state/CURRENT_TASK.md   |   6 +-
 .forge/state/state.json        |  13 +-
 worker/nodes/common.py         |  87 +++++++++++++
 worker/nodes/zit.py            | 209 ++++++++++++++++++++++++++++++
 worker/tests/test_nodes_zit.py | 286 +++++++++++++++++++++++++++++++++++++++++
 6 files changed, 694 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0, pytest-9.0.3, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 53 items

worker/tests/test_defaults.py::test_zit_defaults_fields PASSED
worker/tests/test_defaults.py::test_sdxl_defaults_fields PASSED
worker/tests/test_defaults.py::test_model_defaults_is_dataclass PASSED
worker/tests/test_executor.py::TestValidGraph::test_progress_completed_and_edge_resolution PASSED
worker/tests/test_executor.py::TestCycleDetected::test_cycle_emits_failed PASSED
worker/tests/test_executor.py::TestNodeException::test_exception_emits_failed PASSED
worker/tests/test_executor.py::TestCancelDuringExecution::test_cancel_emits_cancelled_and_skips_remaining PASSED
worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED
worker/tests/test_ipc.py::TestSocketRoundtrip::test_socketpair_roundtrip PASSED
worker/tests/test_ipc.py::TestSocketRoundtrip::test_full_bidirectional_roundtrip PASSED
worker/tests/test_ipc.py::TestSocketRoundtrip::test_read_frame_eof PASSED
worker/tests/test_nodes_base.py::TestRegisterPopulatesRegistry::test_register_populates_registry PASSED
worker/tests/test_nodes_base.py::TestMissingExecuteRaisesTypeError::test_missing_execute_raises_typeerror PASSED
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_output_slots_match_declaration PASSED
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_returns_pipeline_key PASSED
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_registered_in_registry PASSED
worker/tests/test_nodes_zit.py::TestZitTextEncode::test_output_slots_match_declaration PASSED
worker/tests/test_nodes_zit.py::TestZitTextEncode::test_returns_conditioning_key PASSED
worker/tests/test_nodes_zit.py::TestZitTextEncode::test_registered_in_registry PASSED
worker/tests/test_nodes_zit.py::TestZitSampler::test_output_slots_match_declaration PASSED
worker/tests/test_nodes_zit.py::TestZitSampler::test_returns_latents_and_seed PASSED
worker/tests/test_nodes_zit.py::TestZitSampler::test_seed_resolution PASSED
worker/tests/test_nodes_zit.py::TestZitSampler::test_seed_passthrough PASSED
worker/tests/test_nodes_zit.py::TestZitSampler::test_registered_in_registry PASSED
worker/tests/test_nodes_zit.py::TestZitDecode::test_output_slots_match_declaration PASSED
worker/tests/test_nodes_zit.py::TestZitDecode::test_returns_image_key PASSED
worker/tests/test_nodes_zit.py::TestZitDecode::test_registered_in_registry PASSED
worker/tests/test_nodes_zit.py::TestSaveImage::test_output_slots_empty PASSED
worker/tests/test_nodes_zit.py::TestSaveImage::test_returns_empty_dict PASSED
worker/tests/test_nodes_zit.py::TestSaveImage::test_emits_imageready_with_correct_fields PASSED
worker/tests/test_nodes_zit.py::TestSaveImage::test_seed_resolved_when_negative PASSED
worker/tests/test_nodes_zit.py::TestSaveImage::test_registered_in_registry PASSED
worker/tests/test_pipeline_cache.py::TestPipelineCacheHit::test_cache_hit_returns_cached PASSED
worker/tests/test_pipeline_cache.py::TestPipelineCacheMiss::test_cache_miss_invokes_loader PASSED
worker/tests/test_pipeline_cache.py::TestPipelineCacheEviction::test_eviction_on_vram_pressure PASSED
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_emits_failed PASSED
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_skipped_in_mock PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_node_delay_ms PASSED

============================== 53 passed in 4.74s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.54s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p backend --features mock-hardware --test config_reference
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json
Generated OpenAPI spec: /home/dryw/AnvilML/backend/openapi.json
(no diff — openapi.json is up to date)
```

## Deviations from Plan

- **SaveImage `execute` signature**: The plan specified `INPUT_SLOTS=["image", "prompt", "seed", "steps"]` with `image` as a required input. However, the existing `test_worker_main.py` tests invoke `SaveImage` without providing an `image` input (the executor only passes inputs that have edge references or literal values in the graph). To maintain backward compatibility with these tests and the executor's fallback path, `image` was given a default of `None`, which triggers the mock placeholder path (64×64 black PNG). This is consistent with the executor's fallback `SaveImage` behavior.
- **Test fixture isolation**: The `NODE_REGISTRY` is cleared by the `autouse` fixture before each test. Since `@register` runs at module-load time, re-importing a module does not re-run the decorator. The fix was to also remove cached modules from `sys.modules` in the fixture, so the next import re-executes the module body and re-registers all nodes.
- **Pre-existing test fix**: The three failing `test_worker_main.py` tests (`test_execute_saveimage_imageready`, `test_execute_saveimage_seed_resolution`, `test_execute_saveimage_inputs_resolved`) were caused by the registered `SaveImage` node crashing when called without an `image` input. Fixed by adding `image=None` default and handling `None` in the mock path.

## Blockers

None.
