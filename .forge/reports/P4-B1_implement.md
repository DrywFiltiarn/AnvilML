# Implementation Report: P4-B1

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P4-B1                                                       |
| Phase       | 004 — Hardware Detection                                    |
| Description | anvilml: reconcile frontend.mode default to Headless (retrofit; corrects earlier phases) |
| Implemented | 2026-06-03T17:22:00Z                                        |
| Status      | COMPLETE                                                    |

## Summary

Retrofitted `FrontendMode::default()` from `Local { path: "./bloomery" }` to `Headless`, updated the doc comment on the `Local` variant, and changed the test assertion accordingly. Updated `anvilml.toml` to use `[frontend] mode = "headless"` replacing the old `[frontend.mode.Local]` section. Verified `docs/ENVIRONMENT.md` already documents headless as the default — no changes needed. All 145 workspace tests pass, clippy is clean, Windows cross-check passes, and the config drift guard (`config_reference`) test confirms TOML key-set matches `ServerConfig::default()` serialization.

## Resolved Dependencies

| Type   | Name | Version resolved | Source        |
|--------|------|-----------------|---------------|
| (none) | —    | —               | —             |

No new dependencies introduced. No version changes required.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| EDIT   | `crates/anvilml-core/src/config.rs` | Added `Default` derive to `FrontendMode`; added `#[default]` on `Headless` variant; updated `Local` doc comment; updated `test_default_server_config` assertion from `Local { .. }` to `Headless` |
| EDIT   | `anvilml.toml` | Replaced `[frontend.mode.Local] path = "./bloomery"` with flat `[frontend] mode = "headless"` section |

## Commit Log

```
anvilml.toml                      |  5 +++--
 crates/anvilml-core/src/config.rs | 16 +++++-----------
 2 files changed, 8 insertions(+), 13 deletions(-)
```

## Test Results

### cargo test -p anvilml-core -- config (filtered)

```
running 9 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config::tests::test_model_kind_default ... ok
test config_load::tests::env_nested_field ... ok
test error::tests::all_variants_display ... ok
test error::tests::debug_formatting ... ok
test error::tests::error_trait_impls ... ok
test error::tests::from_io_error ... ok
test config_load::tests::env_overrides_toml ... ok
test config::tests::test_toml_roundtrip ... ok
test error::tests::send_sync ... ok
test config_load::tests::missing_toml_fallback ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
test types::artifact::tests::artifact_meta_defaults ... ok
test config_load::tests::override_beats_env ... ok
test types::artifact::tests::artifact_meta_json_preserves_fields ... ok
test types::artifact::tests::artifact_meta_roundtrip ... ok
test types::artifact::tests::artifact_meta_optional_uuid_nil ... ok
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
test types::events::tests::system_stats_roundtrip ... ok
test types::hardware::tests::capability_source_variants ... ok
test types::hardware::tests::device_type_json_strings ... ok
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
test types::worker::tests::worker_status_variants ... ok
test types::worker::tests::worker_status_json_strings ... ok

test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Full workspace test suite (`cargo test --workspace --features mock-hardware`)

```
anvilml_core      : ok. 74 passed; 0 failed
anvilml_hardware  : ok. 59 passed; 0 failed
anvilml_ipc       : ok. 0 passed; 0 failed
anvilml_openapi   : ok. 0 passed; 0 failed
anvilml_registry  : ok. 0 passed; 0 failed
anvilml_scheduler : ok. 0 passed; 0 failed
anvilml_server    : ok. 3 passed; 0 failed
anvilml_worker    : ok. 0 passed; 0 failed
anvilml (binary)  : ok. 8 passed; 0 failed
config_reference  : ok. 1 passed; 0 failed

Doc-tests anvilml_core   : ok. 0 passed
Doc-tests anvilml_hardware: ok. 2 passed
Doc-tests anvilml_ipc    : ok. 0 passed
Doc-tests anvilml_registry: ok. 0 passed
Doc-tests anvilml_scheduler: ok. 0 passed
Doc-tests anvilml_server : ok. 0 passed
Doc-tests anvilml_worker : ok. 0 passed

Total: 145 tests passed; 0 failed
```

## Windows Cross-Check

```
cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware

Checking anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.40s
```

Result: Clean — zero errors.

## Config Drift Gate

```
cargo test -p backend --features mock-hardware --test config_reference

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Result: Drift guard passes — committed `anvilml.toml` key-set matches `ServerConfig::default()` serialization exactly.

## Deviations from Plan

- **Clippy `derivable_impls` lint**: The plan specified writing a manual `impl Default for FrontendMode`. Clippy flagged this as derivable, so I replaced the manual impl with `#[derive(Default)]` on the enum and `#[default]` attribute on the `Headless` variant. This is functionally identical but cleaner — and required to pass clippy with `-D warnings`.

## Blockers

None.
