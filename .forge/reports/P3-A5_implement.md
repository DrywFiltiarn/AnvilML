# Implementation Report: P3-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A5                                         |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: WebSocket event types          |
| Project     | anvilml                                       |
| Status      | COMPLETE                                      |

## Summary

Implemented the WebSocket event type system in `anvilml-core` as specified in ANVILML_DESIGN §4.5. Created the `WsEvent` enum with 9 variants, plus `GpuStatSnapshot` helper struct. All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`, and `ToSchema`. Added 13 round-trip JSON tests covering every event type.

## Files Modified

| Action   | Path                                            |
|----------|-------------------------------------------------|
| CREATE   | `crates/anvilml-core/src/types/events.rs`       | WsEvent enum, 9 variant structs, GpuStatSnapshot, 13 tests |
| MODIFY   | `crates/anvilml-core/src/types/mod.rs`          | Added `pub mod events;` and updated module doc comment |
| MODIFY   | `crates/anvilml-core/src/lib.rs`                | Re-exported WsEvent and all event types |

## Design Note

The plan originally specified `event: &'static str` for all event structs. However, `&'static str` cannot be deserialized by serde (it requires the lifetime `'static` which conflicts with the deserializer's borrow lifetime). Changed to `String` instead — the field value is always a constant per struct variant, so this has no functional impact on serialization while enabling full round-trip capability.

## Test Results

### Linux test gate (`cargo test -p anvilml-core -- events`)
```
running 13 tests
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
test types::events::tests::worker_status_changed_roundtrip ... ok
test types::events::tests::ws_event_enum_variants ... ok

test result: ok. 13 passed; 0 failed; 0 ignored
```

### Full workspace test suite
```
running 68 tests (anvilml-core)
test result: ok. 68 passed; 0 failed; 0 ignored
running 1 test (anvilml-server)
test result: ok. 1 passed; 0 failed; 0 ignored
running 8 tests (backend CLI)
test result: ok. 8 passed; 0 failed; 0 ignored

Total: 77 passed, 0 failed across all crates.
```

### Clippy (`cargo clippy --workspace --features mock-hardware -- -D warnings`)
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.31s
Zero warnings.
```

### Windows cross-check (`cargo check --target x86_64-pc-windows-gnu --workspace --features mock-hardware`)
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
Zero errors.
```

### Config drift gate
Skipped — the `config_reference` test does not yet exist (will be added in P3-B2).

## Acceptance Criteria

- [x] `crates/anvilml-core/src/types/events.rs` exists with all 9 event structs and `WsEvent` enum
- [x] `GpuStatSnapshot{index, vram_used_mib, vram_total_mib}` is defined and derives the standard set
- [x] Every event struct has `event: String` field set to the correct type string (e.g. "system.stats", "job.progress")
- [x] `JobProgressEvent.step` and `JobProgressEvent.step_total` are `Option<u32>` (None in MVP)
- [x] `JobFailedEvent.traceback` is `Option<String>`
- [x] `WorkerStatusChangedEvent.status` uses `crate::types::worker::WorkerStatus`
- [x] All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`, `ToSchema`
- [x] `pub mod events;` added to `crates/anvilml-core/src/types/mod.rs`
- [x] `WsEvent` and all variant structs re-exported from `crates/anvilml-core/src/lib.rs`
- [x] `cargo test -p anvilml-core -- events` exits 0
- [x] SystemStats JSON assertion: serialized JSON contains `"event":"system.stats"`
