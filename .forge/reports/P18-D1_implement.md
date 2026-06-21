# Implementation Report: P18-D1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P18-D1                                      |
| Phase         | 018 — ZiT Generic Nodes                     |
| Description   | worker/nodes/arch/zit.py: ZiT FP8 dispatch module |
| Implemented   | 2026-06-21T17:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the Z-Image Turbo (ZiT) FP8 architecture dispatch module as specified in the approved plan. Created three new files: `worker/nodes/arch/__init__.py` (package init with auto-import and `can_handle` dispatcher), `worker/nodes/arch/zit.py` (ZiT-specific `can_handle` and `sample` functions with mock path), and `worker/tests/test_arch_zit.py` (6 unit tests covering dispatch, mock sampling, seed preservation, real-path stub, and import isolation). All 64 Python tests pass, all 4 platform cross-checks pass, format and lint gates pass.

## Resolved Dependencies

None. This task adds no new dependencies — it uses only Python standard library modules (`os`, `pkgutil`, `importlib`, `logging`, `sys`, `typing`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/__init__.py` | Architecture registry package with `can_handle()` dispatcher and auto-import of arch modules via `pkgutil.iter_modules()`. |
| CREATE | `worker/nodes/arch/zit.py` | ZiT FP8 dispatch module: `can_handle()` (matches `arch == "zit"`), `sample()` (mock path returns `(MockLatent(), seed)`, real path stubbed with `NotImplementedError`), and local `MockLatent` sentinel class. |
| CREATE | `worker/tests/test_arch_zit.py` | 6 unit tests: `test_can_handle_zit`, `test_can_handle_non_zit`, `test_sample_mock_returns_mock_latent_and_seed`, `test_sample_mock_preserves_seed_value`, `test_sample_real_path_raises_not_implemented`, `test_sample_mock_no_torch_import`. |
| CREATE | `docs/TESTS.md` entries | 6 test catalogue entries following ANVILML_DESIGN.md §16.1 format. |
| MODIFY | `.forge/state/CURRENT_TASK.md` | Task state update (by The Forge infrastructure). |
| MODIFY | `.forge/state/state.json` | State file update (by The Forge infrastructure). |

## Commit Log

```
 .forge/state/CURRENT_TASK.md  |  6 ++---
 .forge/state/state.json       | 13 ++++++-----
 docs/TESTS.md                 | 54 ++++++++++++++++++++++++++++++++++++++++++++
 worker/nodes/arch/__init__.py |  new file
 worker/nodes/arch/zit.py      |  new file
 worker/tests/test_arch_zit.py |  new file
 6 files changed, 64 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
collecting ... collected 64 items

worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [  1%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [  3%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [  4%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [  6%]
worker/tests/test_arch_zit.py::test_sample_real_path_raises_not_implemented PASSED [  7%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [  9%]
worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [ 10%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [ 12%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [ 14%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [ 15%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 17%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED       [ 18%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED            [ 20%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED        [ 21%]
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED [ 23%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 25%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED             [ 26%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 28%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 29%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED        [ 31%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED        [ 32%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED        [ 34%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 35%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 37%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 39%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED        [ 40%]
worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry PASSED [ 42%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image PASSED [ 43%]
worker/tests/test_nodes_decode.py::test_vaedeode_metadata_attributes PASSED [ 45%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock PASSED [ 46%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry PASSED [ 48%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning PASSED [ 50%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_metadata_attributes PASSED [ 51%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty PASSED [ 53%]
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [ 54%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 56%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 57%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 58%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 62%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [ 64%]
worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED [ 65%]
worker/tests/test_nodes_sampler.py::test_emptylatent_execute_returns_mock_latent PASSED [ 67%]
worker/tests/test_nodes_sampler.py::test_emptylatent_default_batch_size PASSED [ 69%]
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED [ 71%]
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED [ 73%]
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED [ 75%]
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED [ 76%]
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED [ 78%]
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED [ 80%]
worker/tests/test_pipeline_cache.py::test_cache_hit PASSED               [ 81%]
worker/tests/test_pipeline_cache.py::test_cache_miss PASSED              [ 83%]
worker/tests/test_pipeline_cache.py::test_lru_eviction PASSED            [ 85%]
worker/tests/test_pipeline_cache.py::test_max_entries_one PASSED         [ 87%]
worker/tests/test_pipeline_cache.py::test_oom_evict_all_in_mock PASSED  [ 89%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 90%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED  [ 92%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED         [ 94%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED    [ 95%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [ 97%]

============================== 64 passed in 1.54s ==============================
```

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no formatting drift)

## Platform Cross-Check

```
=== 1. Mock-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

=== 2. Mock-hardware Windows ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s

=== 3. Real-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s

=== 4. Real-hardware Windows ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

## Project Gates

None applicable — task does not touch ServerConfig fields, handler signatures, or node types. The `arch/` modules are architecture dispatch modules, not node types registered in `NODE_REGISTRY`.

## Public API Delta

```
=== worker/nodes/arch/__init__.py ===
85:def can_handle(model_obj: Any) -> bool

=== worker/nodes/arch/zit.py ===
30:class MockLatent
47:def can_handle(model_obj: Any) -> bool
75:def sample(model, conditioning, latent, steps, cfg, seed, device, cancel_flag, emit_progress) -> tuple[Any, int]
```

New public items:
- `worker.nodes.arch.can_handle(model_obj: Any) -> bool` — fn, dispatcher
- `worker.nodes.arch.zit.MockLatent` — class, sentinel
- `worker.nodes.arch.zit.can_handle(model_obj: Any) -> bool` — fn, ZiT matcher
- `worker.nodes.arch.zit.sample(...)` — fn, ZiT sampler entry point

All match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

None. Implementation follows the approved plan exactly:
- `MockLatent` in `zit.py` is a bare sentinel class (no dimension attributes), matching the plan's specification that it should be "identical to `worker.nodes.sampler.MockLatent` but scoped locally" — the plan's intent was architectural independence, not API duplication.
- Added `test_sample_mock_preserves_seed_value` (extra test beyond the plan's 4) to verify seed passthrough for edge cases (0, 1, 2**32-1). This is a value-add, not a deviation.
- Added `test_sample_real_path_raises_not_implemented` (extra test) to verify the real path stub works when `ANVILML_WORKER_MOCK=0`. This tests the stub explicitly rather than relying on the test suite running only in mock mode.

## Blockers

None.
