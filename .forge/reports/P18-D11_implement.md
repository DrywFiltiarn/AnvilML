# Implementation Report: P18-D11

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P18-D11                            |
| Phase         | 018 — ZiT Generic Nodes            |
| Description   | worker/nodes/arch/clip/t5.py: single-file T5-XXL text encoder loading |
| Implemented   | 2026-06-23T02:00:00Z               |
| Status        | COMPLETE                           |

## Summary

Created `worker/nodes/arch/clip/t5.py`, the T5-XXL text encoder architecture dispatch module that enables `LoadClip` to load a real T5 text encoder from a single `.safetensors` file. The module provides `can_handle("t5")` dispatching and `load()` with mock-mode isolation. Also modified `worker/nodes/loader.py` to switch the T5 branch from `T5Tokenizer` to `T5TokenizerFast`, keeping the loader.py path consistent with the t5.py real-mode usage. Created 4 tests covering can_handle dispatch, mock load return type, and import isolation. All 86 Python tests and ~200 Rust tests pass.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| python | transformers| 5.12.x           | pypi-query MCP |
| python | safetensors | 0.8.x            | pypi-query MCP |

Both `T5TokenizerFast` and `T5EncoderModel` are standard, stable APIs in `transformers>=5.12`. The `safetensors.torch.load_file` function is the standard single-file loader. No new dependency versions were written into any manifest file (no manifest changes in this task).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/arch/clip/t5.py` | T5-XXL text encoder dispatch module with `can_handle()` and `load()` |
| MODIFY | `worker/nodes/loader.py` | Switch T5Tokenizer → T5TokenizerFast in the T5 branch (lines 510–517) |
| CREATE | `worker/tests/test_arch_clip_t5.py` | 4 tests: can_handle dispatch, mock load, import isolation |
| MODIFY | `docs/TESTS.md` | Added 4 test catalogue entries for the new T5 tests |

## Commit Log

```
 .forge/reports/P18-D11_plan.md    | 117 +++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +--
 docs/TESTS.md                     |  36 +++++++++
 worker/nodes/arch/clip/t5.py      | 135 ++++++++++++++++++++++++++++++++
 worker/nodes/loader.py            |   9 ++-
 worker/tests/test_arch_clip_t5.py | 161 ++++++++++++++++++++++++++++++++++++++
 7 files changed, 465 insertions(+), 12 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 86 items

worker/tests/test_arch_clip_init.py::test_get_module_returns_dummy_for_dummy_clip_type PASSED [  1%]
worker/tests/test_arch_clip_init.py::test_get_module_returns_none_for_unknown_clip_type PASSED [  2%]
worker/tests/test_arch_clip_init.py::test_can_handle_returns_bools_correctly PASSED [  3%]
worker/tests/test_arch_clip_l.py::test_can_handle_clip_l PASSED          [  4%]
worker/tests/test_arch_clip_l.py::test_can_handle_non_clip_l PASSED      [  5%]
worker/tests/test_arch_clip_l.py::test_load_mock_returns_realclip PASSED [  6%]
worker/tests/test_arch_clip_l.py::test_load_mock_no_torch_import PASSED  [  8%]
worker/tests/test_arch_clip_qwen3.py::test_can_handle_qwen3 PASSED       [  9%]
worker/tests/test_arch_clip_qwen3.py::test_can_handle_non_qwen3 PASSED   [ 10%]
worker/tests/test_arch_clip_qwen3.py::test_load_mock_returns_realclip PASSED [ 11%]
worker/tests/test_arch_clip_qwen3.py::test_load_mock_no_torch_import PASSED [ 12%]
worker/tests/test_arch_clip_t5.py::test_can_handle_t5 PASSED             [ 13%]
worker/tests/test_arch_clip_t5.py::test_can_handle_non_t5 PASSED         [ 15%]
worker/tests/test_arch_clip_t5.py::test_load_mock_returns_realclip PASSED [ 16%]
worker/tests/test_arch_clip_t5.py::test_load_mock_no_torch_import PASSED [ 17%]
worker/tests/test_arch_init.py::test_get_module_returns_zit_for_zit_model PASSED [ 18%]
worker/tests/test_arch_zit.py::test_vae_scale_factor_value PASSED        [ 22%]
worker/tests/test_arch_zit.py::test_can_handle_zit PASSED                [ 23%]
worker/tests/test_arch_zit.py::test_can_handle_non_zit PASSED            [ 24%]
worker/tests/test_arch_zit.py::test_sample_mock_returns_mock_latent_and_seed PASSED [ 25%]
worker/tests/test_arch_zit.py::test_sample_mock_preserves_seed_value PASSED [ 26%]
worker/tests/test_arch_zit.py::test_sample_real_path_raises_not_implemented PASSED [ 27%]
worker/tests/test_arch_zit.py::test_sample_mock_no_torch_import PASSED   [ 29%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims PASSED [ 30%]
worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible PASSED [ 30%]
worker/tests/test_executor.py::test_run_graph_topo_order PASSED          [ 32%]
worker/tests/test_executor.py::test_saveimage_emits_image_ready PASSED   [ 33%]
worker/tests/test_executor.py::test_completed_sent_after_run_graph PASSED [ 34%]
worker/tests/test_executor.py::test_failed_sent_on_node_error PASSED     [ 36%]
worker/tests/test_executor.py::test_topo_sort_cycle_detection PASSED     [ 37%]
worker/tests/test_executor.py::test_topo_sort_linear_chain PASSED        [ 38%]
worker/tests/test_executor.py::test_topo_sort_diamond PASSED             [ 39%]
worker/tests/test_executor.py::test_run_graph_empty_graph PASSED         [ 40%]
worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode PASSED [ 41%]
worker/tests/test_ipc.py::test_connect_succeeds PASSED                   [ 43%]
worker/tests/test_ipc.py::test_connect_sets_identity PASSED              [ 44%]
worker/tests/test_ipc.py::test_send_event_encodes_type_discriminator PASSED [ 45%]
worker/tests/test_ipc.py::test_recv_message_deserialises_correctly PASSED [ 46%]
worker/tests/test_ipc.py::test_roundtrip_via_pair_sockets PASSED         [ 47%]
worker/tests/test_ipc.py::test_send_before_connect_raises PASSED         [ 48%]
worker/tests/test_ipc.py::test_recv_before_connect_raises PASSED         [ 50%]
worker/tests/test_nodes_base.py::test_registry_populated_after_import PASSED [ 51%]
worker/tests/test_nodes_base.py::test_register_decorator_adds_class PASSED [ 52%]
worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated PASSED [ 53%]
worker/tests/test_nodes_base.py::test_slot_spec_dataclass PASSED         [ 54%]
worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry PASSED [ 55%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image PASSED [ 56%]
worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock PASSED [ 58%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_registered_in_registry PASSED [ 60%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_execute_returns_mock_conditioning PASSED [ 61%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_metadata_attributes PASSED [ 62%]
worker/tests/test_nodes_encoder.py::test_cliptextencode_negative_text_defaults_to_empty PASSED [ 63%]
worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED [ 65%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED [ 66%]
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED [ 67%]
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED [ 68%]
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED [ 69%]
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED [ 70%]
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED [ 72%]
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED [ 73%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED [ 74%]
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED [ 75%]
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED [ 76%]
worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED [ 77%]
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED [ 81%]
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED [ 82%]
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED [ 83%]
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED [ 84%]
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED [ 86%]
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED [ 87%]
worker/tests/test_pipeline_cache.py::test_cache_hit PASSED               [ 88%]
worker/tests/test_pipeline_cache.py::test_cache_miss PASSED              [ 89%]
worker/tests/test_pipeline_cache.py::test_lru_eviction PASSED            [ 90%]
worker/tests/test_pipeline_cache.py::test_max_entries_one PASSED         [ 91%]
worker/tests/test_pipeline_cache.py::test_oom_evict_all_in_mock PASSED   [ 93%]
worker/tests/test_placeholder.py::test_placeholder PASSED                [ 94%]
worker/tests/test_worker_main.py::test_mock_startup_sends_ready PASSED   [ 95%]
worker/tests/test_worker_main.py::test_ping_returns_pong PASSED          [ 96%]
worker/tests/test_worker_main.py::test_shutdown_exits_cleanly PASSED     [ 97%]
worker/tests/test_worker_main.py::test_env_vars_read_from_environment PASSED [ 98%]
worker/tests/test_worker_main.py::test_pipeline_cache_reused_across_jobs PASSED [100%]

============================== 86 passed in 2.00s ==============================
```

## Format Gate

```
Format pass 2 PASSED
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s
CHECK 1 PASSED

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.62s
CHECK 2 PASSED

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
CHECK 3 PASSED

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
CHECK 4 PASSED
```

## Project Gates

```
# Gate 1 — Config Surface Sync
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Gate 2 (OpenAPI Drift) and Gate 3 (Node Parity) are not triggered by this task — no handler function signatures, ToSchema derives, or node types were modified.

## Public API Delta

```
=== t5.py new def items ===
def can_handle(clip_type: str) -> bool:
def load(model_id: str, torch_dtype: Any) -> RealClip:  # noqa: F821
```

Two new public functions introduced in `worker.nodes.arch.clip.t5`:
- `can_handle(clip_type: str) -> bool` — matches `__all__` entry
- `load(model_id: str, torch_dtype: Any) -> RealClip` — matches `__all__` entry

No new `pub` Rust items introduced (this task only touches Python files).

## Deviations from Plan

- **Tokenizer path**: The plan specified `Path(__file__).parent.parent / "assets" / "t5_tokenizer"` which would resolve to `worker/nodes/arch/assets/t5_tokenizer/`. The actual tokenizer assets are at `worker/assets/t5_tokenizer/`, requiring `Path(__file__).parent.parent.parent / "assets" / "t5_tokenizer"`. This deviation was necessary to match the actual directory layout used by `qwen3.py` and `clip_l.py`. Documented with an inline comment in the code.
- **loader.py comment update**: The plan specified a mechanical one-line change to the import and assignment. I also updated the comment block (lines 510–513) to mention `T5TokenizerFast` and explain why it is used instead of the slow `T5Tokenizer` (performance benefit for real-time inference). This is a minimal documentation improvement that makes the code self-documenting.

## Blockers

None.
