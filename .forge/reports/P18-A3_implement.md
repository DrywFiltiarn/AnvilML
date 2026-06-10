# Implementation Report: P18-A3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P18-A3                          |
| Phase         | 018 — Worker Restart API & Preflight |
| Description   | anvilml: Python preflight check populating EnvReport |
| Implemented   | 2026-06-10T22:45:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the Python preflight check that resolves the interpreter path, verifies the interpreter exists, runs `python --version` to parse the version, and (when `ANVILML_WORKER_MOCK` is unset) verifies PyTorch is importable. Results are stored in `AppState.env_report` via a new `set_env_report` method. Job submission is gated with a 503 `workers_unavailable` response when preflight fails (unless in mock mode). The existing test suite was updated to bypass the preflight gate in test context by setting `ANVILML_WORKER_MOCK=1`.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | temp-env  | 0.3.6            | Cargo.lock     |
| crate  | tower     | 0.5.3            | Workspace      |

Note: `temp-env` was added to `anvilml-server` dev-dependencies for test isolation. `tower` was added to `backend` dev-dependencies for `ServiceExt` trait in integration tests. No new runtime dependencies were added.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/src/preflight.rs` | New module: `run_preflight(cfg) -> EnvReport`, `resolve_interpreter`, version parsing |
| MODIFY | `backend/src/main.rs` | Wire preflight call after AppState construction; log preflight result |
| MODIFY | `backend/Cargo.toml` | Add `tower` dev-dependency; bump version 0.1.8 → 0.1.9 |
| MODIFY | `backend/tests/preflight_check.rs` | New integration test file (4 tests) |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `set_env_report` method; update comment and stub reason |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Add `temp-env` dev-dependency; bump version 0.1.12 → 0.1.13 |
| MODIFY | `crates/anvilml-server/src/handlers/jobs.rs` | Gate submit_job with preflight check; update test setup with mock mode; update doc comment and utoipa annotations |
| MODIFY | `crates/anvilml-server/src/handlers/system.rs` | Update doc comment to remove "stubbed" language |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Update `env_returns_200_with_stub_report` test to expect "unavailable" reason |

## Commit Log

```
 .forge/reports/P18-A3_plan.md                | 125 ++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   6 +-
 backend/Cargo.toml                           |   3 +-
 backend/src/main.rs                          |  16 +-
 backend/src/preflight.rs                     | 258 +++++++++++++++++++++
 backend/tests/preflight_check.rs             | 326 +++++++++++++++++++++++++++
 crates/anvilml-server/Cargo.toml             |   3 +-
 crates/anvilml-server/src/handlers/jobs.rs   |  28 ++-
 crates/anvilml-server/src/handlers/system.rs |   5 +-
 crates/anvilml-server/src/lib.rs             |   2 +-
 crates/anvilml-server/src/state.rs           |  13 +-
 13 files changed, 782 insertions(+), 22 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-f3df55d7386c8396)

running 74 tests
test config::tests::test_model_kind_default ... ok
test config::tests::test_device_type_default ... ok
test error::tests::send_sync ... ok
test error::tests::from_io_error ... ok
test error::tests::debug_formatting ... ok
test types::artifact::tests::artifact_meta_defaults ... ok
test types::artifact::tests::artifact_meta_default_impl ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test types::artifact::tests::artifact_meta_json_preserves_fields ... ok
test error::tests::all_variants_display ... ok
test config::tests::test_default_server_config ... ok
test config::tests::test_toml_roundtrip ... ok
test types::artifact::tests::artifact_meta_optional_uuid_nil ... ok
test config_load::tests::override_beats_env ... ok
test types::artifact::tests::artifact_meta_roundtrip ... ok
test config_load::tests::missing_toml_fallback ... ok
test types::events::tests::job_cancelled_roundtrip ... ok
test types::events::tests::job_completed_roundtrip ... ok
test types::events::tests::job_failed_no_traceback ... ok
test types::events::tests::job_failed_roundtrip ... ok
test config_load::tests::env_overrides_toml ... ok
test types::events::tests::job_image_ready_roundtrip ... ok
test types::events::tests::job_progress_optional_fields ... ok
test config_load::tests::env_nested_field ... ok
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
test types::hardware::tests::hardware_info_roundtrip ... ok
test types::hardware::tests::inference_caps_defaults ... ok
test types::hardware::tests::inference_caps_roundtrip ... ok
test types::job::tests::job_graph_json_value ... ok
test types::job::tests::job_optional_numeric_fields_default ... ok
test types::job::tests::job_optional_string_fields_default_none ... ok
test types::job::tests::job_optional_timestamps_default_none ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_roundtrip ... ok
test types::job::tests::job_settings_roundtrip ... ok
test types::job::tests::job_status_variants ... ok
test types::job::tests::job_timestamps_iso8601 ... ok
test types::job::tests::submit_job_request_roundtrip ... ok
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

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-395d68b7d76bba7d)

running 56 tests
... (all passed) ...

test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-5ce179a5e12f9aa5)

running 18 tests
... (all passed) ...

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-07dc3a94706f3425)

running 19 tests
... (all passed) ...

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-d6122c7003f8d1b9)

running 43 tests
... (all passed) ...

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-2a6e171e4c3e3e01)

running 38 tests
test artifact::store::tests::artifact_store_delete_for_job_removes_file ... ok
test artifact::store::tests::artifact_store_save_load_roundtrip ... ok
test artifact::store::tests::artifact_store_save_generates_hash ... ok
test handlers::jobs::tests::cancel_job_returns_202_for_queued_job ... ok
test handlers::jobs::tests::cancel_job_returns_404_when_missing ... ok
test handlers::jobs::tests::cancel_job_returns_409_for_completed_job ... ok
test handlers::jobs::tests::clear_jobs_defaults_to_all ... ok
test handlers::jobs::tests::clear_jobs_removes_artifacts ... ok
test handlers::jobs::tests::clear_jobs_returns_200_for_completed_jobs ... ok
test handlers::jobs::tests::clear_jobs_skips_running_jobs ... ok
test handlers::jobs::tests::delete_job_returns_204_for_completed_job ... ok
test handlers::jobs::tests::delete_job_returns_404_when_missing ... ok
test handlers::jobs::tests::delete_job_returns_409_for_queued_job ... ok
test handlers::jobs::tests::get_job_returns_200_with_queued_job ... ok
test handlers::jobs::tests::list_jobs_filters_by_status ... ok
test handlers::jobs::tests::list_jobs_limit_clamps_to_one ... ok
test handlers::jobs::tests::list_jobs_returns_all_submitted_jobs ... ok
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
... (all passed) ...

test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-1b948e3e1e0c0c8c)

running 66 tests
... (all passed) ...

test result: ok. 66 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/preflight_check.rs (target/debug/deps/preflight_check-8af6c235df629636)

running 4 tests
test env_returns_correct_shape_in_stub_context ... ok
test env_endpoint_reflects_failed_preflight ... ok
test job_submit_proceeds_in_mock_mode ... ok
test job_submit_rejected_when_preflight_fails ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-45642a6e168aa1a)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(No output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Checking anvilml-server v0.1.13 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.9 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.06s

# 2. Mock-hardware Windows cross-check
    Checking anvilml-server v0.1.13 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.9 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.55s

# 3. Real-hardware Linux check
    Checking anvilml-server v0.1.13 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.9 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.85s

# 4. Real-hardware Windows cross-check
    Checking anvilml-server v0.1.13 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.9 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.83s
```

All four platform cross-checks passed with exit code 0.

## Project Gates

```
# Config Surface Sync Gate
     Running tests/config_reference.rs (target/debug/deps/config_reference-45642a6e168aa1a)
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **Test isolation for env vars**: The `build_test_app` function in `jobs.rs` sets `ANVILML_WORKER_MOCK=1` at the start but does not remove it (to avoid race conditions with parallel tests). Existing tests that don't need mock mode are unaffected because they don't call `submit_job`.
- **`env_returns_200_with_stub_report` test**: Updated the expected `reason` value from `"not_checked"` to `"unavailable"` to match the new stub value in `AppState` constructors.
- **Integration test approach**: The integration tests in `backend/tests/preflight_check.rs` use the in-memory `App` approach (same as `lib.rs` tests) rather than spinning up a full server, to keep tests fast and isolated.
- **Version bump**: Bumped `backend` from `0.1.8` to `0.1.9` and `anvilml-server` from `0.1.12` to `0.1.13`. No other crates were modified.

## Blockers

None.
