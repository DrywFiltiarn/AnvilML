# Implementation Report: P3-A11

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-A11                             |
| Phase         | 3 — Core Domain Types: Data Model  |
| Description   | anvilml-core: lib.rs final re-export pass, 80-line check |
| Implemented   | 2026-06-28T21:55:00Z               |
| Status        | COMPLETE                           |

## Summary

Reordered module declarations and `pub use` statements in `crates/anvilml-core/src/lib.rs` to be strictly alphabetical by full module path. The file now contains 15 lines (down from 17), well under the 80-line cap. No new public items were introduced; the public API surface is identical. All 90 workspace tests pass, all four platform cross-checks pass, clippy is clean, and the format gate is clean.

## Resolved Dependencies

None. This task introduces no new dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | crates/anvilml-core/src/lib.rs | Reordered `mod` declarations and `pub use` statements into strict alphabetical order by full module path |
| MODIFY | crates/anvilml-core/Cargo.toml | Bumped patch version 0.1.16 → 0.1.17 |

## Commit Log

```
 .forge/state/CURRENT_TASK.md   |  6 +++---
 .forge/state/state.json        | 13 +++++++------
 Cargo.lock                     |  2 +-
 crates/anvilml-core/Cargo.toml |  2 +-
 crates/anvilml-core/src/lib.rs |  6 ++----
 5 files changed, 14 insertions(+), 15 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml-d4b818a3677877a8)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-359a56030c4a33f3)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cli_help_test.rs (target/debug/deps/cli_help_test-1361d02f6e9c84bf)
running 1 test
test tests::cli_help_shows_all_flags ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-5020c02a9c7cca6a)
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/shutdown_tests.rs (target/debug/deps/shutdown_tests-c84063823c12e489)
running 2 tests
test tests::test_shutdown_signal_timeout_cancels ... ok
test tests::test_shutdown_signal_returns_on_ctrl_c ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_artifacts-5082677c2b98637f)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_core-e2a86bb58143f8e1)
running 1 test
test config_load::tests::test_load_none_path_missing_file_returns_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/artifact_tests.rs (target/debug/deps/artifact_tests-13da59de547eeee9)
running 3 tests
test test_artifact_meta_field_names ... ok
test test_artifact_meta_hash_format ... ok
test test_artifact_meta_serde_roundtrip ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_load_tests.rs (target/debug/deps/config_load_tests-15ff35b2a2050c58)
running 13 tests
test test_cli_override_beats_env_var ... ok
test test_env_var_overrides_toml_value ... ok
test test_load_full_toml_roundtrips_all_fields ... ok
test test_env_var_port_override ... ok
test test_load_default_path_resolves_anvilml_toml ... ok
test test_env_var_overrides_default_no_toml ... ok
test test_load_missing_file_falls_back_to_defaults ... ok
test test_load_partial_toml_overrides_only_specified_fields ... ok
test test_load_malformed_toml_returns_err ... ok
test test_load_nested_struct_partial_override ... ok
test test_nested_env_var_gpu_selection ... ok
test test_num_threads_env_var ... ok
test test_unset_env_vars_leave_prior_layer_value ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_tests.rs (target/debug/deps/config_tests-e7cdbad77e0262fb)
running 13 tests
test test_db_path_default ... ok
test test_gpu_selection_default ... ok
test test_artifact_dir_default ... ok
test test_limits_default ... ok
test test_hardware_override_default ... ok
test test_host_default ... ok
test test_max_ipc_payload_mib_default ... ok
test test_model_dirs_default ... ok
test test_model_scan_depth_default ... ok
test test_num_threads_default ... ok
test test_port_default ... ok
test test_rocm_default ... ok
test test_venv_path_default ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs (target/debug/deps/error_tests-f91575277c9460ea)
running 16 tests
test test_artifact_not_found_returns_404 ... ok
test test_db_returns_500 ... ok
test test_cycle_detected_returns_400 ... ok
test test_error_body_has_request_id ... ok
test test_internal_returns_500 ... ok
test test_error_body_message_contains_variant_info ... ok
test test_error_field_is_snake_case ... ok
test test_invalid_graph_returns_400 ... ok
test test_io_returns_500 ... ok
test test_ipc_returns_400 ... ok
test test_job_not_found_returns_404 ... ok
test test_model_not_found_returns_404 ... ok
test test_payload_too_large_returns_413 ... ok
test test_serde_returns_400 ... ok
test test_worker_not_found_returns_404 ... ok
test test_workers_unavailable_returns_503 ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

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
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/hardware_tests.rs (target/debug/deps/hardware_tests-c855f5a25ce59a37)
running 9 tests
test test_capability_source_serde_snake_case ... ok
test test_enumeration_source_copy_trait ... ok
test test_device_type_serde_snake_case ... ok
test test_gpu_device_construction_and_serde ... ok
test test_enumeration_source_serde_snake_case ... ok
test test_hardware_info_serde_roundtrip ... ok
test test_inference_caps_default_roundtrip ... ok
test test_host_info_serde_roundtrip ... ok
test test_inference_caps_non_default_roundtrip ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/job_tests.rs (target/debug/deps/job_tests-12620cdeda9796b6)
running 4 tests
test test_job_settings_default ... ok
test test_job_status_all_variants_roundtrip ... ok
test test_job_serde_roundtrip ... ok
test test_job_with_nulls_roundtrip ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/model_tests.rs (target/debug/deps/model_tests-ecbd8df176baa60d)
running 4 tests
test test_model_dtype_serde_snake_case ... ok
test test_model_format_serde_snake_case ... ok
test test_model_kind_serde_snake_case ... ok
test test_model_meta_serde_roundtrip ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_registry_tests.rs (target/debug/deps/node_registry_tests-3dcb50030bdbfc32)
running 5 tests
test test_list_returns_all ... ok
test test_empty_registry_returns_none ... ok
test test_register_all_replaces_prior_contents ... ok
test test_register_all_populates ... ok
test test_concurrent_get_during_register_all_does_not_deadlock ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_tests.rs (target/debug/deps/node_tests-52b042a75b8864cd)
running 4 tests
test test_node_type_descriptor_empty_slots ... ok
test test_node_type_descriptor_construction ... ok
test test_slot_descriptor_serde_roundtrip ... ok
test test_slot_type_screaming_snake_case_serde ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/worker_tests.rs (target/debug/deps/worker_tests-51e8ea26660d0580)
running 4 tests
test test_provisioning_state_serde_snake_case ... ok
test test_env_report_serde_roundtrip ... ok
test test_worker_status_serde_snake_case ... ok
test test_worker_info_construction_and_serde_roundtrip ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/health_tests.rs (target/debug/deps/health_tests-d19ae5bb491964e3)
running 1 test
test test_health_returns_200 ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Doc-tests: all 0 passed; 0 failed
```

Total: 90 tests, 0 failures.

## Format Gate

```
(no output — cargo fmt --all -- --check exited 0, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.92s
    CHECK1_OK

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.58s
    CHECK2_OK

# 3. Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.98s
    CHECK3_OK

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.64s
    CHECK4_OK
```

All four checks exited 0.

## Project Gates

Gate 1 — Config Surface Sync:
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 (OpenAPI drift) and Gate 3 (Node Parity) are not triggered — this task modifies only `lib.rs` re-export ordering and has no impact on handler signatures, node types, or `#[utoipa::path]` annotations.

## Public API Delta

```
+pub use node_registry::NodeTypeRegistry;
+pub use types::*;
```

These two lines appear in `git diff HEAD` because they were moved from interleaved positions (near their `mod` declarations) to the end of the file. No new `pub` items were introduced and none were removed. The public API surface is identical to the pre-change state.

Refactor verification (`grep -n "^pub " crates/anvilml-core/src/lib.rs`):
```
5:pub mod config_load;
8:pub mod types;
10:pub use config::ServerConfig;
11:pub use config_load::CliOverrides;
12:pub use config_load::load;
13:pub use error::AnvilError;
14:pub use node_registry::NodeTypeRegistry;
15:pub use types::*;
```

All 8 pub items are unchanged in signature.

Line count: 15 lines (≤80 ✓).

## Deviations from Plan

- **Reordering was required.** The plan stated "Current file already matches this ordering. No reordering is needed." Inspection revealed the actual file had `pub use node_registry::NodeTypeRegistry;` at line 9 and `pub use types::*;` at line 12 — interleaved with their `mod` declarations rather than grouped at the end in full alphabetical order. The `pub use` statements were reordered to match the plan's described alphabetical ordering (`config::ServerConfig`, `config_load::CliOverrides`, `config_load::load`, `error::AnvilError`, `node_registry::NodeTypeRegistry`, `types::*`). This brings the file into compliance with the plan's stated goal of strict alphabetical ordering.

## Blockers

None.
