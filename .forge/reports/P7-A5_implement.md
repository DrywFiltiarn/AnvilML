# Implementation Report: P7-A5

| Field       | Value                                                      |
|-------------|------------------------------------------------------------|
| Task ID     | P7-A5                                                      |
| Phase       | 007 — WebSocket Event Stream                               |
| Description | anvilml: start stats tick at startup; verify live WS stream |
| Implemented | 2026-06-04T21:20:00Z                                       |
| Status      | COMPLETE                                                   |

## Summary

Wired the `spawn_system_stats_tick` function (implemented in P7-A4) into `backend/src/main.rs` so that the system stats broadcast task starts immediately after `AppState` is constructed. Added one import line and one spawn call — no new dependencies, no test changes, no config modifications. All build, format, lint, cross-check, test, and gate checks pass.

## Resolved Dependencies

No new dependencies added or modified for this task. The `spawn_system_stats_tick` function and its transitive dependencies (`tokio`, `chrono`, `sysinfo`) were already present from the P7-A4 task.

## Files Changed

| Action | Path                        | Description                                                         |
|--------|-----------------------------|---------------------------------------------------------------------|
| Edit   | `backend/src/main.rs`       | Added import for `spawn_system_stats_tick` and spawn call after AppState construction |

## Commit Log

```
 backend/src/main.rs          |  2 ++
 .forge/reports/P7-A5_plan.md | 76 ++++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md |  6 ++--
 .forge/state/state.json      | 13 ++++----
 4 files changed, 88 insertions(+), 9 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-76fc372595dda5e4)

running 74 tests
test config::tests::test_model_kind_default ... ok
test config::tests::test_device_type_default ... ok
test error::tests::from_io_error ... ok
test error::tests::error_trait_impls ... ok
test config::tests::test_default_server_config ... ok
test error::tests::debug_formatting ... ok
test error::tests::send_sync ... ok
test error::tests::all_variants_display ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
test types::artifact::tests::artifact_meta_defaults ... ok
test types::artifact::tests::artifact_meta_optional_uuid_nil ... ok
test types::artifact::tests::artifact_meta_json_preserves_fields ... ok
test types::events::tests::job_failed_no_traceback ... ok
test types::events::tests::job_completed_roundtrip ... ok
test types::events::tests::job_failed_roundtrip ... ok
test types::artifact::tests::artifact_meta_roundtrip ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test types::events::tests::job_cancelled_roundtrip ... ok
test types::events::tests::job_progress_optional_fields ... ok
test types::events::tests::job_image_ready_roundtrip ... ok
test config_load::tests::override_beats_env ... ok
test config_load::tests::env_nested_field ... ok
test config_load::tests::missing_toml_fallback ... ok
test types::events::tests::job_progress_roundtrip ... ok
test config_load::tests::env_overrides_toml ... ok
test types::events::tests::ws_event_enum_variants ... ok
test types::events::tests::worker_status_changed_roundtrip ... ok
test types::hardware::tests::capability_source_variants ... ok
test types::hardware::tests::capability_source_default_is_fallback ... ok
test types::hardware::tests::device_type_json_strings ... ok
test types::hardware::tests::device_type_variants ... ok
test types::hardware::tests::enumeration_capability_sources_roundtrip ... ok
test types::hardware::tests::enumeration_source_default_is_fallback ... ok
test types::hardware::tests::enumeration_source_variants ... ok
test types::hardware::tests::gpu_device_backward_compat ... ok
test types::hardware::tests::gpu_device_roundtrip ... ok
test types::hardware::tests::hardware_info_empty_gpus ... ok
test types::hardware::tests::inference_caps_defaults ... ok
test types::hardware::tests::hardware_info_roundtrip ... ok
test types::hardware::tests::host_info_roundtrip ... ok
test types::hardware::tests::inference_caps_roundtrip ... ok
test types::job::tests::job_optional_numeric_fields_default ... ok
test types::job::tests::job_graph_json_value ... ok
test types::job::tests::job_optional_string_fields_default_none ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_roundtrip ... ok
test types::job::tests::job_settings_roundtrip ... ok
test types::job::tests::job_status_variants ... ok
test types::job::tests::job_timestamps_iso8601 ... ok
test types::job::tests::submit_job_request_roundtrip ... ok
test types::model::tests::dtype_default_is_unknown ... ok
test types::model::tests::dtype_variants ... ok
test types::model::tests::dtype_roundtrip_json ... ok
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

test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-5fdd1cf1d98e831a)

running 59 tests
test cpu::tests::cpu_refresh_vram ... ok
test device_db::tests::arch_format_validation ... ok
test device_db::tests::field_count_no_vram ... ok
test device_db::tests::miss_returns_none ... ok
test device_db::tests::no_duplicate_pci_ids ... ok
test device_db::tests::seed_entries_lookup ... ok
test device_db::tests::boolean_flag_consistency ... ok
test cpu::tests::cpu_device_fields ... ok
test device_db::tests::seed_entry_integrity ... ok
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_new_fields ... ok
test mock::tests::mock_detect_rocm ... ok
test mock::tests::mock_detect_cuda ... ok
test mock::tests::mock_detect_default_cpu ... ok
test nvml::tests::nvml_all_devices_are_cuda ... ok
test mock::tests::mock_device_new_fields ... ok
test nvml::tests::nvml_detect_returns_ok ... ok
test nvml::tests::nvml_init_fallback_no_library ... ok
test nvml::tests::nvml_library_load_fails_gracefully ... ok
test sysfs::tests::parse_pci_ids_valid_hex ... ok
test nvml::tests::nvml_shutdown_in_drop_no_panic ... ok
test sysfs::tests::read_vram_helper_converts_bytes_to_mib ... ok
test sysfs::tests::sysfs_detect_returns_ok_on_absent_dir ... ok
test tests::or_all_caps_empty ... ok
test tests::or_all_caps_merges ... ok
test sysfs::tests::sysfs_detect_with_fixture_data ... ok
test sysfs::tests::vendor_id_maps_cpu_intel ... ok
test sysfs::tests::vendor_id_maps_cpu_unknown ... ok
test sysfs::tests::vendor_id_maps_cuda ... ok
test sysfs::tests::vendor_id_maps_rocm ... ok
test tests::devices_have_sequential_indices ... ok
test tests::detect_all_devices_override_source ... ok
test tests::detect_all_devices_override ... ok
test tests::detect_all_devices_override_rocm ... ok
test tests::detect_all_devices_vulkan_empty ... ok
test tests::detect_all_devices_mock_cuda ... ok
test tests::detect_all_devices_override_cpu ... ok
test tests::override_device_new_fields ... ok
test tests::host_info_populated ... ok
test tests::detect_all_devices_mock_device_type ... ok
test tests::detect_all_devices_never_errs ... ok
test tests::detect_all_devices_mock_enum_source ... ok
test tests::detect_all_devices_mock_rocm ... ok
test tests::detect_all_devices_mock_vram ... ok
test tests::mock_device_new_fields_in_detect_all ... ok
test vulkan::tests::free_vram_fallback_no_budget ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_zero ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok
test vulkan::tests::vulkan_detect_returns_ok ... ok

test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.72s

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-9d39e30982bb9c7f)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-3092c5600816a5d8)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-9ad0f13b6aa8274b)

running 10 tests
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_vram_estimate_mib ... ok
test scanner::tests::test_sha256_hex ... ok
test db::tests::test_open_creates_tables ... ok
test db::tests::test_reset_ghost_jobs ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-b78b37bb4a06ecfc)

running 1 test
test test_open_creates_tables ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/rescan.rs (target/debug/deps/rescan-1f68aa81b5f32f7d)

running 2 tests
test test_rescan_adds_models ... ok
test test_rescan_idempotent ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/scanner.rs (target/debug/deps/scanner-ef7a597e59fb4c90)

running 1 test
test test_scan_dirs_two_files ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/store_get.rs (target/debug/deps/store_get-67ec6059b34bbce2e10bab52)

running 2 tests
test test_get_missing_returns_none ... ok
test test_upsert_then_get_returns_equal_meta ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/store_list.rs (target/debug/deps/store_list-b09ee53cf064464)

running 3 tests
test test_list_empty_returns_empty_vec ... ok
test test_list_after_upserts_returns_ordered ... ok
test test_list_kind_filter ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-9ca1d3256f774631)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-d9f7c9ca32a9308c)

running 8 tests
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::health_returns_200 ... ok
test tests::rescan_returns_202 ... ok
test tests::system_returns_200_with_hardware_info ... ok
test tests::get_model_returns_404_when_missing ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.01s

     Running tests/api_models.rs (target/debug/deps/api_models-5a0fe6e4712d0c9f)

running 3 tests
test list_models_kind_filter_diffusion ... ok
test list_models_returns_scanned_models ... ok
test list_models_kind_filter_no_match ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-c34bbce2e10bab52)

running 1 test
test ws_connect_broadcast_receive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-72ee4379635c8b26)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/anvilml-dc988a6cc3e67e82)

running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/config_reference.rs (target/debug/deps/config_reference-e98d42e4064540cd)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests anvilml_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_hardware

running 2 tests
test crates/anvilml-hardware/src/sysfs.rs - sysfs::read_vram_from_amdgpu_sysfs (line 89) ... ok
test crates/anvilml-hardware/src/sysfs.rs - sysfs::parse_pci_id (line 65) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s

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

Total: 154 tests passed, 0 failed, 0 ignored.

## Platform Cross-Check

### Check 1: Mock-hardware Windows-gnu cross-check
```
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.12s
```

### Check 2: Real-hardware Linux native
```
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.92s
```

### Check 3: Real-hardware Windows-gnu cross-check
```
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.11s
```

All three checks exit 0. Zero errors.

## Project Gates

### Config Surface Sync (config_reference)
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-b5e7d85be9b94dc4)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Gate passes. Zero failures.

## Deviations from Plan

None. Implementation follows the approved plan exactly:
- Added `use anvilml_server::ws::stats_tick::spawn_system_stats_tick;` import at line 7 of `backend/src/main.rs`
- Inserted `spawn_system_stats_tick(state.clone());` after AppState construction (line 170) and before `build_router(state)` (line 171)
- No scope creep, no additional files modified

## Blockers

None. All checks pass, no MCP servers needed (no new dependencies), no pre-existing issues encountered.
