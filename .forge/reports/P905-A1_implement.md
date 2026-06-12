# Implementation Report: P905-A1

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P905-A1                                             |
| Phase       | 905 — FP8 dtype support                             |
| Description | anvilml-core: add F8E4M3 and F8E5M2 variants to DType enum |
| Implemented | 2026-06-12T11:48:00Z                                |
| Status      | COMPLETE                                            |

## Summary

Added `F8E4M3` and `F8E5M2` variants to the `DType` enum in `crates/anvilml-core/src/types/model.rs`, placed after `BF16` and before `Q8`. Each variant carries an explicit `#[serde(rename = ...)]` attribute to ensure correct snake_case serialization (`f8_e4m3` / `f8_e5m2`), since Rust's default snake_case conversion would produce `f8_e4_m3`. Updated all existing tests to cover the new variants and added a new test asserting exact serialization strings. Also fixed a non-exhaustive match in `anvilml-registry`'s `vram_estimate_mib` function to cover the new variants. Bumped `anvilml-core` patch version from `0.1.2` to `0.1.3`.

## Resolved Dependencies

No new dependencies added. This task only adds enum variants and tests.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/model.rs` | Added F8E4M3 and F8E5M2 enum variants with serde rename attributes; updated `dtype_variants` and `dtype_roundtrip_json` tests; added `dtype_f8_serde_strings` test |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3` |
| Modify | `crates/anvilml-registry/src/scanner.rs` | Fixed non-exhaustive match in `vram_estimate_mib` to cover `F8E4M3` and `F8E5M2` (factor 0.5, grouped with Q8) |

## Commit Log

```
 .forge/state/CURRENT_TASK.md           |  6 +++---
 .forge/state/state.json                | 13 +++++++------
 Cargo.lock                             |  2 +-
 crates/anvilml-core/Cargo.toml         |  2 +-
 crates/anvilml-core/src/types/model.rs | 24 +++++++++++++++++++++++-
 crates/anvilml-registry/src/scanner.rs |  2 +-
 6 files changed, 36 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-a571643e7e057e5a)

running 75 tests
test config::tests::test_device_type_default ... ok
test config::tests::test_default_server_config ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_toml_roundtrip ... ok
test config_load::tests::env_nested_field ... ok
test error::tests::all_variants_display ... ok
test config_load::tests::missing_toml_fallback ... ok
test error::tests::debug_formatting ... ok
test error::tests::error_trait_impls ... ok
test config_load::tests::env_overrides_toml ... ok
test error::tests::send_sync ... ok
test config_load::tests::override_beats_env ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
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
test types::events::tests::job_queued_roundtrip ... ok
test types::events::tests::job_progress_roundtrip ... ok
test types::events::tests::job_started_roundtrip ... ok
test types::events::tests::system_stats_event_json ... ok
test types::events::tests::system_stats_roundtrip ... ok
test types::events::tests::ws_event_enum_variants ... ok
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
test types::hardware::tests::inference_caps_defaults ... ok
test types::hardware::tests::hardware_info_roundtrip ... ok
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
test types::model::tests::dtype_f8_serde_strings ... ok
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

test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
    Checking anvilml-core v0.1.3 (/home/dryw/AnvilML/crates/anvilml-core)
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-ipc v0.1.4 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-hardware v0.1.1 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-worker v0.1.21 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.18 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.18 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.1 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.13 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.65s

# 2. Mock-hardware Windows cross
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.77s

# 3. Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.04s

# 4. Real-hardware Windows cross
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.46s
```

## Project Gates

```
Gate 1 — Config Surface Sync:
     Running tests/config_reference.rs (target/debug/deps/config_reference-aa83f75977bccf09)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **Serde rename attributes required:** The plan assumed `#[serde(rename_all = "snake_case")]` would produce `f8_e4m3` / `f8_e5m2` from `F8E4M3` / `F8E5M2`. In practice, serde's snake_case conversion treats consecutive uppercase letters as separate words, producing `f8_e4_m3` / `f8_e5_m2`. Added `#[serde(rename = "f8_e4m3")]` and `#[serde(rename = "f8_e5m2")]` to the variants to get the correct serialization.
- **anvilml-registry scanner match fix:** The `vram_estimate_mib` function in `crates/anvilml-registry/src/scanner.rs` had an exhaustive match on `DType` that did not cover the new variants. Added `F8E4M3` and `F8E5M2` with factor 0.5 (same as Q8, since both are 1-byte types). This is a necessary fix to keep the workspace compiling.

## Blockers

None.
