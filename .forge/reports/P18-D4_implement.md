# Implementation Report: P18-D4

| Field         | Value                                      |
|---------------|--------------------------------------------|
| Task ID       | P18-D4                                     |
| Phase         | 018 — ZiT Generic Nodes                    |
| Description   | worker/nodes/loader.py: LoadModel real safetensors loading path |
| Implemented   | 2026-06-22T12:00:00Z                       |
| Status        | COMPLETE                                   |

## Summary

Replaced `LoadModel.execute()`'s stubbed real-path `NotImplementedError` with actual safetensors-based model loading. Added a `RealModel` lightweight wrapper class that exposes `.arch` (str) and `.in_channels` (int) for downstream consumers. The real path uses `safetensors.torch.safe_open()` to read model metadata, detects architecture from safetensors metadata with a directory-naming fallback, constructs a `diffusers.ZImageTransformer2DModel` via `from_pretrained()`, caches it via `ctx.pipeline_cache.get_or_load(model_id, "fp8", loader_fn)`, and returns `{"model": result}`. All 71 existing mock-mode tests pass unchanged.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| python | diffusers   | 0.38.0           | pypi-query MCP |
| python | safetensors | 0.8.0            | pypi-query MCP |

Both packages were already declared in `worker/requirements/base.txt`. The `ZImageTransformer2DModel` class was confirmed to exist in diffusers 0.38.0 via venv inspection. The `safe_open` function was confirmed in `safetensors.torch` (requires torch to be installed at import time, which is guaranteed in the real-path branch).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/nodes/loader.py | Added `RealModel` wrapper class; replaced `NotImplementedError` in `LoadModel.execute()` with real safetensors loading logic including lazy imports, architecture detection, pipeline cache integration |

## Commit Log

```
 .forge/state/CURRENT_TASK.md |   6 +--
 .forge/state/state.json      |  13 +++---
 worker/nodes/loader.py       | 104 +++++++++++++++++++++++++++++++++++++++----
 3 files changed, 105 insertions(+), 18 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 71 items

worker/tests/test_arch_init.py::test_get_module_returns_zit_for_zit_model PASSED [  1%]
worker/tests/test_arch_init.py::test_get_module_returns_none_for_unknown_arch PASSED [  2%]
worker/tests/test_arch_init.py::test_can_handle_still_works_after_refactor PASSED [  4%]
worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [  5%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [  7%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [  8%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [  9%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [ 11%]
worker/tests/test_arch_zit.py::test_sample_real_path_raises_not_implemented PASSED [ 12%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [ 14%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [ 15%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [ 16%]
worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [ 18%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [ 19%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [ 21%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [ 23%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 23%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 25%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED             [ 26%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED         [ 28%]
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED [ 29%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 30%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 32%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 33%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 35%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 36%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 38%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 39%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 40%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 42%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 43%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 45%]
worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry PASSED [ 46%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image PASSED [ 47%]
worker/tests/test_nodes_decode.py::test_vaedeode_metadata_attributes PASSED [ 49%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock PASSED [ 50%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry PASSED [ 52%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning PASSED [ 53%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_metadata_attributes PASSED [ 54%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty PASSED [ 56%]
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [ 57%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 59%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 60%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 61%]
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED [ 63%]
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED [ 64%]
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED [ 66%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 67%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 69%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 70%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [ 71%]
worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED [ 73%]
worker/tests/test_nodes_sampler.py::test_emptylatent_execute_returns_mock_latent PASSED [ 74%]
worker/tests/test_nodes_sampler.py::test_emptylatent_default_batch_size PASSED [ 76%]
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED [ 77%]
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED [ 78%]
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED [ 80%]
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED [ 81%]
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED [ 83%]
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED [ 84%]
worker/tests/test_pipeline_cache.py::test_cache_hit PASSED               [ 85%]
worker/tests/test_pipeline_cache.py::test_cache_miss PASSED              [ 87%]
worker/tests/test_pipeline_cache.py::test_lru_eviction PASSED            [ 88%]
worker/tests/test_pipeline_cache.py::test_max_entries_one PASSED         [ 90%]
worker/tests/test_pipeline_cache.py::test_oom_evict_all_in_mock PASSED   [ 91%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 92%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 94%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED          [ 95%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED     [ 97%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [ 98%]
worker/tests/test_worker_main.py::test_pipeline_cache_reused_across_jobs PASSED [100%]

============================== 71 passed in 1.97s ==============================
```

## Format Gate

```
(Not applicable — cargo fmt --all -- --check exited 0 with no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s

# 3. Real-hardware Linux:
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
```

## Project Gates

None applicable — task does not touch config fields, handler signatures, or node type additions/removals.

## Public API Delta

```
(no output — Python code uses no `pub` keyword)
```

No new pub items introduced. The `RealModel` class is internal to `loader.py` and is not added to `__all__`.

## Deviations from Plan

- The plan's `loader_fn()` closure references `torch_dtype=torch_dtype` as a variable, but `torch_dtype` was not defined in the plan. I resolved this by adding `import torch` to the lazy imports block and using `torch.float16` as the dtype. This is consistent with how FP8 safetensors models are loaded in diffusers (loaded as FP16, kept at FP8 by InferenceCaps during inference).
- The `py_compile` step in ENVIRONMENT.md §7 Step 7 uses `git ls-files 'worker/*.py'` which compiles all Python files in the worker directory. This passed with no errors.

## Blockers

None.
