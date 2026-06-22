# Implementation Report: P903-A2

| Field         | Value                                                |
|---------------|------------------------------------------------------|
| Task ID       | P903-A2                                              |
| Phase         | 903 — Pipeline Cache & Model Path Resolution Retrofit |
| Description   | worker/worker_main.py: wire real PipelineCache into NodeContext |
| Implemented   | 2026-06-22T08:12:00Z                                 |
| Status        | COMPLETE                                             |

## Summary

Replaced the `pipeline_cache={}` empty-dict placeholder in `worker/worker_main.py` with a real `PipelineCache` singleton created once at module scope. The same instance is now passed to every `NodeContext` constructed in the Execute handler, enabling cache entries (loaded model components) to persist across jobs dispatched to the same worker process. Added one test (`test_pipeline_cache_reused_across_jobs`) that verifies the same `PipelineCache` instance (by `id()`) is reused for two sequential Execute messages.

## Resolved Dependencies

None. This task uses only existing, already-imported modules within the worker package (`worker.pipeline_cache`). No new external packages or crates are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/worker_main.py` | Import `PipelineCache`, create module-level `_pipeline_cache` singleton, pass it to `NodeContext` instead of `{}` |
| MODIFY | `worker/tests/test_worker_main.py` | Add `test_pipeline_cache_reused_across_jobs` test |
| MODIFY | `docs/TESTS.md` | Add test catalogue entry for `test_pipeline_cache_reused_across_jobs` |

## Commit Log

```
 .forge/reports/P903-A2_plan.md   | 150 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 ++--
 docs/TESTS.md                    |   9 +++
 worker/tests/test_worker_main.py | 137 +++++++++++++++++++++++++++
 worker/worker_main.py            |  15 +++-
 6 files changed, 318 insertions(+), 12 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 65 items

worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [  1%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [  3%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [  4%]
worker/tests/test_arch_zit.py::test_sample_real_path_raises_not_implemented PASSED [  7%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [  9%]
worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [ 10%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [ 12%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [ 13%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [ 15%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 16%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 18%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED             [ 20%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED         [ 21%]
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED [ 23%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 24%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 26%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 27%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 29%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 30%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 32%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 33%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 35%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 36%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 38%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 40%]
worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry PASSED [ 41%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image PASSED [ 43%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock PASSED [ 46%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry PASSED [ 47%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning PASSED [ 49%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty PASSED [ 52%]
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [ 53%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 55%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 56%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 58%]
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED [ 61%]
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED [ 63%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 64%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 66%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 67%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [ 69%]
worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED [ 70%]
worker/tests/test_nodes_sampler.py::test_emptylatent_execute_returns_mock_latent PASSED [ 72%]
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED [ 75%]
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED [ 76%]
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED [ 78%]
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED [ 80%]
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED [ 81%]
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED [ 83%]
worker/tests/test_pipeline_cache.py::test_cache_hit PASSED               [ 84%]
worker/tests/test_pipeline_cache.py::test_cache_miss PASSED              [ 86%]
worker/tests/test_pipeline_cache.py::test_lru_eviction PASSED            [ 87%]
worker/tests/test_pipeline_cache.py::test_max_entries_one PASSED         [ 89%]
worker/tests/test_pipeline_cache.py::test_oom_evict_all_in_mock PASSED   [ 90%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 92%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 93%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED         [ 95%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED    [ 96%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [ 98%]
worker/tests/test_worker_main.py::test_pipeline_cache_reused_across_jobs PASSED [100%]

============================== 65 passed in 1.94s ==============================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, zero drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.39s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Gate 1: PASSED
```

Gate 2 (OpenAPI drift) and Gate 3 (Node parity) are not triggered — this task does not modify handler function signatures, `ToSchema` derives, `AppState` fields, node types, or `node_registry.rs`.

## Public API Delta

```
(no new pub items — grep returned zero matches)
```

No new `pub` items introduced. The only changes are:
- A new private module-level variable `_pipeline_cache: PipelineCache` (underscore-prefixed, private)
- A new private import `from worker.pipeline_cache import PipelineCache`
- An argument change in the existing `NodeContext(...)` call (internal to the Execute handler)

## Deviations from Plan

- The test implementation deviated from the plan's initial monkey-patch approach. The plan suggested monkey-patching `NodeContext.__init__` via a temp file mechanism. The actual implementation uses a `sitecustomize.py` module loaded at Python startup (via `PYTHONPATH`) to intercept `NodeContext.__init__` calls. An additional fix was needed: the `sitecustomize.py` must insert the repo root into `sys.path` before importing `worker.nodes.base`, because the venv's `site-packages` does not include the project root on `sys.path`.
- The plan's test used `graph: []` (a raw list). The actual `run_graph()` function expects `graph` to be a dict with a `"nodes"` key. The test was corrected to use `graph: {"nodes": []}`.
- No `defers_to` comment markers are needed — this task's `defers_to` field in `tasks_phase903.json` is empty.

## Blockers

None.
