# Implementation Report: P6-A1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P6-A1                                             |
| Phase         | 006 — Model Registry & Artifacts                  |
| Description   | database/: migrations dir + 001_initial.sql (models, device_capabilities) |
| Implemented   | 2026-06-29T14:10:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created the `database/migrations/` directory and the first SQL migration file
`database/migrations/001_initial.sql`. The migration defines two tables — `models`
(mapping from `ModelMeta` in `anvilml-core`) and `device_capabilities` (PCI-ID
capability hints from `InferenceCaps`) — plus a unique index on the composite
PCI-ID key. All acceptance criteria passed: the SQL executes cleanly in
`sqlite3 :memory:` and both tables are created.

## Resolved Dependencies

None. This task writes a single SQL migration file with no Rust dependencies,
no Cargo.toml changes, and no Python packages.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `database/migrations/001_initial.sql` | First migration: `models` and `device_capabilities` tables plus unique index |

## Commit Log

```
 .forge/reports/P6-A1_plan.md              | 147 ++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 ++--
 database/migrations/001_initial.sql       |  46 +++++++++++
 4 files changed, 203 insertions(+), 9 deletions(-)
```

## Test Results

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 20.43s
     Running unittests src/lib.rs (target/debug/deps/anvilml-54734929787501cf)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/anvilml-ee2a7f2c9dadf82e)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cli_help_test.rs (target/debug/deps/cli_help_test-489c4e2c9433b437)
running 1 test
test tests::cli_help_shows_all_flags ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/config_reference.rs (target/debug/deps/config_reference-6e185552ff7a87b9)
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/hw_probe_help_test.rs (target/debug/deps/hw_probe_help_test-a85195637752a59c)
running 1 test
test tests::hw_probe_help_shows_subcommand ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/shutdown_tests.rs (target/debug/deps/shutdown_tests-1d94135a94578645)
running 2 tests
test tests::test_shutdown_signal_timeout_cancels ... ok
test tests::test_shutdown_signal_returns_on_ctrl_c ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s

     Running unittests src/lib.rs (target/debug/deps/anvilml_artifacts-5082677c2b98637f)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_core-e2a86bb58143f8e1)
running 1 test
test config_load::tests::test_load_none_path_missing_file_returns_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/artifact_tests.rs (target/debug/deps/artifact_tests-13da59de547eeee9)
running 3 tests
test test_artifact_meta_field_names ... ok
test test_artifact_meta_hash_format ... ok
test test_artifact_meta_serde_roundtrip ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/config_load_tests.rs (target/debug/deps/config_load_tests-15ff35b2a2050c58)
running 13 tests
test test_cli_override_beats_env_var ... ok
test test_env_var_overrides_default_no_toml ... ok
test test_env_var_port_override ... ok
test test_env_var_overrides_toml_value ... ok
test test_load_default_path_resolves_anvilml_toml ... ok
test test_load_full_toml_roundtrips_all_fields ... ok
test test_load_malformed_toml_returns_err ... ok
test test_load_missing_file_falls_back_to_defaults ... ok
test test_load_partial_toml_overrides_only_specified_fields ... ok
test test_nested_env_var_gpu_selection ... ok
test test_load_nested_struct_partial_override ... ok
test test_num_threads_env_var ... ok
test test_unset_env_vars_leave_prior_layer_value ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/config_tests.rs (target/debug/deps/config_tests-e7cdbad72e0262fb)
running 13 tests
test test_artifact_dir_default ... ok
test test_db_path_default ... ok
test test_gpu_selection_default ... ok
test test_hardware_override_default ... ok
test test_host_default ... ok
test test_limits_default ... ok
test test_max_ipc_payload_mib_default ... ok
test test_model_scan_depth_default ... ok
test test_model_dirs_default ... ok
test test_port_default ... ok
test test_num_threads_default ... ok
test test_rocm_default ... ok
test test_venv_path_default ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/error_tests.rs (target/debug/deps/error_tests-f91575277c9460ea)
running 16 tests
test test_artifact_not_found_returns_404 ... ok
test test_error_body_message_contains_variant_info ... ok
test test_error_body_has_request_id ... ok
test test_db_returns_500 ... ok
test test_cycle_detected_returns_400 ... ok
test test_error_field_is_snake_case ... ok
test test_invalid_graph_returns_400 ... ok
test test_internal_returns_500 ... ok
test test_io_returns_500 ... ok
test test_ipc_returns_400 ... ok
test test_job_not_found_returns_404 ... ok
test test_model_not_found_returns_404 ... ok
test test_payload_too_large_returns_413 ... ok
test test_serde_returns_400 ... ok
test test_workers_unavailable_returns_503 ... ok
test test_worker_not_found_returns_404 ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/events_tests.rs (target/debug/deps/events_tests-fbee2f6044539869)
running 10 tests
test test_ws_event_job_cancelled_serde_roundtrip ... ok
test test_ws_event_job_completed_serde_roundtrip ... ok
test test_ws_event_job_failed_serde_roundtrip ... ok
test test_ws_event_job_image_ready_serde_roundtrip ... ok
test test_ws_event_job_progress_serde_roundtrip ... ok
test test_ws_event_job_queued_serde_roundtrip ... ok
test test_ws_event_job_started_serde_roundtrip ... ok
test test_ws_event_provisioning_progress_serde_roundtrip ... ok
test test_ws_event_system_stats_serde_roundtrip ... ok
test test_ws_event_worker_status_changed_serde_roundtrip ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/hardware_tests.rs (target/debug/deps/hardware_tests-c855f2a25ce59a37)
running 9 tests
test test_capability_source_serde_snake_case ... ok
test test_device_type_serde_snake_case ... ok
test test_enumeration_source_serde_snake_case ... ok
test test_enumeration_source_copy_trait ... ok
test test_gpu_device_construction_and_serde ... ok
test test_host_info_serde_roundtrip ... ok
test test_inference_caps_default_roundtrip ... ok
test test_hardware_info_serde_roundtrip ... ok
test test_inference_caps_non_default_roundtrip ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/job_tests.rs (target/debug/deps/job_tests-12620cdeda9796b6)
running 4 tests
test test_job_settings_default ... ok
test test_job_serde_roundtrip ... ok
test test_job_status_all_variants_roundtrip ... ok
test test_job_with_nulls_roundtrip ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/model_tests.rs (target/debug/deps/model_tests-ecbd8df176baa60d)
running 4 tests
test test_model_format_serde_snake_case ... ok
test test_model_kind_serde_snake_case ... ok
test test_model_dtype_serde_snake_case ... ok
test test_model_meta_serde_roundtrip ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/node_registry_tests.rs (target/debug/deps/node_registry_tests-3dcb50030bdbfc32)
running 5 tests
test test_empty_registry_returns_none ... ok
test test_list_returns_all ... ok
test test_register_all_populates ... ok
test test_register_all_replaces_prior_contents ... ok
test test_concurrent_get_during_register_all_does_not_deadlock ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/node_tests.rs (target/debug/deps/node_tests-52b042a75b8864cd)
running 4 tests
test test_node_type_descriptor_empty_slots ... ok
test test_node_type_descriptor_construction ... ok
test test_slot_type_screaming_snake_case_serde ... ok
test test_slot_descriptor_serde_roundtrip ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/worker_tests.rs (target/debug/deps/worker_tests-51e8ea26660d0580)
running 4 tests
test test_env_report_serde_roundtrip ... ok
test test_provisioning_state_serde_snake_case ... ok
test test_worker_status_serde_snake_case ... ok
test test_worker_info_construction_and_serde_roundtrip ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-232f890ff9d91c0c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cpu_tests.rs (target/debug/deps/cpu_tests-f42f97abfe36dea0)
running 6 tests
test test_cpu_detect_never_errors ... ok
test test_cpu_detector_all_device_fields ... ok
test test_cpu_detector_device_type_is_cpu ... ok
test test_cpu_detector_enumeration_source_is_cpu ... ok
test test_cpu_detector_refresh_vram_returns_zero ... ok
test test_cpu_detector_returns_one_device ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/detect_tests.rs (target/debug/deps/detect_tests-7dbacbc4d70c282e)
running 14 tests
test test_inference_caps_is_caps_union ... ok
test test_host_fields_non_empty ... ok
test test_cpu_device_always_present_and_last ... ok
test test_mock_detector_env_vars_propagate_through_detect_all_devices ... ok
test test_mock_hardware_feature_returns_mock_device ... ok
test test_inference_caps_union_correctness ... ok
test test_override_absent_returns_hardware_info ... ok
test test_override_cpu_device_type ... ok
test test_override_inference_caps_is_default ... ok
test test_override_path_still_has_cpu_device ... ok
test test_override_present_returns_device ... ok
test test_override_rocm_device_type ... ok
test test_override_unrecognized_device_type_defaults_to_cpu ... ok
test test_override_takes_priority_over_mock ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/dxgi_tests.rs (target/debug/deps/dxgi_tests-9ac153dd49ad7a7c)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/mock_tests.rs (target/debug/deps/mock_tests-48cc212875aa942c)
running 6 tests
test test_mock_cuda_device_type ... ok
test test_mock_detector_defaults ... ok
test test_mock_device_name_override ... ok
test test_mock_refresh_vram ... ok
test test_mock_rocm_device_type ... ok
test test_mock_vram_override ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/sysfs_tests.rs (target/debug/deps/sysfs_tests-34744aa674d9745a)
running 7 tests
test test_sysfs_detect_missing_path_returns_empty ... ok
test test_sysfs_detect_never_errors ... ok
test test_sysfs_refresh_vram_returns_zero ... ok
test test_sysfs_detect_nvidia_vendor ... ok
test test_sysfs_detect_synthetic_display_device ... ok
test test_sysfs_filter_non_display_class ... ok
test test_sysfs_multi_device_filter ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/vulkan_tests.rs (target/debug/deps/vulkan_tests-6fff7b72e47c9c16)
running 8 tests
test test_vulkan_amd_vendor_maps_to_rocm ... ok
test test_vulkan_intel_vendor_skipped ... ok
test test_vulkan_nvidia_vendor_maps_to_cuda ... ok
test test_vulkan_unknown_vendor_skipped ... ok
test test_vulkan_detect_never_errors ... ok
test test_vulkan_refresh_vram_out_of_range ... ok
test test_vulkan_detect_returns_empty_when_no_gpu ... ok
test test_vulkan_refresh_vram_never_errors ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-fe3a343aa5f8068f)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-15a7113ab4ed4040)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-2aed7e9e9b8b1d48)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-f3a8352e7deaa764)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-0ae154da9d6ab85b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/health_tests.rs (target/debug/deps/health_tests-14cef9ef48d3620b)
running 1 test
test test_health_returns_200 ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-1cf8536802706829)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_artifacts
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_core
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests anvilml_hardware
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

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

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.41s

# 2. Mock-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.76s

# 3. Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.09s

# 4. Real-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.02s
```

## Project Gates

None defined for this task. Gate 1 (Config Surface Sync) and Gate 2 (OpenAPI Drift)
are only triggered when specific source files are modified; this task creates only
a SQL migration file.

## Public API Delta

No new pub items introduced. This task creates a SQL migration file only — no Rust
types, functions, traits, or re-exports.

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
