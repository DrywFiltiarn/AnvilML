# Implementation Report: P3-A9

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P3-A9                                       |
| Phase         | 003 — Core Domain Types: Data Model         |
| Description   | anvilml-core: WsEvent worker/system/provisioning variants |
| Implemented   | 2026-06-28T21:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Added three system-level event variants (`WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`) to the `WsEvent` enum in `crates/anvilml-core/src/types/events.rs`, importing `WorkerStatus` and `WorkerInfo` from the existing `types::worker` module. Updated the module-level doc comment to reflect ten variants instead of seven. Added three serde roundtrip tests to `crates/anvilml-core/tests/events_tests.rs`, bringing the total to ten. Bumped `anvilml-core` patch version from 0.1.14 to 0.1.15. All tests, lint, format, and platform cross-checks pass.

## Resolved Dependencies

None. This task only adds enum variants referencing existing types (`WorkerStatus`, `WorkerInfo`, `DeviceType`) already defined in the `anvilml-core` crate. No new external dependencies are introduced.

| Type   | Name | Version resolved | Source        |
|--------|------|------------------|---------------|
| (none) | —    | —                | —             |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/events.rs` | Added `use super::worker::{WorkerInfo, WorkerStatus}` import; added three `WsEvent` variants (`WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`); updated module-level doc comment from "seven" to "ten" variants |
| Modify | `crates/anvilml-core/tests/events_tests.rs` | Added `DeviceType`, `WorkerInfo`, `WorkerStatus` imports; added three serde roundtrip tests |
| Modify | `crates/anvilml-core/Cargo.toml` | Bumped patch version 0.1.14 → 0.1.15 |
| Modify | `docs/TESTS.md` | Added three test catalogue entries for the new tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md              |  6 +--
 .forge/state/state.json                   | 13 ++---
 Cargo.lock                                |  2 +-
 crates/anvilml-core/Cargo.toml            |  2 +-
 crates/anvilml-core/src/types/events.rs   | 48 +++++++++++++++--
 crates/anvilml-core/tests/events_tests.rs | 87 ++++++++++++++++++++++++++++++-
 docs/TESTS.md                             | 36 +++++++++++++
 7 files changed, 178 insertions(+), 16 deletions(-)
```

## Test Results

```
     Running tests/events_tests.rs (target/debug/deps/events_tests-72480bf43abf713a)

running 10 tests
test test_ws_event_job_cancelled_serde_roundtrip ... ok
test test_ws_event_job_completed_serde_roundtrip ... ok
test test_ws_event_job_failed_serde_roundtrip ... ok
test test_ws_event_job_image_ready_serde_roundtrip ... ok
test test_ws_event_job_queued_serde_roundtrip ... ok
test test_ws_event_job_progress_serde_roundtrip ... ok
test test_ws_event_job_started_serde_roundtrip ... ok
test test_ws_event_provisioning_progress_serde_roundtrip ... ok
test test_ws_event_worker_status_changed_serde_roundtrip ... ok
test test_ws_event_system_stats_serde_roundtrip ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: all 82 tests passed across all crates. Zero failures.

## Format Gate

```
(no output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.15s

# 2. Real-hardware Linux (anvilml binary)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.34s

# 3. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.11s

All four checks exited 0.
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
Running tests/config_reference.rs ...
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:**
`api/openapi.json` does not yet exist in the repository (prior to the phase that introduces the `anvilml-openapi` binary with generated schema). Gate not applicable — per ENVIRONMENT.md §8, the gate is skipped only when `api/openapi.json` does not yet exist.

## Public API Delta

```
(no new pub fn, pub struct, pub trait items introduced)
```

The three new enum variants (`WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`) are `pub` by virtue of `WsEvent` being `pub`. They are data-only struct variants with no `pub fn` or `pub struct` items added.

## Deviations from Plan

None. All implementation matches the approved plan exactly:
- Import path `super::worker::{WorkerInfo, WorkerStatus}` verified correct.
- All three variants added with exact field names and types from the plan.
- Module doc comment updated from "seven" to "ten".
- All three tests follow the exact pattern specified in the plan.
- `SystemStats` test includes minimal `WorkerInfo` in `workers` vec as planned.

## Blockers

None.
