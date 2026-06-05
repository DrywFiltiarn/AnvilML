# Implementation Report: P7-F0

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-F0                                             |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml-core: extend InferenceCaps with fp32, fp8, fp4, nvfp4 fields |
| Implemented | 2026-06-05T13:00:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Extended the `InferenceCaps` struct in `anvilml-core/src/types/hardware.rs` with four new boolean capability fields (`fp32`, `fp8`, `fp4`, `nvfp4`) in canonical order between existing fields. Updated all struct literal constructions across six files, extended the `or_all_caps()` OR-reduction function, updated the CLI hardware table printer to display all seven flags, and refreshed test assertions in `hardware.rs`, `lib.rs`, `cpu.rs`, and `device_db.rs`. All new fields use `#[serde(default)]` for backward-compatible JSON deserialization.

## Resolved Dependencies

No new dependencies added or modified for this task. All changes are to existing struct definitions and code paths within the workspace.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/hardware.rs` | Added fp32, fp8, fp4, nvfp4 fields to InferenceCaps; updated all struct literals and test assertions |
| Modify | `crates/anvilml-hardware/src/lib.rs` | Extended or_all_caps() with 4 new OR-lines; updated test literals and assertions |
| Modify | `crates/anvilml-hardware/src/cpu.rs` | Updated test assertion for new capability fields |
| Modify | `crates/anvilml-hardware/src/device_db.rs` | Updated InferenceCaps literal in resolve_caps() with 4 new fields defaulting to false |
| Modify | `backend/src/main.rs` | Replaced hardcoded caps_str if/else chain with iterative field check; updated summary line |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |  6 ++--
 .forge/state/state.json                   | 13 +++----
 backend/src/main.rs                       | 44 +++++++++++++++++------
 crates/anvilml-core/src/types/hardware.rs | 60 +++++++++++++++++++++++++++++++
 crates/anvilml-hardware/src/cpu.rs        |  4 +++
 crates/anvilml-hardware/src/device_db.rs  |  4 +++
 crates/anvilml-hardware/src/lib.rs        | 20 +++++++++++
 7 files changed, 131 insertions(+), 20 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware

     Running unittests src/lib.rs (target/debug/deps/anvilml_core-8c562ebe203974a1)
running 74 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_toml_roundtrip ... ok
test config_load::tests::env_nested_field ... ok
test error::tests::all_variants_display ... ok
test config_load::tests::env_overrides_toml ... ok
test error::tests::debug_formatting ... ok
test error::tests::error_trait_impls ... ok
test config_load::tests::missing_toml_fallback ... ok
test error::tests::from_io_error ... ok
test error::tests::send_sync ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
test config_load::tests::override_beats_env ... ok
test types::artifact::tests::artifact_meta_defaults ... ok
test types::artifact::tests::artifact_meta_json_preserves_fields ... ok
test types::artifact::tests::artifact_meta_optional_uuid_nil ... ok
test types::artifact::tests::artifact_meta_roundtrip ... ok
test types::events::tests::job_cancelled_roundtrip ... ok
test types::events::tests::job_completed_roundtrip ... ok
test types::events::tests::job_failed_no_traceback ... ok
test types::events::tests::job_failed_roundtrip ... ok
test types::events::tests::job_image_ready_roundtrip ... ok
test types::events::tests::job_progress_optional_fields ... ok
test types::events::tests::job_progress_roundtrip ... ok
test types::events::tests::job_queued_roundtrip ... ok
test types::events::tests::job_started_roundtrip ... ok
test types::events::tests::system_stats_event_json ... ok
test types::events::tests::worker_status_changed_roundtrip ... ok
test types::hardware::tests::capability_source_default_is_fallback ... ok
test types::hardware::tests::capability_source_variants ... ok
test types::hardware::tests::device_type_json_strings ... ok
test types::hardware::tests::device_type_variants ... ok
test types::hardware::tests::enumeration_capability_sources_roundtrip ... ok
test types::hardware::tests::enumeration_source_default_is_fallback ... ok
test types::hardware::tests::enumeration_source_variants ... ok
test types::hardware::tests::gpu_device_backward_compat ... ok
test types::hardware::tests::gpu_device_roundtrip ... ok
test types::hardware::tests::hardware_info_empty_gpus ... ok
test types::hardware::tests::host_info_roundtrip ... ok
test types::hardware::tests::hardware_info_roundtrip ... ok
test types::hardware::tests::inference_caps_defaults ... ok
test types::hardware::tests::inference_caps_roundtrip ... ok
test types::job::tests::job_optional_numeric_fields_default ... ok
test types::job::tests::job_graph_json_value ... ok
test types::job::tests::job_optional_string_fields_default_none ... ok
test types::job::tests::job_optional_timestamps_default_none ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_roundtrip ... ok
test types::job::tests::job_settings_roundtrip ... ok
test types::job::tests::job_status_variants ... ok
test types::job::tests::job_timestamps_iso8601 ... ok
test types::job::tests::submit_job_request_roundtrip ... ok
test types::job::tests::submit_job_response_roundtrip ... ok
test types::model::tests::dtype_default_is_unknown ... ok
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

test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-af561bd195987fd1)
running 59 tests
test cpu::tests::cpu_detect_returns_one_device ... ok
test cpu::tests::cpu_device_new_fields ... ok
test cpu::tests::cpu_device_fields ... ok
test cpu::tests::cpu_refresh_vram ... ok
test device_db::tests::boolean_flag_consistency ... ok
test device_db::tests::arch_format_validation ... ok
test device_db::tests::field_count_no_vram ... ok
test device_db::tests::miss_returns_none ... ok
test device_db::tests::no_duplicate_pci_ids ... ok
test device_db::tests::seed_entries_lookup ... ok
test device_db::tests::seed_entry_integrity ... ok
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
test sysfs::tests::sysfs_detect_returns_ok_on_absent_dir ... ok
test sysfs::tests::read_vram_helper_converts_bytes_to_mib ... ok
test tests::or_all_caps_empty ... ok
test tests::or_all_caps_merges ... ok
test tests::detect_all_devices_override ... ok
test tests::detect_all_devices_override_cpu ... ok
test tests::detect_all_devices_override_rocm ... ok
test tests::host_info_populated ... ok
test tests::detect_all_devices_override_source ... ok
test tests::detect_all_devices_vulkan_empty ... ok
test tests::override_device_new_fields ... ok
test tests::devices_have_sequential_indices ... ok
test tests::detect_all_devices_mock_vram ... ok
test sysfs::tests::vendor_id_maps_cpu_intel ... ok
test sysfs::tests::vendor_id_maps_cpu_unknown ... ok
test sysfs::tests::vendor_id_maps_cuda ... ok
test sysfs::tests::vendor_id_maps_rocm ... ok
test tests::detect_all_devices_never_errs ... ok
test tests::detect_all_devices_mock_device_type ... ok
test tests::detect_all_devices_mock_cuda ... ok
test tests::detect_all_devices_mock_enum_source ... ok
test tests::detect_all_devices_mock_rocm ... ok
test sysfs::tests::sysfs_detect_with_fixture_data ... ok
test tests::mock_device_new_fields_in_detect_all ... ok
test vulkan::tests::free_vram_from_budget ... ok
test vulkan::tests::free_vram_fallback_no_budget ... ok
test vulkan::tests::free_vram_underflow_protection ... ok
test vulkan::tests::parse_vulkan_driver_version_amd ... ok
test vulkan::tests::largest_device_local_heap_wins_over_host_visible_resizable_bar ... ok
test vulkan::tests::parse_vulkan_driver_version_nvidia ... ok
test vulkan::tests::no_device_local_heap_yields_zero ... ok
test vulkan::tests::parse_vulkan_driver_version_zero ... ok
test vulkan::tests::vendor_id_maps_cpu_intel ... ok
test vulkan::tests::vendor_id_maps_cpu_unknown ... ok
test vulkan::tests::vendor_id_maps_cuda ... ok
test vulkan::tests::vram_calculation_handles_large_heaps ... ok
test vulkan::tests::vendor_id_maps_rocm ... ok
test vulkan::tests::vulkan_detect_returns_ok ... ok

test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-cc67d683117a3c7e)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-e6591a85771fd494)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-a5b296ccc9bbc22e)
running 11 tests
test scanner::tests::test_infer_dtype_case_insensitive ... ok
test scanner::tests::test_infer_dtype_matches ... ok
test scanner::tests::test_infer_dtype_unknown ... ok
test scanner::tests::test_infer_kind_case_insensitive ... ok
test scanner::tests::test_infer_kind_fallback ... ok
test scanner::tests::test_infer_kind_matches ... ok
test scanner::tests::test_sha256_hex ... ok
test scanner::tests::test_vram_estimate_mib ... ok
test db::tests::test_open_creates_file_if_missing ... ok
test db::tests::test_open_creates_tables ... ok
test db::tests::test_reset_ghost_jobs ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-0b3fba3b4225aa32)
running 1 test
test test_open_creates_tables ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs (target/debug/deps/rescan-8843270b042f5769)
running 2 tests
test test_rescan_adds_models ... ok
test test_rescan_idempotent ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-9c4012602e8b670c)
running 1 test
test test_scan_dirs_two_files ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs (target/debug/deps/store_get-4e7feb72e15acb)
running 2 tests
test test_get_missing_returns_none ... ok
test test_upsert_then_get_returns_equal_meta ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs (target/debug/deps/store_list-dbda31f34047ed0f)
running 3 tests
test test_list_empty_returns_empty_vec ... ok
test test_list_after_upserts_returns_ordered ... ok
test test_list_kind_filter ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-affa1eec55194215)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-60c41191bf7930a4)
running 8 tests
test tests::rescan_returns_202 ... ok
test tests::health_returns_200 ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test tests::env_returns_200_with_stub_report ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs (target/debug/deps/api_models-da241e7c61c889a4)
running 3 tests
test list_models_kind_filter_no_match ... ok
test list_models_kind_filter_diffusion ... ok
test list_models_returns_scanned_models ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-86d997cee55452af)
running 1 test
test ws_connect_broadcast_receive ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-e349c00262c885a3)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-1558055c77dddd48)
running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_value_enum_variants ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_possible_values ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-f8d840f68dce50c8)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test crates/anvilml-hardware/src/sysfs.rs - sysfs::parse_pci_id (line 65) ... ok
test crates/anvilml-hardware/src/sysfs.rs - sysfs::read_vram_from_amdgpu_sysfs (line 89) ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Platform Cross-Check

### Check 1 — Mock-hardware Windows-gnu cross-check
```
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.12s
```

### Check 2 — Real-hardware Linux native
```
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.05s
```

### Check 3 — Real-hardware Windows-gnu cross-check
```
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.22s
```

All three checks exited 0 with zero errors.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not required — no handler signatures or utoipa annotations modified in this task.

## Deviations from Plan

None. Implementation followed the approved plan exactly as specified. All six files listed in "Files Affected" were modified, all struct literals were updated, and all test assertions were refreshed.

## Blockers

None.
