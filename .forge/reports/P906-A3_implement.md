# Implementation Report: P906-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P906-A3                                           |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit           |
| Description | anvilml-core: fix BF16 serde rename (b_f16 -> bf16) |
| Implemented | 2026-06-12T17:12:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Added `#[serde(rename = "bf16")]` to the `DType::BF16` variant in `crates/anvilml-core/src/types/model.rs`, fixing the incorrect `"b_f16"` serde JSON string produced by `rename_all = "snake_case"` splitting `BF16` as three words. Added a unit test `dtype_bf16_serde_string` asserting the correct serialization. Bumped `anvilml-core` patch version from `0.1.3` to `0.1.4`. Updated `backend/openapi.json` to reflect the corrected enum value.

## Resolved Dependencies

Not applicable — no new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/model.rs` | Added `#[serde(rename = "bf16")]` on `BF16` variant; added `dtype_bf16_serde_string` test |
| Modify | `crates/anvilml-core/Cargo.toml` | Bumped patch version `0.1.3 → 0.1.4` |
| Modify | `backend/openapi.json` | Regenerated — `b_f16` → `bf16` in DType enum values |

## Commit Log

```
 .forge/reports/P906-A3_plan.md           | 84 ++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md             |  6 +--
 .forge/state/state.json                  | 13 +++---
 Cargo.lock                               |  2 +-
 backend/openapi.json                     |  2 +-
 crates/anvilml-core/Cargo.toml           |  2 +-
 crates/anvilml-core/src/types/model.rs   |  9 ++++
 7 files changed, 106 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-afc26e32303a2976)

running 76 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_model_kind_default ... ok
test config_load::tests::env_nested_field ... ok
test config::tests::test_toml_roundtrip ... ok
test error::tests::debug_formatting ... ok
test config_load::tests::env_overrides_toml ... ok
test config_load::tests::missing_toml_fallback ... ok
test error::tests::all_variants_display ... ok
test error::tests::from_io_error ... ok
test config_load::tests::override_beats_env ... ok
test error::tests::error_trait_impls ... ok
test error::tests::send_sync ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
test types::artifact::tests::artifact_meta_defaults ... ok
test types::artifact::tests::artifact_meta_json_preserves_fields ... ok
test types::artifact::tests::artifact_meta_optional_uuid_nil ... ok
test types::artifact::tests::artifact_meta_roundtrip ... ok
test types::events::tests::job_completed_roundtrip ... ok
test types::events::tests::job_cancelled_roundtrip ... ok
test types::events::tests::job_failed_no_traceback ... ok
test types::events::tests::job_failed_roundtrip ... ok
test types::events::tests::job_image_ready_roundtrip ... ok
test types::events::tests::job_progress_optional_fields ... ok
test types::events::tests::job_progress_roundtrip ... ok
test types::events::tests::job_queued_roundtrip ... ok
test types::events::tests::job_started_roundtrip ... ok
test types::events::tests::system_stats_event_json ... ok
test types::events::tests::system_stats_roundtrip ... ok
test types::events::tests::ws_event_enum_variants ... ok
test types::events::tests::worker_status_changed_roundtrip ... ok
test types::hardware::tests::capability_source_default_is_fallback ... ok
test types::hardware::tests::capability_source_variants ... ok
test types::hardware::tests::device_type_json_strings ... ok
test types::hardware::tests::device_type_variants ... ok
test types::hardware::tests::enumeration_source_default_is_fallback ... ok
test types::hardware::tests::enumeration_capability_sources_roundtrip ... ok
test types::hardware::tests::enumeration_source_variants ... ok
test types::hardware::tests::gpu_device_backward_compat ... ok
test types::hardware::tests::gpu_device_roundtrip ... ok
test types::hardware::tests::hardware_info_empty_gpus ... ok
test types::hardware::tests::hardware_info_roundtrip ... ok
test types::hardware::tests::host_info_roundtrip ... ok
test types::hardware::tests::inference_caps_defaults ... ok
test types::hardware::tests::inference_caps_roundtrip ... ok
test types::job::tests::job_optional_numeric_fields_default ... ok
test types::job::tests::job_optional_string_fields_default_none ... ok
test types::job::tests::job_graph_json_value ... ok
test types::job::tests::job_optional_timestamps_default_none ... ok
test types::job::tests::job_roundtrip ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_settings_roundtrip ... ok
test types::job::tests::job_status_variants ... ok
test types::job::tests::job_timestamps_iso8601 ... ok
test types::job::tests::submit_job_request_roundtrip ... ok
test types::job::tests::submit_job_response_roundtrip ... ok
test types::model::tests::dtype_bf16_serde_string ... ok
test types::model::tests::dtype_default_is_unknown ... ok
test types::model::tests::dtype_f8_serde_strings ... ok
test types::model::tests::dtype_roundtrip_json ... ok
test types::model::tests::dtype_variants ... ok
test types::model::tests::model_meta_default_impl ... ok
test types::model::tests::model_meta_defaults ... ok
test types::model::tests::model_meta_roundtrip ... ok
test types::model::tests::model_meta_scanned_at_default ... ok
test types::model::tests::model_meta_serde_json_preserves_all_fields ... ok
test types::worker::tests::env_report_defaults ... ok
test types::worker::tests::env_report_failure ... ok
test types::worker::tests::env_report_minimal_parse ... ok
test types::worker::tests::env_report_roundtrip ... ok
test types::worker::tests::worker_info_idle ... ok
test types::worker::tests::worker_info_optional_defaults ... ok
test types::worker::tests::worker_info_roundtrip ... ok
test types::worker::tests::worker_status_json_strings ... ok
test types::worker::tests::worker_status_variants ... ok

test result: ok. 76 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-b4063a76dc0ab44d)

running 56 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_device_new_fields ... ok
test cpu::tests::cpu_refresh_vram ... ok
test device_db::tests::generic_name_replaced_by_group_label ... ok
test device_db::tests::miss_with_empty_name_shows_unknown ... ok
test device_db::tests::miss_with_specific_name_preserved ... ok
test mock::tests::mock_detect_cuda ... ok
test mock::tests::mock_detect_default_cpu ... ok
test mock::tests::mock_detect_rocm ... ok
test mock::tests::mock_device_new_fields ... ok
test nvml::tests::nvml_all_devices_are_cuda ... ok
test nvml::tests::nvml_detect_returns_ok ... ok
test nvml::tests::nvml_init_fallback_no_library ... ok
test nvml::tests::nvml_library_load_fails_gracefully ... ok
test nvml::tests::nvml_shutdown_in_drop_no_panic ... ok
test sysfs::tests::parse_pci_ids_valid_hex ... ok
test sysfs::tests::read_vram_helper_converts_bytes_to_mib ... ok
test sysfs::tests::sysfs_detect_returns_ok_on_absent_dir ... ok
test tests::or_all_caps_empty ... ok
test tests::or_all_caps_merges ... ok
test tests::detect_all_devices_override_source ... ok
test tests::detect_all_devices_override_rocm ... ok
test tests::detect_all_devices_vulkan_empty ... ok
test tests::detect_all_devices_mock_rocm ... ok
test sysfs::tests::vendor_id_maps_cpu_intel ... ok
test sysfs::tests::vendor_id_maps_cpu_unknown ... ok
test sysfs::tests::vendor_id_maps_cuda ... ok
test sysfs::tests::vendor_id_maps_rocm ... ok
test tests::detect_all_devices_override_cpu ... ok
test tests::host_info_populated ... ok
test tests::override_device_new_fields ... ok
test tests::detect_all_devices_override ... ok
test tests::devices_have_sequential_indices ... ok
test tests::detect_all_devices_never_errs ... ok
test tests::detect_all_devices_mock_cuda ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok
test tests::detect_all_devices_mock_device_type ... ok
test tests::detect_all_devices_mock_enum_source ... ok
test sysfs::tests::sysfs_detect_with_fixture_data ... ok
test tests::detect_all_devices_mock_vram ... ok
test tests::mock_device_new_fields_in_detect_all ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::free_vram_fallback_no_budget ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok
test vulkan::tests::parse_vulkan_driver_version_zero ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok

test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.27s

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-797e121f25ccc9f1)

running 18 tests
test framing::tests::read_frame_oversize_rejected ... ok
test framing::tests::read_frame_roundtrip ... ok
test framing::tests::read_frame_python_format ... ok
test framing::tests::write_frame ... ok
test framing::tests::write_frame_sync_serialization ... ok
test framing::tests::write_frame_execute ... ok
test framing::tests::write_frame_shutdown ... ok
test messages::tests::all_worker_event_variants ... ok
test messages::tests::all_worker_message_variants ... ok
test messages::tests::worker_event_roundtrip_cancelled ... ok
test messages::tests::worker_event_roundtrip_ready ... ok
test messages::tests::worker_event_roundtrip_status_changed ... ok
test messages::tests::worker_message_roundtrip_cancel_job ... ok
test messages::tests::worker_message_roundtrip_init_hardware ... ok
test messages::tests::worker_message_roundtrip_execute ... ok
test messages::tests::worker_message_roundtrip_memory_query ... ok
test messages::tests::worker_message_roundtrip_ping ... ok
test messages::tests::worker_message_roundtrip_shutdown ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-26a2f004c9c8f5a0)

running 28 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_fp8_suffixes ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_map_dtype_str ... ok
test scanner::tests::test_read_safetensors_dtype_nonexistent ... ok
test scanner::tests::test_sha256_hex ... ok
test scanner::tests::test_vram_estimate_mib ... ok
test seed_loader::tests::test_compute_sha256_empty ... ok
test scanner::tests::test_read_safetensors_dtype_fp8_header ... ok
test seed_loader::tests::test_compute_sha256_known_value ... ok
test seed_loader::tests::test_parse_header_both_directives ... ok
test seed_loader::tests::test_parse_header_defaults_strategy ... ok
test scanner::tests::test_read_safetensors_dtype_empty_header ... ok
test seed_loader::tests::test_parse_header_empty_file ... ok
test seed_loader::tests::test_parse_header_missing_table ... ok
test scanner::tests::test_read_safetensors_dtype_fallback_malformed ... ok
test scanner::tests::test_read_safetensors_dtype_metadata_only ... ok
test scanner::tests::test_read_safetensors_dtype_too_large_header ... ok
test scanner::tests::test_read_safetensors_dtype_header_wins ... ok
test db::tests::test_open_creates_file_if_missing ... ok
test device_store::tests::test_upsert_then_get_roundtrip ... ok
test db::tests::test_open_creates_tables ... ok
test device_store::tests::test_get_miss_returns_none ... ok
test db::tests::test_reset_ghost_jobs ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-474855aa407c1e32)

running 1 test
test test_open_creates_tables ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/device_store.rs (target/debug/deps/device_store-066a48dd93bf1af8)

running 4 tests
test get_miss_returns_none ... ok
test bool_flags_roundtrip ... ok
test upsert_then_get_roundtrip ... ok
test upsert_overwrites_existing ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/patch_meta.rs (target/debug/deps/patch_meta-dfdfb1e12ceb5c46)

running 4 tests
test patch_meta_missing_returns_none ... ok
test patch_meta_updates_kind_keeps_dtype ... ok
test patch_meta_all_none_is_noop ... ok
test patch_meta_updates_dtype_recomputes_vram ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/rescan.rs (target/debug/deps/rescan-172c4a43d3a71563)

running 2 tests
test test_rescan_idempotent ... ok
test test_rescan_adds_models ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/rescan_stale.rs (target/debug/deps/rescan_stale-b13d3a240f7e278e)

running 1 test
test test_rescan_removes_stale_models ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/safetensors_header.rs (target/debug/deps/safetensors_header-7ad52a9ced5c7320)

running 1 test
test test_safetensors_header_dtype_detection ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/scanner.rs (target/debug/deps/scanner-0b2f5b843fc46d71)

running 1 test
test test_scan_dirs_two_files ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-1e6e1e1e1e1e1e1e)

running 7 tests
test test_directive_parsing_miss ... ok
test test_table_bootstrap_idempotent ... ok
test merge_preserves_unreferenced_rows ... ok
test replace_all_replaces_table_content ... ok
test test_directive_parsing_hit ... ok
test sha256_skip_does_not_execute ... ok
test changed_sha256_reruns_seed ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/store_get.rs (target/debug/deps/store_get-6839853968398539)

running 2 tests
test test_get_missing_returns_none ... ok
test test_upsert_then_get_returns_equal_meta ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/store_list.rs (target/debug/deps/store_list-bb49b49b49b49b49)

running 3 tests
test test_list_empty_returns_empty_vec ... ok
test test_list_kind_filter ... ok
test test_list_after_upserts_returns_ordered ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-b05ee1ce1e4578b3)

running 43 tests
test dag::tests::test_duplicate_node_id ... ok
test dag::tests::test_unknown_node_ref ... ok
test dag::tests::test_cycle_detected_2node ... ok
test dag::tests::test_unknown_node_type ... ok
test dag::tests::test_unknown_output_slot ... ok
test dag::tests::test_valid_edge_references ... ok
test dag::tests::test_valid_graph ... ok
test dag::tests::test_valid_zit_5node_passes ... ok
test ledger::tests::test_free_mib_unknown_device ... ok
test ledger::tests::test_update ... ok
test ledger::tests::test_would_fit_false ... ok
test nodes::tests::test_all_nine_types_present ... ok
test queue::tests::test_cancel_skipped_on_pop ... ok
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok
test queue::tests::test_enqueue_pop_order ... ok
test ledger::tests::test_init_from ... ok
test ledger::tests::test_would_fit_true ... ok
test scheduler::tests::test_select_auto_all_busy ... ok
test scheduler::tests::test_select_auto_single_idle ... ok
test scheduler::tests::test_select_auto_tie_break_device_index ... ok
test scheduler::tests::test_select_preference_busy ... ok
test scheduler::tests::test_select_auto_ranked_by_free_mib ... ok
test scheduler::tests::test_select_cpu_not_available ... ok
test scheduler::tests::test_select_preference_idle ... ok
test job_store::tests::test_insert_and_get ... ok
test job_store::tests::test_list_jobs_before_cursor ... ok
test job_store::tests::test_list_jobs_all ... ok
test job_store::tests::test_update_status ... ok
test job_store::tests::test_list_jobs_status_filter ... ok
test job_store::tests::test_list_jobs_limit ... ok
test scheduler::tests::test_submit_persists_settings ... ok
test scheduler::tests::test_cancel_queued ... ok
test scheduler::tests::test_select_cpu ... ok
test scheduler::tests::test_select_preference_not_found ... ok
test scheduler::tests::test_submit_invalid_graph ... ok
test scheduler::tests::test_submit_broadcasts_event ... ok
test scheduler::tests::test_submit_valid_job ... ok
test scheduler::tests::test_dispatch_sends_execute ... ok
test scheduler::tests::test_cancel_running ... ok
test scheduler::tests::test_cancel_broadcasts_event ... ok
test scheduler::tests::test_image_ready_broadcasts_event ... ok
test scheduler::tests::test_progress_broadcasts_event ... ok
test scheduler::tests::test_complete ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-3be553a117d0d534)

running 45 tests
test frontend::tests::test_frontend_headless ... ok
test frontend::tests::test_frontend_local_missing_path ... ok
test frontend::tests::test_frontend_remote ... ok
test frontend::tests::test_frontend_local_serves_fixture ... ok
test artifact::store::tests::delete_for_job_empty_returns_zero ... ok
test artifact::store::tests::list_empty_returns_empty_array ... ok
test artifact::store::tests::list_with_job_id_filter ... ok
test artifact::store::tests::list_before_filter ... ok
test handlers::artifacts::tests::list_artifacts_empty_returns_200_with_empty_array ... ok
test artifact::store::tests::list_limit_clamped ... ok
test handlers::jobs::tests::cancel_job_returns_404_when_missing ... ok
test handlers::jobs::tests::cancel_job_returns_202_for_queued_job ... ok
test handlers::artifacts::tests::list_artifacts_with_job_id_filter ... ok
test handlers::jobs::tests::cancel_job_returns_409_for_completed_job ... ok
test handlers::jobs::tests::delete_job_returns_204_for_completed_job ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test artifact::store::tests::delete_for_job_removes_files_and_rows ... ok
test handlers::jobs::tests::clear_jobs_defaults_to_all ... ok
test handlers::jobs::tests::clear_jobs_rejects_invalid_status ... ok
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test handlers::jobs::tests::get_job_returns_404_when_missing ... ok
test handlers::jobs::tests::delete_job_returns_404_when_missing ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
test handlers::jobs::tests::clear_jobs_skips_running_jobs ... ok
test handlers::jobs::tests::clear_jobs_returns_200_for_completed_jobs ... ok
test handlers::jobs::tests::get_job_returns_200_with_queued_job ... ok
test handlers::jobs::tests::delete_job_returns_409_for_queued_job ... ok
test handlers::jobs::tests::list_jobs_limit_clamps_to_one ... ok
test handlers::jobs::tests::list_jobs_returns_all_submitted_jobs ... ok
test handlers::jobs::tests::clear_jobs_removes_artifacts ... ok
test tests::health_returns_200 ... ok
test handlers::jobs::tests::list_jobs_filters_by_status ... ok
test tests::rescan_returns_202 ... ok
test tests::restart_worker_returns_404_for_unknown_worker ... ok
test tests::restart_worker_returns_503_when_no_workers ... ok
test tests::workers_endpoint_returns_200 ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::patch_model_returns_404 ... ok
test tests::patch_model_updates_dtype_hint ... ok
test tests::patch_model_partial_preserves_other_fields ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok
test tests::restart_worker_returns_202_for_existing_worker ... ok

test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.27s

     Running tests/api_artifact_save.rs (target/debug/deps/api_artifact_save-959c4742cbbb5534)

running 1 test
test artifact_save ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/api_artifact_serve.rs (target/debug/deps/api_artifact_serve-7e65eb2749087d4a)

running 3 tests
test artifact_serve_404_when_missing ... ok
test artifact_serve_returns_correct_bytes ... ok
test artifact_serve_200_with_headers ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/api_models.rs (target/debug/deps/api_models-8c8d47c70c560974)

running 3 tests
test list_models_returns_scanned_models ... ok
test list_models_kind_filter_no_match ... ok
test list_models_kind_filter_diffusion ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-4f40b9120744db05)

running 1 test
test ws_connect_broadcast_receive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-27f8f16d63c4c2cf)

running 19 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_ipc_socket_path ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::restart_exits_0_and_returns_to_idle ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::handshake_completes_once ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::spawn_ping_pong ... ok
test managed::tests::status_transitions ... ok
test managed::tests::spawn_reaches_idle ... ok
test pool::tests::shutdown_all_stops_all ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.07s

     Running unittests src/main.rs (target/debug/deps/anvilml-2d19a32f612190ac)

running 17 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok
test preflight::tests::is_python_3_12_false ... ok
test preflight::tests::is_python_3_12_true ... ok
test preflight::tests::parse_version_3_11 ... ok
test preflight::tests::parse_version_empty_fails ... ok
test preflight::tests::parse_version_no_python_prefix ... ok
test preflight::tests::parse_version_python_3_12_4 ... ok
test preflight::tests::parse_version_with_suffix ... ok
test preflight::tests::resolve_interpreter_unix ... ok
test preflight::tests::resolve_interpreter_windows ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/api_cancel.rs (target/debug/deps/api_cancel-96bd40cde4146dd7)

running 2 tests
test cancel_running_job_returns_202_and_ws_cancelled ... ok
test cancel_terminal_job_returns_409 ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s

     Running tests/api_delete.rs (target/debug/deps/api_delete-9278d079b190954e)

running 5 tests
test delete_completed_job_removes_artifact_and_row ... ok
test bulk_delete_all_terminal_jobs ... ok
test delete_nonexistent_job_returns_404 ... ok
test bulk_delete_by_status_removes_only_matching ... ok
test delete_running_job_returns_409 ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s

     Running tests/api_ws_lifecycle.rs (target/debug/deps/api_ws_lifecycle-10d97c28b059b1d8)

running 1 test
test test_ws_lifecycle_full_job ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.63s

     Running tests/config_reference.rs (target/debug/deps/config_reference-dc2f2eb70b97d6b6)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/preflight_check.rs (target/debug/deps/preflight_check-eceaa0dd290fd64f)

running 4 tests
test env_returns_correct_shape_in_stub_context ... ok
test env_endpoint_reflects_failed_preflight ... ok
test job_submit_rejected_when_preflight_fails ... ok
test job_submit_proceeds_in_mock_mode ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests anvilml_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_hardware

running 2 tests
test crates/anvilml-hardware/src/sysfs.rs - sysfs::read_vram_from_amdgpu_sysfs (line 89) ... ok
test crates/anvilml-hardware/src/sysfs.rs - sysfs::parse_pci_id (line 65) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s

   Doc-tests anvilml_ipc

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_registry

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_scheduler

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_worker

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Total: 263 tests passed, 0 failed across all crates.

## Format Gate

```
(No output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.37s
    Status: exit 0

# 2. Mock-hardware Windows cross-check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.74s
    Status: exit 0

# 3. Real-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.02s
    Status: exit 0

# 4. Real-hardware Windows cross-check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.41s
    Status: exit 0
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 36.56s
     Running tests/config_reference.rs (target/debug/deps/config_reference-dc2f2eb70b97d6b6)
    running 1 test
    test test_toml_key_set_matches_default ... ok
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
    Status: exit 0
```

### Gate 2 — OpenAPI Drift
```
Generated OpenAPI spec: /home/dryw/AnvilML/backend/openapi.json
diff --git a/backend/openapi.json b/backend/openapi.json
index 0831791..8e60640 100644
--- a/backend/openapi.json
+++ b/backend/openapi.json
@@ -676,7 +676,7 @@
         "enum": [
           "f32",
           "f16",
-          "b_f16",
+          "bf16",
           "f8_e4m3",
           "f8_e5m2",
           "q8",
```
Regenerated and staged. Re-run confirmed idempotent (exit 0, no diff).

## Deviations from Plan

- None. All three plan steps were implemented exactly as specified. The OpenAPI drift gate (Gate 2) was additionally run and passed by regenerating `backend/openapi.json` — this was required because the trigger condition was met (DType is a ToSchema-derived type with a serde rename change), even though plan step 4 only explicitly listed `cargo test -p anvilml-core`.

## Blockers

None.
