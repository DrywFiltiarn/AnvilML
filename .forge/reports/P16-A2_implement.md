# Implementation Report: P16-A2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P16-A2                          |
| Phase         | 16 — Live Events                |
| Description   | anvilml-scheduler: event_loop updates Job status in JobStore on events |
| Implemented   | 2026-07-09T12:30:00Z            |
| Status        | COMPLETE                          |

## Summary

This task closes the job-status-persistence gap that existed since Phase 14. The event loop in `crates/anvilml-scheduler/src/event_loop.rs` now handles `WorkerEvent::Completed`, `WorkerEvent::Failed`, and `WorkerEvent::Cancelled` explicitly, persisting terminal status transitions (status, completed_at, error) to the JobStore and releasing VRAM ledger reservations. The interim stopgap module (`interim_job_completion.rs`) and all its wiring in `backend/src/main.rs`, `crates/anvilml-worker/src/managed.rs`, and `crates/anvilml-worker/src/pool.rs` have been removed. Six new integration tests verify the terminal event handling, bringing the total test count to 17.

## Resolved Dependencies

None — no new dependencies introduced. All types and methods used exist in resolved versions:
| Type   | Name      | Version verified | MCP source   |
|--------|-----------|------------------|--------------|
| crate  | zeromq    | 0.6.0            | rust-docs MCP|
| crate  | rmp-serde | 1.3.1            | rust-docs MCP|

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/ledger.rs` | Added `get_reservation()` and `#[cfg(feature = "test-util")]` `reservations()` accessor methods. |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Added `get_reservation()`, `release_reservation()`, `update_job_terminal_status()` methods; added `#[cfg(feature = "test-util")]` test helpers (`ledger_reservations_test`, `reserve_vram_test`, `persist_job_test`). |
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Replaced `_` catch-all with explicit `Completed`, `Failed`, `Cancelled` arms that persist status and release VRAM; added `JobStatus` import; fixed `self.` → `scheduler.` references. |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Removed `interim_job_completion` module declaration and re-export. |
| Delete | `crates/anvilml-scheduler/src/interim_job_completion.rs` | Removed the interim stopgap module. |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bumped patch version 0.1.21 → 0.1.22. |
| Modify | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | Added 6 new integration tests; converted `JobScheduler` to `Arc<JobScheduler>` for post-event assertions; added test helper calls. |
| Modify | `backend/src/main.rs` | Removed `spawn_interim_job_completion_listener` import, channel construction, and listener spawn (interim-patch wiring). |
| Modify | `crates/anvilml-worker/src/managed.rs` | Removed `job_completion_tx` field, `set_job_completion_tx()` method, interim-patch send calls in `handle_event()`, and unused imports (`JobStatus`, `uuid::Uuid`). |
| Modify | `crates/anvilml-worker/src/pool.rs` | Removed `job_completion_tx` field, `set_job_completion_tx()` method, and propagation code in `spawn_all_impl()`; removed unused imports (`JobStatus`, `uuid::Uuid`). |
| Modify | `docs/TESTS.md` | Added 6 entries for new integration tests. |

## Commit Log

```
 backend/src/main.rs                    |  27 +------
 crates/anvilml-scheduler/Cargo.toml    |   2 +-
 crates/anvilml-scheduler/src/event_loop.rs  | 175 ++++++++++++++++++++++++++-
 crates/anvilml-scheduler/src/ledger.rs     |  24 +++++
 crates/anvilml-scheduler/src/lib.rs        |   3 -
 crates/anvilml-scheduler/src/scheduler.rs  |  91 +++++++++++++++
 .../src/interim_job_completion.rs          | 100 ----------------
 crates/anvilml-scheduler/tests/event_loop_tests.rs | 838 +++++++++++++++++++++++
 crates/anvilml-worker/src/managed.rs       |  51 +-------
 crates/anvilml-worker/src/pool.rs          |  46 +------
 docs/TESTS.md                            |  88 +++++++++
 11 files changed, 1247 insertions(+), 246 deletions(-)
```

## Test Results

```
running 17 tests
test test_image_ready_publishes_after_save ... ok
test test_map_cancelled ... ok
test test_map_completed ... ok
test test_map_failed ... ok
test test_image_ready_malformed_base64_errors ... ok
test test_map_progress ... ok
test test_image_ready_artifact_meta_fields_match ... ok
test test_image_ready_saves_artifact ... ok
test test_image_ready_empty_image_b64 ... ok
test test_spawn_event_loop_handles_recv_error ... ok
test test_spawn_event_loop_receives_and_publishes ... ok
test test_progress_still_published_via_map_worker_event ... ok
test test_terminal_event_unknown_job_logs_warning ... ok
test test_terminal_events_publish_ws_event ... ok
test test_failed_persists_status_error_and_releases_ledger ... ok
test test_cancelled_persists_status_and_releases_ledger ... ok
test test_completed_persists_status_and_releases_ledger ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:  Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.49s
# 2. Mock-hardware Windows: Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.39s
# 3. Real-hardware Linux:    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.50s
# 4. Real-hardware Windows:  Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.73s
All four checks exited 0.
```

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
Exit 0.
```

### Gate 2 — OpenAPI Drift
Not triggered — no handler function signatures, `#[utoipa::path]` annotations, or `ToSchema` derives were modified.

### Gate 3 — Node Parity
Not triggered — no node types added, removed, or renamed.

### Gate 4 — Mock/Real Parity Markers
Not triggered — no node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` functions were added or modified.

## Public API Delta

New `pub` items introduced:
| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `VramLedger::get_reservation` | `pub fn` | `ledger.rs` | Returns reservation amount for a device index. |
| `VramLedger::reservations` | `pub fn` | `ledger.rs` | Test-only accessor returning the reservations map. |
| `JobScheduler::get_reservation` | `pub(crate) async fn` | `scheduler.rs` | Look up current VRAM reservation for a device. |
| `JobScheduler::release_reservation` | `pub(crate) async fn` | `scheduler.rs` | Release VRAM reservation from the ledger. |
| `JobScheduler::update_job_terminal_status` | `pub async fn` | `scheduler.rs` | Update a job's terminal status in the database. |
| `JobScheduler::ledger_reservations_test` | `pub async fn` | `scheduler.rs` | Test-only: returns cloned reservations map. |
| `JobScheduler::reserve_vram_test` | `pub async fn` | `scheduler.rs` | Test-only: reserves VRAM on the ledger. |
| `JobScheduler::persist_job_test` | `pub async fn` | `scheduler.rs` | Test-only: persists a job to the database. |

Removed `pub` items:
| Item | Type | Module Path | Reason |
|------|------|-------------|--------|
| `spawn_interim_job_completion_listener` | `pub fn` | `lib.rs` re-export | Module deleted. |

## Deviations from Plan

- **`self.` → `scheduler.` in event_loop.rs**: The plan used `self.` references inside `spawn_event_loop()`, but this is a free function (not a method), so `self` is not available. Changed all `self.` references to `scheduler.` to match the actual function signature.
- **`reservations()` feature gate**: The plan used `#[cfg(test)]` for `VramLedger::reservations()`, but the scheduler's test helper is gated by `#[cfg(feature = "test-util")]`. Changed to `#[cfg(feature = "test-util")]` for consistency.
- **Test helper methods**: Added `reserve_vram_test()` and `persist_job_test()` as test-only helpers to avoid direct field access (`scheduler.ledger`, `scheduler.job_store`) in integration tests, since these fields are private.
- **`Arc<JobScheduler>` in tests**: Tests wrap `JobScheduler` in `Arc` before spawning the event loop (using `Arc::clone(&scheduler)`), enabling post-event assertions on the same scheduler instance.
- **`error.clone()` in Failed arm**: The `error` string from `WorkerEvent::Failed` is consumed by `update_job_terminal_status()`, so it is cloned before passing to the DB, and the original is used for the `WsEvent::JobFailed` broadcast.

## Blockers

None.
