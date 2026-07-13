# Implementation Report: P19-E1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P19-E1                          |
| Phase         | 19 — Model Loading Contract Groundwork |
| Description   | CI: worker-test job collects loader.py + pipeline_cache.py tests |
| Implemented   | 2026-07-13T13:15:00Z            |
| Status        | COMPLETE                        |

## Summary

This task verified — without any code changes — that Phase 19's new test files
(`worker/tests/test_pipeline_cache.py` and `worker/tests/test_nodes_loader.py`) are
correctly collected and executed by the existing `worker-test` CI job. Both mock-mode
and real-mode pytest invocations exit 0 with all expected tests present. The CI
workflow (`.github/workflows/ci.yml`) already uses `worker/tests` glob patterns that
auto-collect new test files, so no structural change was needed.

## Resolved Dependencies

None. This task performs verification only — no new dependencies are introduced or
referenced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Read | `.github/workflows/ci.yml` | Confirm CI wiring for worker tests |
| Read | `worker/tests/test_pipeline_cache.py` | Verify test file exists and is collected |
| Read | `worker/tests/test_nodes_loader.py` | Verify test file exists and is collected |
| Read | `worker/pipeline_cache.py` | Confirm source under test |
| Read | `worker/nodes/loader.py` | Confirm source under test |
| Modify | `.forge/reports/P19-E1_plan.md` | Inherited from prior PLAN session |
| Modify | `.forge/state/CURRENT_TASK.md` | State update |
| Modify | `.forge/state/state.json` | State update |

## Commit Log

```
 .forge/reports/P19-E1_plan.md | 218 ++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 +-
 .forge/state/state.json       |  13 +--
 3 files changed, 228 insertions(+), 9 deletions(-)
```

## Test Results

### Mock-mode test run (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v`)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 123 items

worker/tests/test_arch_dispatch.py::test_get_module_returns_none_when_empty PASSED [  0%]
worker/tests/test_arch_dispatch.py::test_get_module_does_not_raise_for_various_key_types PASSED [  1%]
worker/tests/test_arch_dispatch.py::test_get_module_skips_module_with_can_handle_false PASSED [  2%]
worker/tests/test_arch_dispatch.py::test_clip_get_module_returns_none_when_empty PASSED [  3%]
worker/tests/test_arch_dispatch.py::test_clip_get_module_does_not_raise_for_various_key_types PASSED [  4%]
worker/tests/test_arch_dispatch.py::test_clip_get_module_skips_module_with_can_handle_false PASSED [  4%]
worker/tests/test_arch_dispatch.py::test_vae_get_module_returns_none_when_empty PASSED [  5%]
worker/tests/test_arch_dispatch.py::test_vae_get_module_does_not_raise_for_various_key_types PASSED [  6%]
worker/tests/test_arch_dispatch.py::test_vae_get_module_skips_module_with_can_handle_false PASSED [  7%]
worker/tests/test_base.py::test_node_registry_starts_empty PASSED        [  8%]
worker/tests/test_base.py::test_slotspec_optional_defaults_to_false PASSED [  8%]
worker/tests/test_base.py::test_slotspec_accepts_explicit_optional_true PASSED [  9%]
worker/tests/test_base.py::test_register_success PASSED                  [ 10%]
worker/tests/test_base.py::test_register_missing_NODE_TYPE PASSED        [ 11%]
worker/tests/test_base.py::test_register_missing_CATEGORY PASSED         [ 12%]
worker/tests/test_base.py::test_register_missing_DISPLAY_NAME PASSED     [ 13%]
worker/tests/test_base.py::test_register_missing_DESCRIPTION PASSED      [ 13%]
worker/tests/test_base.py::test_register_missing_INPUT_SLOTS PASSED      [ 14%]
worker/tests/test_base.py::test_register_missing_OUTPUT_SLOTS PASSED     [ 15%]
worker/tests/test_base.py::test_register_returns_class_identity PASSED   [ 16%]
worker/tests/test_base.py::test_node_context_assigns_all_attrs PASSED    [ 17%]
worker/tests/test_base.py::test_node_context_mock_true PASSED            [ 17%]
worker/tests/test_base.py::test_node_context_mock_false PASSED           [ 18%]
worker/tests/test_base.py::test_node_context_caps_accepts_arbitrary_dict PASSED [ 19%]
worker/tests/test_base.py::test_base_node_cannot_be_instantiated PASSED  [ 20%]
worker/tests/test_base.py::test_concrete_subclass_instantiates PASSED    [ 21%]
worker/tests/test_base.py::test_execute_calls_subclass_impl PASSED       [ 21%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp32_cpu_returns_true PASSED [ 22%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp16_cpu_returns_true PASSED [ 23%]
worker/tests/test_capability.py::TestProbeDtypes::test_bf16_cpu_returns_true PASSED [ 24%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp8_cpu_returns_false PASSED [ 25%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp4_cpu_returns_false PASSED [ 26%]
worker/tests/test_capability.py::TestProbeDtypes::test_float8_tensor_construction_and_forward_succeed_on_cpu PASSED [ 26%]
worker/tests/test_capability.py::TestProbeFlashAttention::test_flash_attention_cpu_returns_true PASSED [ 27%]
worker/tests/test_capability.py::TestProbeStructure::test_returns_dict_with_exactly_six_bool_keys PASSED [ 28%]
worker/tests/test_capability.py::TestProbeStructure::test_never_raises_for_cpu PASSED [ 29%]
worker/tests/test_capability.py::TestProbeStructure::test_device_selection_cpu PASSED [ 30%]
worker/tests/test_capability.py::TestProbeStructure::test_device_selection_rocm_does_not_raise PASSED [ 30%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp8_probe_still_called_for_non_cpu_device_type PASSED [ 31%]
worker/tests/test_executor.py::test_topo_sort_single_node PASSED         [ 32%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 33%]
worker/tests/test_executor.py::test_topo_sort_parallel_branches PASSED   [ 34%]
worker/tests/test_executor.py::test_topo_sort_cycle_detected PASSED      [ 34%]
worker/tests/test_executor.py::test_topo_sort_no_edges_key PASSED        [ 35%]
worker/tests/test_executor.py::test_topo_sort_empty_graph PASSED         [ 36%]
worker/tests/test_executor.py::test_topo_sort_missing_nodes_key PASSED   [ 37%]
worker/tests/test_executor.py::test_topo_sort_no_torch_import PASSED     [ 38%]
worker/tests/test_executor.py::test_execute_graph_cancel_before_first PASSED [ 39%]
worker/tests/test_executor.py::test_execute_graph_cancel_after_first PASSED [ 39%]
worker/tests/test_executor.py::test_execute_graph_no_cancel_completes PASSED [ 40%]
worker/tests/test_executor.py::test_execute_graph_execution_order_matches_topo_sort PASSED [ 41%]
worker/tests/test_executor.py::test_execute_graph_results_dict PASSED    [ 42%]
worker/tests/test_executor.py::test_execute_graph_no_torch_import PASSED [ 43%]
worker/tests/test_ipc.py::TestConnectIdentity::test_connect_sets_identity PASSED [ 43%]
worker/tests/test_ipc.py::TestPreConnectErrors::test_send_event_before_connect_raises PASSED [ 44%]
worker/tests/test_ipc.py::TestRecvMessage::test_recv_message_before_connect_raises PASSED [ 45%]
worker/tests/test_ipc.py::TestRoundtrip::test_roundtrip_send_recv PASSED [ 46%]
worker/tests/test_ipc.py::TestNoTorchImport::test_module_no_torch_import PASSED [ 47%]
worker/tests/test_ipc.py::TestContextReuse::test_connect_twice_reuses_context PASSED [ 47%]
worker/tests/test_ipc.py::TestRecvMessage::test_recv_message_decodes_real_router_framing PASSED [ 48%]
worker/tests/test_ipc.py::TestRecvMessage::test_recv_message_raises_before_connect PASSED [ 49%]
worker/tests/test_nodes_init.py::test_import_does_not_raise PASSED       [ 50%]
worker/tests/test_nodes_init.py::test_node_registry_empty_after_import PASSED [ 51%]
worker/tests/test_nodes_init.py::test_reimport_is_idempotent PASSED      [ 52%]
worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel PASSED [ 52%]
worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented PASSED [ 53%]
worker/tests/test_nodes_loader.py::test_load_model_in_registry PASSED    [ 54%]
worker/tests/test_nodes_loader.py::test_load_model_real_cache_key_format PASSED [ 55%]
worker/tests/test_nodes_loader.py::test_load_model_real_raises_no_diffusion_arch PASSED [ 56%]
worker/tests/test_nodes_loader.py::test_load_vae_mock_returns_sentinel PASSED [ 56%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented PASSED [ 57%]
worker/tests/test_nodes_loader.py::test_load_vae_in_registry PASSED      [ 58%]
worker/tests/test_nodes_loader.py::test_load_vae_real_cache_key_format PASSED [ 59%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_no_diffusion_arch PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_load_clip_mock_returns_sentinel PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_not_implemented PASSED [ 61%]
worker/tests/test_nodes_loader.py::test_load_clip_in_registry PASSED     [ 62%]
worker/tests/test_nodes_loader.py::test_load_clip_real_cache_key_format PASSED [ 63%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_no_diffusion_arch PASSED [ 64%]
worker/tests/test_passthrough.py::test_class_attributes PASSED           [ 65%]
worker/tests/test_passthrough.py::test_execute_mock_returns_input PASSED [ 65%]
worker/tests/test_passthrough.py::test_execute_real_returns_input PASSED [ 66%]
worker/tests/test_passthrough.py::test_node_in_registry_after_import PASSED [ 67%]
worker/tests/test_passthrough.py::test_markers_name_collectible_tests PASSED [ 68%]
worker/tests/test_passthrough.py::test_execute_returns_new_dict PASSED   [ 69%]
worker/tests/test_pipeline_cache.py::test_get_or_load_cached_returns_without_calling_loader PASSED [ 69%]
worker/tests/test_pipeline_cache.py::test_get_or_load_different_keys_each_call_loader PASSED [ 70%]
worker/tests/test_pipeline_cache.py::test_lru_eviction_removes_least_recently_used PASSED [ 71%]
worker/tests/test_pipeline_cache.py::test_access_refreshes_recency PASSED [ 72%]
worker/tests/test_pipeline_cache.py::test_custom_max_entries PASSED      [ 73%]
worker/tests/test_pipeline_cache.py::test_evicted_entry_is_truly_removed PASSED [ 73%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys PASSED [ 74%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool PASSED [ 75%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false PASSED [ 76%]
worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load PASSED [ 77%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_ipc_connect PASSED [ 78%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_cpu_skips_cuda_set_device PASSED [ 78%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_probe_capabilities PASSED [ 79%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_exit_path PASSED [ 80%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_sends_ready_event PASSED [ 81%]
worker/tests/test_worker_main.py::TestNoMockGate::test_import_nodes_returns_registered_nodes PASSED [ 82%]
worker/tests/test_worker_main.py::TestNoMockGate::test_dispatch_loop_exists_and_is_callable PASSED [ 82%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_no_nonzero_exit_for_cpu PASSED [ 83%]
worker/tests/test_worker_main.py::TestNoMockGate::test_mock_startup_sends_ready_event PASSED [ 84%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_in_main_block PASSED [ 85%]
worker/tests/test_worker_main.py::TestDispatchLoopPing::test_ping_receives_matching_pong PASSED [ 86%]
worker/tests/test_worker_main.py::TestDispatchLoopPing::test_multiple_pings_each_get_matching_pong PASSED [ 86%]
worker/tests/test_worker_main.py::TestDispatchLoopPing::test_non_ping_message_gets_no_pong PASSED [ 87%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_shutdown_message_exits_loop_cleanly PASSED [ 88%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_shutdown_after_other_messages_still_exits PASSED [ 89%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_keyboard_interrupt_during_recv_exits_cleanly PASSED [ 90%]
worker/tests/test_worker_main.py::TestDispatchLoopShutdown::test_keyboard_interrupt_after_other_messages_still_exits_cleanly PASSED [ 91%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_triggers_execute_graph_with_job_scoped_ctx_factory PASSED [ 91%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_success_sends_completed_with_elapsed_ms PASSED [ 92%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_on_background_thread_stays_responsive PASSED [ 93%]
worker/tests/test_worker_main.py::TestDispatchLoopExecute::test_execute_graph_called_with_correct_graph PASSED [ 94%]
worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_sends_failed_event PASSED [ 95%]
worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_error_contains_exception_message PASSED [ 95%]
worker/tests/test_worker_main.py::TestDispatchLoopExecuteFailure::test_execute_failure_traceback_is_populated PASSED [ 96%]
worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_sets_cancel_flag_for_current_job PASSED [ 97%]
worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_for_nonmatching_job_id_is_ignored PASSED [ 98%]
worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_cancelled_execution_sends_cancelled_event PASSED [ 99%]
worker/tests/test_worker_main.py::TestDispatchLoopCancelJob::test_canceljob_after_job_completed_is_ignored PASSED [100%]

============================= 123 passed in 4.91s ==============================
```

**Exit code: 0 — PASS**

Verified tests present:
- `test_pipeline_cache.py`: 6 tests (get_or_load_cached, get_or_load_different_keys, lru_eviction, access_refreshes_recency, custom_max_entries, evicted_entry_is_truly_removed)
- `test_nodes_loader.py` mock sentinel/registry: 6 tests (test_load_model/vae/clip_mock_returns_sentinel, test_load_model/vae/clip_in_registry)

### Real-mode test run (`python -m pytest worker/tests -v -m real_mode`)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 123 items / 92 deselected / 31 selected

worker/tests/test_capability.py::TestProbeDtypes::test_fp32_cpu_returns_true PASSED [  3%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp16_cpu_returns_true PASSED [  6%]
worker/tests/test_capability.py::TestProbeDtypes::test_bf16_cpu_returns_true PASSED [  9%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp8_cpu_returns_false PASSED [ 12%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp4_cpu_returns_false PASSED [ 16%]
worker/tests/test_capability.py::TestProbeDtypes::test_float8_tensor_construction_and_forward_succeed_on_cpu PASSED [ 19%]
worker/tests/test_capability.py::TestProbeFlashAttention::test_flash_attention_cpu_returns_true PASSED [ 22%]
worker/tests/test_capability.py::TestProbeStructure::test_returns_dict_with_exactly_six_bool_keys PASSED [ 25%]
worker/tests/test_capability.py::TestProbeStructure::test_never_raises_for_cpu PASSED [ 29%]
worker/tests/test_capability.py::TestProbeStructure::test_device_selection_cpu PASSED [ 32%]
worker/tests/test_capability.py::TestProbeStructure::test_device_selection_rocm_does_not_raise PASSED [ 35%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp8_probe_still_called_for_non_cpu_device_type PASSED [ 38%]
worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented PASSED [ 41%]
worker/tests/test_nodes_loader.py::test_load_model_real_cache_key_format PASSED [ 45%]
worker/tests/test_nodes_loader.py::test_load_model_real_raises_no_diffusion_arch PASSED [ 48%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_not_implemented PASSED [ 51%]
worker/tests/test_nodes_loader.py::test_load_vae_real_cache_key_format PASSED [ 54%]
worker/tests/test_nodes_loader.py::test_load_vae_real_raises_no_diffusion_arch PASSED [ 58%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_not_implemented PASSED [ 61%]
worker/tests/test_nodes_loader.py::test_load_clip_real_cache_key_format PASSED [ 64%]
worker/tests/test_nodes_loader.py::test_load_clip_real_raises_no_diffusion_arch PASSED [ 67%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_ipc_connect PASSED [ 70%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_cpu_skips_cuda_set_device PASSED [ 74%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_probe_capabilities PASSED [ 77%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_exit_path PASSED [ 80%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_sends_ready_event PASSED [ 83%]
worker/tests/test_worker_main.py::TestNoMockGate::test_import_nodes_returns_registered_nodes PASSED [ 87%]
worker/tests/test_worker_main.py::TestNoMockGate::test_dispatch_loop_exists_and_is_callable PASSED [ 90%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_no_nonzero_exit_for_cpu PASSED [ 93%]
worker/tests/test_worker_main.py::TestNoMockGate::test_mock_startup_sends_ready_event PASSED [ 96%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_in_main_block PASSED [100%]

====================== 31 passed, 92 deselected in 2.15s =======================
```

**Exit code: 0 — PASS**

Verified tests present:
- `test_nodes_loader.py` real-mode: 9 tests (test_load_model/vae/clip_real_raises_not_implemented, test_load_model/vae/clip_real_cache_key_format, test_load_model/vae/clip_real_raises_no_diffusion_arch)

## Format Gate

```
cargo fmt --all -- --check
```
Exit code: 0. No formatting drift detected.

## Platform Cross-Check

Not applicable — this task modifies no Rust source files. The existing CI matrix
(`rust-linux`, `rust-windows`, `worker-linux-mock`, `worker-linux-real`,
`worker-windows-mock`, `worker-windows-real`) already covers all platform targets.

## Project Gates

Gate 1 (Config Surface Sync): Not triggered — no `ServerConfig` fields added/removed.
Gate 2 (OpenAPI Drift): Not triggered — no handler signatures changed.
Gate 3 (Node Parity): Not triggered — no node types added/removed/renamed.
Gate 4 (Mock/Real Parity Markers): Not triggered — no `execute()` functions modified.

## Public API Delta

No new pub items introduced. This task performs verification only — no source files
were modified.

## Deviations from Plan

None. Implementation followed the approved plan exactly.

## Blockers

None. All acceptance criteria met:
- Mock-mode pytest exits 0 with all expected tests collected.
- Real-mode pytest exits 0 with all expected tests collected.
- CI wiring confirmed: `worker-test` job uses `worker/tests` glob for both mock and
  real-mode steps, auto-collecting new test files.
- §9a.1 findings (unmarked stubs for LoadVae and LoadClip `NotImplementedError` from
  P19-C3) are pre-existing task-authoring defects documented in the plan report; they
  are not introduced by this task and do not affect P19-E1's verification scope.
