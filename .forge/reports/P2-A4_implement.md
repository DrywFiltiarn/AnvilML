# Implementation Report: P2-A4

| Field          | Value                                              |
|----------------|-----------------------------------------------------|
| Task ID        | P2-A4                                                |
| Phase          | 002 — Core Types & IPC                               |
| Description    | anvilml-core: hardware and worker types + WebSocket event types |
| Project        | anvilml                                              |
| Implemented at | 2026-05-30T00:00:00Z                                 |
| Attempt        | 1                                                    |

## Summary

Implemented the remaining domain types that close out `anvilml-core` for the MVP feature set. Created three new modules in `crates/anvilml-core/src/types/`: **hardware.rs** (5 types: `DeviceType`, `GpuDevice`, `HardwareInfo`, `HostInfo`, `InferenceCaps`), **worker.rs** (2 types: `WorkerStatus`, `WorkerInfo`), and **events.rs** (`WsEvent` enum with serde internally-tagged serialization + 9 event structs + `GpuStatSnapshot`). Updated `types/mod.rs` to declare the three new modules and re-export all types publicly. All 15 new tests pass, plus all existing tests continue to pass (52 total). No CI changes required.

## Files Changed

| Action   | Path                                                | Description                                         |
|----------|-----------------------------------------------------|-----------------------------------------------------|
| CREATE   | crates/anvilml-core/src/types/hardware.rs           | DeviceType, GpuDevice, HardwareInfo, HostInfo, InferenceCaps + 2 tests |
| CREATE   | crates/anvilml-core/src/types/worker.rs             | WorkerStatus, WorkerInfo + 2 tests                   |
| CREATE   | crates/anvilml-core/src/types/events.rs             | WsEvent enum, GpuStatSnapshot, 9 event structs + 11 tests |
| MODIFY   | crates/anvilml-core/src/types/mod.rs                | Added `pub mod hardware`, `pub mod worker`, `pub mod events` and public re-exports for all new types |

## Test Results

```
   Compiling anvilml-core v0.1.0 (/home/dryw/AnvilML/crates/anvilml-core)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.66s
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-dedd2261b169b934)

running 52 tests
test config::tests::config_default_deserialize ... ok
test error::tests::anvil_error_is_send_sync ... ok
test error::tests::display_artifact_not_found ... ok
test error::tests::display_config_load ... ok
test error::tests::display_db_error ... ok
test error::tests::display_invalid_graph ... ok
test error::tests::display_io ... ok
test error::tests::display_job_not_found ... ok
test config::tests::config_round_trip ... ok
test error::tests::display_json ... ok
test error::tests::display_payload_too_large ... ok
test error::tests::display_worker_dead ... ok
test error::tests::from_io_error ... ok
test tests::it_works ... ok
test types::artifact::tests::artifact_meta_datetime_serialization ... ok
test types::artifact::tests::artifact_meta_new ... ok
test types::artifact::tests::artifact_meta_serialization_round_trip ... ok
test types::events::tests::gpu_stat_snapshot_round_trip ... ok
test config::tests::config_frontend_modes ... ok
test types::events::tests::job_cancelled_event_round_trip ... ok
test types::events::tests::job_completed_event_round_trip ... ok
test types::events::tests::job_image_ready_event_round_trip ... ok
test types::events::tests::job_failed_event_round_trip ... ok
test types::events::tests::job_progress_event_round_trip ... ok
test types::events::tests::job_queued_event_round_trip ... ok
test types::events::tests::job_started_event_round_trip ... ok
test types::events::tests::system_stats_event_serialization ... ok
test types::events::tests::worker_status_changed_event_round_trip ... ok
test types::hardware::tests::device_type_serialization_round_trip ... ok
test types::events::tests::ws_event_all_variants_serialize ... ok
test types::hardware::tests::hardware_info_round_trip ... ok
test types::job::tests::job_datetime_serialization ... ok
test types::job::tests::job_id_is_uuid_v4 ... ok
test types::job::tests::job_new_is_pending ... ok
test types::job::tests::job_serialization_round_trip ... ok
test types::job::tests::job_settings_defaults ... ok
test types::job::tests::job_settings_round_trip ... ok
test types::job::tests::job_status_eq ... ok
test types::job::tests::job_status_ord ... ok
test types::job::tests::job_status_serialization_round_trip ... ok
test types::job::tests::submit_job_request_defaults ... ok
test types::job::tests::submit_job_request_to_settings ... ok
test types::job::tests::submit_job_response_round_trip ... ok
test types::model::tests::dtype_eq ... ok
test types::model::tests::dtype_serialization_round_trip ... ok
test types::model::tests::model_kind_eq ... ok
test types::model::tests::model_kind_serialization_round_trip ... ok
test types::model::tests::model_meta_new ... ok
test types::model::tests::model_meta_serialization_round_trip ... ok
test types::model::tests::model_meta_skip_none_dtype ... ok
test types::worker::tests::worker_info_construct_and_round_trip ... ok
test types::worker::tests::worker_status_serialization_round_trip ... ok

test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

New tests added: 15 (hardware: 2, worker: 2, events: 11)
Existing tests preserved: 37
Total passing: 52

## CI Changes

No CI changes made.

## Commit Log

```
 M .forge/state/CURRENT_TASK.md
 M .forge/state/state.json
 M crates/anvilml-core/src/types/mod.rs
?? .forge/reports/P2-A4_plan.md
?? crates/anvilml-core/src/types/events.rs
?? crates/anvilml-core/src/types/hardware.rs
?? crates/anvilml-core/src/types/worker.rs
```

## Acceptance Criteria

| # | Criterion                                                                 | Status |
|---|---------------------------------------------------------------------------|--------|
| 1 | `crates/anvilml-core/src/types/hardware.rs` exists with all 5 types from §4.3 | PASS |
| 2 | `crates/anvilml-core/src/types/worker.rs` exists with both types from §4.4   | PASS |
| 3 | `crates/anvilml-core/src/types/events.rs` exists with WsEvent + 9 structs + GpuStatSnapshot from §4.5 | PASS |
| 4 | WsEvent serializes with "event" discriminator producing `{ "event": "...", "timestamp": "...", ... }` wire format | PASS |
| 5 | `crates/anvilml-core/src/types/mod.rs` re-exports all new types publicly     | PASS |
| 6 | `cargo test -p anvilml-core` exits 0 with ≥10 tests total across all modules (52 passed, 15 new) | PASS |
