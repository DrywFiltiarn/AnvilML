# Implementation Report: P900-A4

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P900-A4                                         |
| Phase         | 900 — Spec-Drift & Logging Retrofit             |
| Description   | anvilml-core: add missing ToSchema to ModelMeta/ModelKind/ModelDtype/ModelFormat |
| Implemented   | 2026-06-30T14:35:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Added the `utoipa::ToSchema` derive to all four model-related types (`ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat`) in `crates/anvilml-core/src/types/model.rs`, completing the OpenAPI schema coverage for the model metadata types. The `ModelMeta::path` field required `#[schema(value_type = String)]` because `PathBuf` has no `ToSchema` impl in utoipa 5.5.0 — this follows the identical pattern already established in `artifact.rs` for `ArtifactMeta::file_path`. All existing serde roundtrip tests pass, documentation builds cleanly, and no new public API items were introduced.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | utoipa  | 5.5.0            | rust-docs MCP  |

The `utoipa` dependency was already declared in `anvilml-core/Cargo.toml` with features `["uuid", "chrono"]`. The `ToSchema` trait exists on utoipa v5.5.0 and is provided by the `macros` default feature. No new dependency or feature flag was introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-core/src/types/model.rs | Added `use utoipa::ToSchema;` import; appended `ToSchema` to derive list on ModelMeta, ModelKind, ModelDtype, ModelFormat; added `#[schema(value_type = String)]` to ModelMeta::path for PathBuf compatibility |
| Modify | crates/anvilml-core/Cargo.toml | Bumped patch version 0.1.18 → 0.1.19 |

## Commit Log

```
 .forge/reports/P900-A4_plan.md         | 143 +++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md           |   6 +-
 .forge/state/state.json                |  13 +--
 Cargo.lock                             |   2 +-
 crates/anvilml-core/Cargo.toml         |   2 +-
 crates/anvilml-core/src/types/model.rs |  10 ++-
 6 files changed, 161 insertions(+), 15 deletions(-)
```

## Test Results

```
     Running tests/model_tests.rs (target/debug/deps/model_tests-e3dcc5e7cf033ea9)

running 4 tests
test test_model_dtype_serde_snake_case ... ok
test test_model_kind_serde_snake_case ... ok
test test_model_format_serde_snake_case ... ok
test test_model_meta_serde_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/artifact_tests.rs (target/debug/deps/artifact_tests-d2d64d34330f0409)

running 3 tests
test test_artifact_meta_field_names ... ok
test test_artifact_meta_hash_format ... ok
test test_artifact_meta_serde_roundtrip ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/job_tests.rs (target/debug/deps/job_tests-42e3584755a61ecd)

running 4 tests
test test_job_settings_default ... ok
test test_job_serde_roundtrip ... ok
test test_job_status_all_variants_roundtrip ... ok
test test_job_with_nulls_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/config_tests.rs (target/debug/deps/config_tests-ed90d56f95baa402)

running 13 tests
test test_artifact_dir_default ... ok
test test_gpu_selection_default ... ok
test test_db_path_default ... ok
test test_hardware_override_default ... ok
test test_host_default ... ok
test test_limits_default ... ok
test test_max_ipc_payload_mib_default ... ok
test test_model_scan_depth_default ... ok
test test_model_dirs_default ... ok
test test_num_threads_default ... ok
test test_port_default ... ok
test test_rocm_default ... ok
test test_venv_path_default ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/config_load_tests.rs (target/debug/deps/config_load_tests-958e22c19f4d1a35)

running 13 tests
test test_cli_override_beats_env_var ... ok
test test_env_var_overrides_default_no_toml ... ok
test test_env_var_overrides_toml_value ... ok
test test_env_var_port_override ... ok
test test_load_default_path_resolves_anvilml_toml ... ok
test test_load_full_toml_roundtrips_all_fields ... ok
test test_load_malformed_toml_returns_err ... ok
test test_load_missing_file_falls_back_to_defaults ... ok
test test_load_partial_toml_overrides_only_specified_fields ... ok
test test_load_nested_struct_partial_override ... ok
test test_nested_env_var_gpu_selection ... ok
test test_num_threads_env_var ... ok
test test_unset_env_vars_leave_prior_layer_value ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/error_tests.rs (target/debug/deps/error_tests-9675d6f2fe5be8fb)

running 16 tests
test test_error_body_has_request_id ... ok
test test_db_returns_500 ... ok
test test_artifact_not_found_returns_404 ... ok
test test_error_body_message_contains_variant_info ... ok
test test_cycle_detected_returns_400 ... ok
test test_internal_returns_500 ... ok
test test_invalid_graph_returns_400 ... ok
test test_error_field_is_snake_case ... ok
test test_io_returns_500 ... ok
test test_ipc_returns_400 ... ok
test test_job_not_found_returns_404 ... ok
test test_payload_too_large_returns_413 ... ok
test test_model_not_found_returns_404 ... ok
test test_serde_returns_400 ... ok
test test_worker_not_found_returns_404 ... ok
test test_workers_unavailable_returns_503 ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/events_tests.rs (target/debug/deps/events_tests-ae307a1aaf307487)

running 10 tests
test test_ws_event_job_cancelled_serde_roundtrip ... ok
test test_ws_event_job_completed_serde_roundtrip ... ok
test test_ws_event_job_failed_serde_roundtrip ... ok
test test_ws_event_job_progress_serde_roundtrip ... ok
test test_ws_event_job_image_ready_serde_roundtrip ... ok
test test_ws_event_job_started_serde_roundtrip ... ok
test test_ws_event_provisioning_progress_serde_roundtrip ... ok
test test_ws_event_job_queued_serde_roundtrip ... ok
test test_ws_event_worker_status_changed_serde_roundtrip ... ok
test test_ws_event_system_stats_serde_roundtrip ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/hardware_tests.rs (target/debug/deps/hardware_tests-2af6fdd58731fa10)

running 9 tests
test test_capability_source_serde_snake_case ... ok
test test_device_type_serde_snake_case ... ok
test test_enumeration_source_copy_trait ... ok
test test_enumeration_source_serde_snake_case ... ok
test test_gpu_device_construction_and_serde ... ok
test test_host_info_serde_roundtrip ... ok
test test_inference_caps_default_roundtrip ... ok
test test_hardware_info_serde_roundtrip ... ok
test test_inference_caps_non_default_roundtrip ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/node_registry_tests.rs (target/debug/deps/node_registry_tests-d2d64d34330f0402)

running 5 tests
test test_empty_registry_returns_none ... ok
test test_register_all_replaces_prior_contents ... ok
test test_concurrent_get_during_register_all_does_not_deadlock ... ok
test test_list_returns_all ... ok
test test_register_all_populates ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/node_tests.rs (target/debug/deps/node_tests-35aa87ba8002f20e)

running 4 tests
test test_node_type_descriptor_empty_slots ... ok
test test_node_type_descriptor_construction ... ok
test test_slot_descriptor_serde_roundtrip ... ok
test test_slot_type_screaming_snake_case_serde ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/worker_tests.rs (target/debug/deps/worker_tests-1233721fb432c988)

running 4 tests
test test_provisioning_state_serde_snake_case ... ok
test test_env_report_serde_roundtrip ... ok
test test_worker_info_construction_and_serde_roundtrip ... ok
test test_worker_status_serde_snake_case ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/scanner_tests.rs (target/debug/deps/scanner_tests-d32b7a654645cfb)

running 20 tests
test test_format_inference_safetensors ... ok
test test_dtype_inference_bf16 ... ok
test test_hash_small_file ... ok
test test_format_inference_ckpt ... ok
test test_dtype_inference_fp16 ... ok
test test_depth_limit_respected ... ok
test test_depth_zero_scans_only_root ... ok
test test_dtype_inference_fp32 ... ok
test test_dtype_inference_fp8_e4m3fn ... ok
test test_kind_inference_diffusion ... ok
test test_kind_inference_vae ... ok
test test_format_inference_pt ... ok
test test_format_inference_bin ... ok
test test_root_level_kind_unknown ... ok
test test_kind_inference_unknown_dir ... ok
test test_kind_inference_text_encoders ... ok
test test_unchanged_file_skips_rehash ... ok
test test_multiple_files_scanned ... ok
test test_mixed_formats_and_dtypes ... ok
test test_hash_stability_across_rename ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

     Running tests/seed_loader_tests.rs (target/debug/deps/seed_loader_tests-82d5123079f0d9fa)

running 8 tests
test test_already_applied_unseen_seed_returns_false ... ok
test test_already_applied_hash_mismatch_returns_false ... ok
test test_already_applied_hash_match_returns_true ... ok
test test_seed_log_created_on_first_use ... ok
test test_run_malformed_sql_returns_err_no_partial_state ... ok
test test_run_first_time_applies_and_records ... ok
test test_run_skips_when_already_applied ... ok
test test_run_reapplies_on_changed_content ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/store_tests.rs (target/debug/deps/store_tests-3a5017dec58e56e1)

running 5 tests
test test_get_missing_id_returns_none ... ok
test test_list_no_filter ... ok
test test_upsert_get_roundtrip ... ok
test test_delete_removes_row ... ok
test test_list_with_kind_filter ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/db_tests.rs (target/debug/deps/db_tests-f97f9a6b52b2e1e9)

running 4 tests
test test_wal_mode_enabled ... ok
test test_pool_creation_succeeds ... ok
test test_migrations_create_tables ... ok
test test_migrations_idempotent ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/device_store_tests.rs (target/debug/deps/device_store_tests-1a395a5c14541029)

running 5 tests
test test_lookup_known_pciid_returns_caps ... ok
test test_lookup_integer_to_bool_mapping ... ok
test test_lookup_unknown_pciid_returns_none ... ok
test test_lookup_boundary_0xffff ... ok
test test_lookup_multiple_ids_no_interference ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/cpu_tests.rs (target/debug/deps/cpu_tests-0fea5c8484d6a21b)

running 6 tests
test test_cpu_detect_never_errors ... ok
test test_cpu_detector_device_type_is_cpu ... ok
test test_cpu_detector_enumeration_source_is_cpu ... ok
test test_cpu_detector_returns_one_device ... ok
test test_cpu_detector_all_device_fields ... ok
test test_cpu_detector_refresh_vram_returns_zero ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/detect_tests.rs (target/debug/deps/detect_tests-a811c67353725082)

running 14 tests
test test_inference_caps_is_caps_union ... ok
test test_cpu_device_always_present_and_last ... ok
test test_host_fields_non_empty ... ok
test test_inference_caps_union_correctness ... ok
test test_mock_detector_env_vars_propagate_through_detect_all_devices ... ok
test test_override_cpu_device_type ... ok
test test_override_inference_caps_is_default ... ok
test test_mock_hardware_feature_returns_mock_device ... ok
test test_override_absent_returns_hardware_info ... ok
test test_override_path_still_has_cpu_device ... ok
test test_override_present_returns_device ... ok
test test_override_rocm_device_type ... ok
test test_override_unrecognized_device_type_defaults_to_cpu ... ok
test test_override_takes_priority_over_mock ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/mock_tests.rs (target/debug/deps/mock_tests-dc11094eecb9dd9b)

running 6 tests
test test_mock_device_name_override ... ok
test test_mock_detector_defaults ... ok
test test_mock_rocm_device_type ... ok
test test_mock_cuda_device_type ... ok
test test_mock_vram_override ... ok
test test_mock_refresh_vram ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/sysfs_tests.rs (target/debug/deps/sysfs_tests-f7a7eea837e80ef4)

running 7 tests
test test_sysfs_detect_missing_path_returns_empty ... ok
test test_sysfs_refresh_vram_returns_zero ... ok
test test_sysfs_detect_never_errors ... ok
test test_sysfs_detect_nvidia_vendor ... ok
test test_sysfs_detect_synthetic_display_device ... ok
test test_sysfs_filter_non_display_class ... ok
test test_sysfs_multi_device_filter ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/vulkan_tests.rs (target/debug/deps/vulkan_tests-d5d6bc20d7014e4e)

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

     Running tests/roundtrip_tests.rs (target/debug/deps/roundtrip_tests-ab7783d044bdcc44)

running 4 tests
test test_publish_one_subscriber_delivers ... ok
test test_publish_multiple_subscribers_independent_copies ... ok
test test_publish_zero_subscribers ... ok
test test_subscribe_returns_valid_receiver ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

     Running tests/store_tests.rs (target/debug/deps/store_tests-1df3494910e22a84)

running 9 tests
test test_get_unknown_hash_returns_none ... ok
test test_list_empty_table_returns_empty_vec ... ok
test test_save_then_get_roundtrips ... ok
test test_get_after_duplicate_save_returns_original_content ... ok
test test_save_writes_file_once ... ok
test test_duplicate_save_does_not_duplicate_or_error ... ok
test test_different_content_produces_different_hash ... ok
test test_list_with_job_id_filter ... ok
test test_list_without_filter_returns_all ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/health_tests.rs (target/debug/deps/health_tests-c8e7ae423af92183)

running 1 test
test test_health_returns_200 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cli_help_test.rs (target/debug/deps/cli_help_test-2e60b1b3f35b8cd8)

running 1 test
test tests::cli_help_shows_all_flags ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/config_reference.rs (target/debug/deps/config_reference-0567a83466f2ce81)

running 1 test
test tests::config_reference_matches_defaults ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/hw_probe_help_test.rs (target/debug/deps/hw_probe_help_test-45cc931717044633)

running 1 test
test tests::hw_probe_help_shows_subcommand ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/logging_tests.rs (target/debug/deps/logging_tests-6e5c94857ec74b90)

running 2 tests
test tests::test_anvilml_log_debug_yields_stderr ... ok
test tests::test_rust_log_debug_yields_stderr ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/shutdown_tests.rs (target/debug/deps/shutdown_tests-3cec80afbe3618fc)

running 2 tests
test tests::test_shutdown_signal_timeout_cancels ... ok
test tests::test_shutdown_signal_returns_on_ctrl_c ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s

all doctests in 14 crates: 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

all doctests in 14 crates: 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Doc-tests anvilml_artifacts: 0 passed; 0 failed
Doc-tests anvilml_core: 0 passed; 0 failed
Doc-tests anvilml_hardware: 0 passed; 0 failed
Doc-tests anvilml_ipc: 0 passed; 0 failed
Doc-tests anvilml_registry: 2 passed; 0 failed
Doc-tests anvilml_scheduler: 0 passed; 0 failed
Doc-tests anvilml_server: 0 passed; 0 failed
Doc-tests anvilml_worker: 0 passed; 0 failed
Doc-tests anvilml: 0 passed; 0 failed
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.00s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.99s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.87s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.23s
```

All four platform cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  Running tests/config_reference.rs
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Gate 1 passed. No other gates are triggered by this task (it does not modify handler signatures, node types, or arch module methods).

## Public API Delta

```
(no new pub items introduced — only additive derives on existing pub types)
```

The four types `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` already existed as `pub` items. Adding `ToSchema` to their derive lists does not change their public signatures or visibility. No new `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub const`, `pub type`, or `pub mod` items were introduced.

## Deviations from Plan

- **PathBuf schema annotation:** The plan's approach assumed `PathBuf` would compile with `ToSchema` out of the box. In utoipa 5.5.0, `PathBuf` does not implement `ToSchema`, producing a compile error. Added `#[schema(value_type = String)]` to `ModelMeta::path` — following the identical pattern already established in `crates/anvilml-core/src/types/artifact.rs` for `ArtifactMeta::file_path`. This is a minimal, correct fix that does not change serde behavior (PathBuf still serialises as a JSON string).

## Blockers

None.
