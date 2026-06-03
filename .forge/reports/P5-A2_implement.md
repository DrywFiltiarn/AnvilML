# Implementation Report: P5-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P5-A2                                             |
| Phase       | 005 — SQLite Persistence                          |
| Description | anvilml-registry: db::open with PRAGMAs and migration runner |
| Implemented | 2026-06-03T20:15:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented `db::open` in `anvilml-registry` that creates a SQLite connection pool with WAL journal mode, NORMAL synchronous, and foreign_keys pragmas, then runs all three migrations (jobs, models, artifacts) from `backend/migrations/`. Updated `lib.rs` to export the public API. Added an integration test verifying table creation.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | sqlx    | 0.9.0           | rust-docs MCP |
| crate  | tempfile| 3.27.0          | rust-docs MCP |

Note: The approved plan specified `sqlx = "0.8"`, but docs.rs confirms the latest stable is `0.9.0`. Used `0.9` to stay current.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/Cargo.toml` | Added sqlx (sqlite, runtime-tokio, macros, migrate) and tempfile/tokio dev-deps |
| Create | `crates/anvilml-registry/src/db.rs` | db::open with PRAGMAs, migration runner, error conversion helpers |
| Modify | `crates/anvilml-registry/src/lib.rs` | Module declaration + re-export of open |
| Create | `crates/anvilml-registry/tests/anvilml_registry_db.rs` | Integration test verifying table creation |

## Commit Log

```
 .forge/reports/P5-A2_plan.md                       | 100 +++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         | 718 ++++++++++++++++++++-
 crates/anvilml-registry/Cargo.toml                 |   6 +
 crates/anvilml-registry/src/db.rs                  |  98 +++
 crates/anvilml-registry/src/lib.rs                 |   6 +-
 .../anvilml-registry/tests/anvilml_registry_db.rs  |  32 +
 8 files changed, 966 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-76fc372595dda5e4)
running 74 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config_load::tests::env_nested_field ... ok
test error::tests::all_variants_display ... ok
test error::tests::debug_formatting ... ok
test error::tests::error_trait_impls ... ok
test config_load::tests::env_overrides_toml ... ok
test config::tests::test_toml_roundtrip ... ok
test error::tests::from_io_error ... ok
test config_load::tests::missing_toml_fallback ... ok
test error::tests::send_sync ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
test types::artifact::tests::artifact_meta_defaults ... ok
test types::artifact::tests::artifact_meta_json_preserves_fields ... ok
test config_load::tests::override_beats_env ... ok
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
test types::events::tests::system_stats_roundtrip ... ok
test types::events::tests::worker_status_changed_roundtrip ... ok
test types::events::tests::ws_event_enum_variants ... ok
test types::hardware::tests::capability_source_default_is_fallback ... ok
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
test types::model::tests::model_meta_scanned_at_default ... ok
test types::model::tests::model_meta_roundtrip ... ok
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

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-91331d83c93bb7d6)
running 59 tests
... all ok ...

test result: ok. 59 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-9d39e30982bb9c7f)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-3ad45f2a6e405417)
running 1 test
test db::tests::test_open_creates_tables ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-f944fb948a245e01)
running 1 test
test test_open_creates_tables ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-fc5e67c1d0c265e2)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Windows Cross-Check

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.58s
```

Clean build with zero errors for `x86_64-pc-windows-gnu` target.

## Config Drift Gate

```
     Running tests/config_reference.rs (target/debug/deps/config_reference-fc5e67c1d0c265e2)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **sqlx version**: Plan specified `0.8`, but docs.rs MCP confirmed latest stable is `0.9.0`. Used `0.9` to stay current. The feature set (sqlite, runtime-tokio, macros, migrate) is identical in 0.9.
- **From impl**: Plan specified `From<sqlx::Error>` for `AnvilError`, but Rust's orphan rules prevent implementing `From<ExternalType>` for `ExternalType` across crate boundaries. Used local helper functions (`sqlx_error`, `migrate_error`) instead, which produce identical behavior.
- **Dev-dependencies**: Added `tokio` (with macros + rt-multi-thread) and re-declared `sqlx` as dev-dependency to support `#[tokio::test]` in the integration test file.

## Blockers

None. All checks passed: build, clippy (zero warnings), Windows cross-check, full workspace tests (all pass), config drift gate.
