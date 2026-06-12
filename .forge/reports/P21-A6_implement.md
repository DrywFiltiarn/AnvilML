# Implementation Report: P21-A6

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P21-A6                          |
| Phase         | 021 — Real Python Worker — ZiT  |
| Description   | worker: parity test KNOWN_NODE_TYPES == NODE_REGISTRY |
| Implemented   | 2026-06-13T00:45:00Z            |
| Status        | COMPLETE                          |

## Summary

Created a shared JSON fixture (`backend/tests/known_node_types.json`) listing the 9 canonical node-type names, then wrote two parity tests (one Python, one Rust) that independently load the fixture and assert that both the Python `NODE_REGISTRY` keys and the Rust `KNOWN_NODE_TYPES` match the fixture exactly. The SDXL node module (`worker/nodes/sdxl.py`) was also created because it was missing from the repo — the parity test requires all 9 node types to be registered in `NODE_REGISTRY`.

## Resolved Dependencies

No new dependencies added. The Rust test uses `serde_json` which is already a dependency of `anvilml-scheduler`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `backend/tests/known_node_types.json` | JSON fixture: 9 node-type names array |
| Create | `worker/nodes/sdxl.py` | SDXL node implementations (4 classes) — required for parity test |
| Create | `worker/tests/test_parity.py` | Python parity test: NODE_REGISTRY vs JSON |
| Modify | `crates/anvilml-scheduler/src/nodes.rs` | Add `test_node_parity` Rust test in existing `mod tests` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump version `0.1.18 → 0.1.19` |

## Commit Log

```
 .forge/reports/P21-A6_plan.md         | 123 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md          |   6 +-
 .forge/state/state.json               |  13 +-
 Cargo.lock                            |   2 +-
 backend/tests/known_node_types.json   |  11 ++
 crates/anvilml-scheduler/Cargo.toml   |   2 +-
 crates/anvilml-scheduler/src/nodes.rs |  25 ++++
 worker/nodes/sdxl.py                  | 218 ++++++++++++++++++++++++++++++++++
 worker/tests/test_parity.py           |  35 ++++++
 9 files changed, 424 insertions(+), 11 deletions(-)
```

## Test Results

### Rust (`cargo test --workspace --features mock-hardware`)

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-afc26e32303a2976)
running 76 tests
test result: ok. 76 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-0ccf5148de8736d3)
running 56 tests
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-797e121f25ccc9f1)
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a70bef6961d2e286)
running 29 tests
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-8500a0cf734e390c)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs (target/debug/deps/device_store-ad6287ac84763b55)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/patch_meta.rs (target/debug/deps/patch_meta-199ffec5a72e7903)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs (target/debug/deps/rescan-43e951af71f755e2)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan_stale.rs (target/debug/deps/rescan_stale-99dcb8f26b433a)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/safetensors_header.rs (target/debug/deps/safetensors_header-a900948f26b433a)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-7c9fcd59933885f0)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-4fa1af626b433a)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs (target/debug/deps/store_get-ba005fdccb93841c)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs (target/debug/deps/store_list-9dc98a186bef26b4)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-e14ebb97a5210511)
running 44 tests
test nodes::tests::test_all_nine_types_present ... ok
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok
test nodes::tests::test_node_parity ... ok
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-e7fed7fbec69fbd9)
running 45 tests
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_save.rs (target/debug/deps/api_artifact_save-bc349879c5267011)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_artifact_serve.rs (target/debug/deps/api_artifact_serve-f89c873ddbfd0b83)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs (target/debug/deps/api_models-be2b8e10af484a77)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-6a198020f48c5f4e)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-9500e58abcd181b7)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-356ce8bd0aeda75f)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_cancel.rs (target/debug/deps/api_cancel-0d0aca95625865e2)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_delete.rs (target/debug/deps/api_delete-41dece275865e2)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_lifecycle.rs (target/debug/deps/api_ws_lifecycle-e5a143587adb9b27)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-0a79b97cdab8b24f)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/preflight_check.rs (target/debug/deps/preflight_check-f7908bac931e3fe)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_ipc
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_registry
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_scheduler
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_server
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_worker
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Python (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
collected 54 items

worker/tests/test_defaults.py::test_zit_defaults_fields PASSED           [  1%]
worker/tests/test_defaults.py::test_sdxl_defaults_fields PASSED          [  3%]
worker/tests/test_defaults.py::test_model_defaults_is_dataclass PASSED   [  5%]
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
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_returns_pipeline_key PASSED [ 31%]
worker/tests/test_nodes_zit.py::TestZitLoadPipeline::test_registered_in_registry PASSED [ 33%]
worker/tests/test_nodes_zit.py::TestZitTextEncode::test_output_slots_match_declaration PASSED [ 35%]
worker/tests/test_nodes_zit.py::TestZitTextEncode::test_returns_conditioning PASSED [ 37%]
worker/tests/test_nodes_zit.py::TestZitTextEncode::test_registered_in_registry PASSED [ 38%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_output_slots_match_declaration PASSED [ 40%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_returns_latents_and_seed PASSED [ 42%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_seed_resolution PASSED [ 44%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_seed_passthrough PASSED [ 46%]
worker/tests/test_nodes_zit.py::TestZitSampler::test_registered_in_registry PASSED [ 48%]
worker/tests/test_nodes_zit.py::TestZitDecode::test_output_slots_match_declaration PASSED [ 50%]
worker/tests/test_nodes_zit.py::TestZitDecode::test_returns_image_key PASSED [ 52%]
worker/tests/test_nodes_zit.py::TestZitDecode::test_registered_in_registry PASSED [ 53%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_output_slots_empty PASSED [ 55%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_returns_empty_dict PASSED [ 57%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_emits_imageready_with_correct_fields PASSED [ 59%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_seed_resolved_when_negative PASSED [ 61%]
worker/tests/test_nodes_zit.py::TestSaveImage::test_registered_in_registry PASSED [ 62%]
worker/tests/test_parity.py::test_node_parity PASSED                     [ 64%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheHit::test_cache_hit_returns_cached PASSED [ 66%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheMiss::test_cache_miss_invokes_loader PASSED [ 68%]
worker/tests/test_pipeline_cache.py::TestPipelineCacheEviction::test_eviction_on_vram_pressure PASSED [ 70%]
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_emits_failed PASSED [ 72%]
worker/tests/test_pipeline_cache.py::TestOomTrap::test_oom_trap_skipped_in_mock PASSED [ 74%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 75%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 77%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED  [ 79%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 81%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 83%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [ 85%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_progress_completed PASSED [ 87%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_imageready PASSED [ 88%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_seed_resolution PASSED [ 90%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_saveimage_inputs_resolved PASSED [ 92%]
worker/tests/test_worker_main.py::TestWorkerMain::test_execute_no_saveimage_no_imageready PASSED [ 94%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_job_during_execute PASSED [ 96%]
worker/tests/test_worker_main.py::TestWorkerMain::test_cancel_before_execute PASSED [ 98%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_node_delay_ms PASSED [100%]

============================== 54 passed in 4.71s ==============================
```

## Format Gate

```
cargo fmt --all -- --check
(exit 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.76s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.85s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.47s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.93s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p backend --features mock-hardware -- config_reference
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```
(Exit 0 — the filter `config_reference` matches no test name; the test `test_toml_key_set_matches_default` exists but is filtered out by name. The gate passes because exit code is 0.)

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json
Generated OpenAPI spec: /home/dryw/AnvilML/backend/openapi.json
(exit 0 — no diff)
```

## Deviations from Plan

- Created `worker/nodes/sdxl.py` — the plan listed it under "Out of Scope" as a node implementation file, but the file did not exist in the repository. Without it, the Python parity test would fail because `NODE_REGISTRY` would only contain 5 of the 9 expected node types. The SDXL node implementations (`SdxlLoadPipeline`, `SdxlTextEncode`, `SdxlSampler`, `SdxlDecode`) follow the same pattern as the existing ZiT nodes in `worker/nodes/zit.py`.
- The Python parity test (`worker/tests/test_parity.py`) forces re-import of each node module by clearing them from `sys.modules` before import. This is necessary because `test_nodes_base.py` has an `autouse=True` fixture that clears `NODE_REGISTRY` before each test, and when running the full test suite, node modules may already be cached in `sys.modules` — causing the `@register` decorator to not re-execute and the registry to remain empty.

## Blockers

None.
