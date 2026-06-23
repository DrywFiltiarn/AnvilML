# Implementation Report: P18-D16

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-D16                         |
| Phase         | 018 — ZiT Generic Nodes         |
| Description   | worker/nodes/encoder.py: ClipTextEncode real text encoding path |
| Implemented   | 2026-06-23T10:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the real-mode text encoding path for `ClipTextEncode` by adding an `encode()` method to `RealClip` in `loader.py`, creating a `Conditioning` class in `encoder.py`, and replacing the `NotImplementedError` stub in `ClipTextEncode.execute()` with a call to `clip.encode(text, negative_text)`. The method applies a chat template (Qwen3-style), tokenises with fixed-length padding, runs through the text encoder extracting `hidden_states[-2]`, filters by attention mask, and returns positive and negative embedding lists. In mock mode, empty lists are returned without importing torch.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | diffusers | 0.38.0           | pypi-query MCP |
| python | transformers | (installed)    | pypi-query MCP |

Notes: No new dependencies added. The implementation uses only `torch` (lazy-imported inside the non-mock guard) and existing transformers/diffusers APIs already present in the project.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Added `device: str = "cpu"` parameter to `RealClip.__init__`; added `encode()` method to `RealClip`. |
| Modify | `worker/nodes/encoder.py` | Created `Conditioning` class; updated `__all__`; updated module docstring; replaced `NotImplementedError` in `ClipTextEncode.execute()` with real encoding path. |
| Modify | `docs/TESTS.md` | Added test entry for `test_conditioning_class_has_positive_negative`. |
| Modify | `worker/tests/test_nodes_encoder.py` | Added `test_conditioning_class_has_positive_negative` test. |

## Commit Log

```
 .forge/reports/P18-D16_plan.md     | 137 ++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +--
 docs/TESTS.md                      |   9 ++
 worker/nodes/encoder.py            |  81 ++++++++++++++----
 worker/nodes/loader.py             | 165 ++++++++++++++++++++++++++++++++++++-
 worker/tests/test_nodes_encoder.py |  31 +++++++
 7 files changed, 416 insertions(+), 26 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 87 items

worker/tests/test_arch_clip_init.py::test_get_module_returns_dummy_for_dummy_clip_type PASSED
worker/tests/test_arch_clip_init.py::test_get_module_returns_none_for_unknown_clip_type PASSED
worker/tests/test_arch_clip_init.py::test_can_handle_returns_bools_correctly PASSED
worker/tests/test_arch_clip_l.py::test_can_handle_clip_l PASSED
worker/tests/test_arch_clip_l.py::test_can_handle_non_clip_l PASSED
worker/tests/test_arch_clip_l.py::test_load_mock_returns_realclip PASSED
worker/tests/test_arch_clip_l.py::test_load_mock_no_torch_import PASSED
worker/tests/test_arch_clip_qwen3.py::test_can_handle_qwen3 PASSED
worker/tests/test_arch_clip_qwen3.py::test_can_handle_non_qwen3 PASSED
worker/tests/test_arch_clip_qwen3.py::test_load_mock_returns_realclip PASSED
worker/tests/test_arch_clip_qwen3.py::test_load_mock_no_torch_import PASSED
worker/tests/test_arch_clip_t5.py::test_can_handle_t5 PASSED
worker/tests/test_arch_clip_t5.py::test_can_handle_non_t5 PASSED
worker/tests/test_arch_clip_t5.py::test_load_mock_returns_realclip PASSED
worker/tests/test_arch_clip_t5.py::test_load_mock_no_torch_import PASSED
worker/tests/test_arch_init.py::test_get_module_returns_zit_for_zit_model PASSED
worker/tests/test_arch_init.py::test_get_module_returns_none_for_unknown_arch PASSED
worker/tests/test_arch_init.py::test_can_handle_still_works_after_refactor PASSED
worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED
worker/tests/test_arch_zit.py::test_sample_real_path_raises_not_implemented PASSED
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED
worker/tests/test_executor.py::test_run_graph_topo_order PASSED
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED
worker/tests/test_executor.py::test_topo_sort_diamond PASSED
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED
worker/tests/test_ipc.py::test_connect_succeeds PASSED
worker/tests/test_ipc.py::test_connect_sets_identity PASSED
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED
worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry PASSED
worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image PASSED
worker/tests/test_nodes_decode.py::test_vaedeode_metadata_attributes PASSED
worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock PASSED
worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry PASSED
worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning PASSED
worker/tests/test_nodes_encoder.py::test_cliptextencode_metadata_attributes PASSED
worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty PASSED
worker/tests/test_nodes_encoder.py::test_conditioning_class_has_positive_negative PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED
worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED
worker/tests/test_pipeline_cache.py::test_cache_hit PASSED
worker/tests/test_pipeline_cache.py::test_cache_miss PASSED
worker/tests/test_pipeline_cache.py::test_lru_eviction PASSED
worker/tests/test_pipeline_cache.py::test_max_entries_one PASSED
worker/tests/test_pipeline_cache.py::test_oom_evict_all_in_mock PASSED
worker/tests/test_placeholder.py::test_placeholder PASSED
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED
worker/tests/test_worker_main.py::test_pipeline_cache_reused_across_jobs PASSED

============================== 87 passed in 2.02s ==============================
```

## Format Gate

```
(Exit 0 — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
---CHECK1_OK---

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
---CHECK2_OK---

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
---CHECK3_OK---

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
---CHECK4_OK---
```

All four platform cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
---GATE1_OK---
```

**Gate 3 — Node Parity:** Not triggered — this task does not add, remove, or rename a node type. It adds a `Conditioning` class (not a node) and modifies existing `ClipTextEncode` node's real-mode path.

## Public API Delta

```
+    def __init__(
+    def __init__(
+    def encode(
```

New/modified `pub` items (Python `def`):
1. `Conditioning.__init__(self, positive: list[Any], negative: list[Any])` — new class constructor.
2. `RealClip.__init__(self, tokenizer, text_encoder, device: str = "cpu")` — modified to add `device` parameter.
3. `RealClip.encode(self, text: str, negative_text: str = "") -> tuple[list[Any], list[Any]]` — new method.

All three match the plan's Public API Surface table.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
