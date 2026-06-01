# Implementation Report: P3-A2

| Field          | Value                                              |
|----------------|-----------------------------------------------------|
| Task ID        | P3-A2                                                |
| Phase          | 003 — Core Domain Types                              |
| Description    | anvilml-core: Job domain types                       |
| Project        | anvilml                                              |
| Implemented at | 2026-06-01T10:05:00Z                                 |
| Attempt        | 1                                                    |

## Summary

Implemented the complete Job domain type hierarchy in `anvilml-core` as specified in ANVILML_DESIGN §4.1. Created `JobStatus` enum (5 variants), `JobSettings` struct (6 fields with defaults), `Job` struct (12 fields with optional timestamp/string/numeric defaults), `SubmitJobRequest`, and `SubmitJobResponse` — all deriving `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`. Added `chrono` (0.4, serde feature), `utoipa` (5, chrono + uuid features), and `serde_json` (1) to crate dependencies. Updated `lib.rs` to export the new `types` module. Wrote 11 unit tests covering round-trip serialization, variant count/equality, defaults, ISO 8601 timestamp format, and optional field handling.

## Files Changed

| Action   | Path                              | Description                                                      |
|----------|-----------------------------------|------------------------------------------------------------------|
| MODIFY   | crates/anvilml-core/Cargo.toml    | Added `chrono`, `utoipa` (chrono+uuid features), `serde_json` dependencies |
| CREATE   | crates/anvilml-core/src/types/mod.rs | Module declaration for job types                            |
| CREATE   | crates/anvilml-core/src/types/job.rs | JobStatus, JobSettings, Job, SubmitJobRequest, SubmitJobResponse + 11 unit tests |
| MODIFY   | crates/anvilml-core/src/lib.rs    | Added `pub mod types;`                                           |

## Test Results

### Job-specific unit tests (anvilml-core)

```
   Compiling anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.26s
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-19ae8861237c1a19)

running 11 tests
test types::job::tests::job_optional_numeric_fields_default ... ok
test types::job::tests::job_optional_string_fields_default_none ... ok
test types::job::tests::job_graph_json_value ... ok
test types::job::tests::job_roundtrip ... ok
test types::job::tests::job_optional_timestamps_default_none ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_settings_roundtrip ... ok
test types::job::tests::job_status_variants ... ok
test types::job::tests::job_timestamps_iso8601 ... ok
test types::job::tests::submit_job_request_roundtrip ... ok
test types::job::tests::submit_job_response_roundtrip ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s

   Doc-tests anvilml_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Full workspace test suite

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-07dea96ced852234)

running 25 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config_load::tests::env_nested_field ... ok
test error::tests::all_variants_display ... ok
test config_load::tests::env_overrides_toml ... ok
test error::tests::debug_formatting ... ok
test config_load::tests::missing_toml_fallback ... ok
test config::tests::test_toml_roundtrip ... ok
test error::tests::from_io_error ... ok
test error::tests::error_trait_impls ... ok
test error::tests::send_sync ... ok
test config_load::tests::override_beats_env ... ok
test types::job::tests::job_optional_numeric_fields_default ... ok
test types::job::tests::job_graph_json_value ... ok
test types::job::tests::job_optional_string_fields_default_none ... ok
test types::job::tests::job_optional_timestamps_default_none ... ok
test types::job::tests::job_roundtrip ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_settings_roundtrip ... ok
test types::job::tests::job_status_variants ... ok
test types::job::tests::job_timestamps_iso8601 ... ok
test types::job::tests::submit_job_request_roundtrip ... ok
test types::job::tests::submit_job_response_roundtrip ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-e900805a38464db4)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-ac6fa962a14fee4d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-556918d2bad7ae5f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-ac13e72bb2559f83)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-68df2ef2e9832e07)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-288ec98f2defc051)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-3ca62cf2a913c339)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-76b19bd34a47f292)
running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_core through anvilml_worker — all ok (0 tests each)
```

### Clippy (zero warnings)

```
     Checking anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
     Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
     Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
     Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
     Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
     Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
     Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
     Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
     Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.38s
```

### Windows cross-check (x86_64-pc-windows-gnu)

```
   Compiling serde_core v1.0.228
   Compiling autocfg v1.5.1
   Compiling num-traits v0.2.19
   Compiling chrono v0.4.44
   Compiling uuid v1.23.2
   Compiling utoipa-gen v5.5.0
    Checking utoipa v5.5.0
    Checking anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
    Checking anvilml-ipc v0.1.0 (/home/dryw/AnvilML/crates/anvilml-ipc)
    Checking anvilml-hardware v0.1.0 (/home/dryw/AnvilML/crates/anvilml-hardware)
    Checking anvilml-registry v0.1.0 (/home/dryw/AnvilML/crates/anvilml-registry)
    Checking anvilml-worker v0.1.0 (/home/dryw/AnvilML/crates/anvilml-worker)
    Checking anvilml-scheduler v0.1.0 (/home/dryw/AnvilML/crates/anvilml-scheduler)
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.36s
```

## CI Changes

No CI changes made.

## Commit Log

```
M  .forge/state/CURRENT_TASK.md
M  Cargo.lock
M  crates/anvilml-core/Cargo.toml
A  crates/anvilml-core/src/types/mod.rs
A  crates/anvilml-core/src/types/job.rs
M  crates/anvilml-core/src/lib.rs
```

## Acceptance Criteria — Verification

| Criterion                                         | Status | Evidence                                    |
|---------------------------------------------------|--------|---------------------------------------------|
| `cargo test -p anvilml-core -- job` exits 0       | PASS   | 11 tests passed, 0 failed                   |
| Cargo.toml contains chrono (serde), utoipa (chrono+uuid), serde_json | PASS | Dependencies added to `[dependencies]` |
| JobStatus enum has exactly 5 variants             | PASS   | `test types::job::tests::job_status_variants ... ok` |
| All types derive Serialize, Deserialize, Clone, Debug, ToSchema | PASS | Compile succeeds with all derives |
| JobStatus additionally derives PartialEq, Eq      | PASS   | Equality/inequality tests pass              |
| `Job.graph` is typed as `serde_json::Value`       | PASS   | Field declared as `pub graph: Value`        |
| Timestamp fields use `DateTime<Utc>` and serialize as ISO 8601 | PASS | `test types::job::tests::job_timestamps_iso8601 ... ok` |
| `pub mod types;` present in `src/lib.rs`          | PASS   | Verified in file content                    |
| Clippy zero warnings                              | PASS   | Full workspace clippy passes cleanly        |
| Windows cross-check clean                         | PASS   | `cargo check --target x86_64-pc-windows-gnu` succeeds |
