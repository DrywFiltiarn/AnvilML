# Implementation Report: P15-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P15-A1                                            |
| Phase       | 015 — Live Job Events                             |
| Description | anvilml-scheduler: emit JobProgress events from worker Progress |
| Implemented | 2026-06-10T10:30:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Wired the missing `WorkerEvent::Progress` and `WorkerEvent::Cancelled` handlers into the scheduler's dispatch loop so that progress updates and cancellation events from the Python worker are translated into `WsEvent::JobProgress` and `WsEvent::JobCancelled` WebSocket events and broadcast to all subscribers. Added a `handle_cancelled()` async helper following the existing `handle_completed()` and `handle_failed()` patterns. Added two unit tests verifying the broadcast behavior. All five job-lifecycle event types are now confirmed wired through the broadcaster.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| (none) | —         | —                | —              |

No new dependencies added. All types (`JobCancelledEvent`, `JobProgressEvent`, `WsEvent`) already existed in `anvilml-core`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `Progress` and `Cancelled` arms in dispatch loop match block; add `handle_cancelled()` helper; add two unit tests; update catch-all comment; add `JobCancelledEvent` and `JobProgressEvent` to imports |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.15 → 0.1.16` |

## Commit Log

```
 .forge/reports/P15-A1_plan.md             | 240 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  11 +-
 Cargo.lock                                |   2 +-
 crates/anvilml-scheduler/Cargo.toml       |   2 +-
 crates/anvilml-scheduler/src/scheduler.rs | 269 +++++++++++++++++++++++++++++-
 6 files changed, 518 insertions(+), 12 deletions(-)
```

## Test Results

```
running 41 tests
test dag::tests::test_duplicate_node_id ... ok
test dag::tests::test_cycle_detected_2node ... ok
test dag::tests::test_unknown_node_ref ... ok
test dag::tests::test_unknown_node_type ... ok
test dag::tests::test_unknown_output_slot ... ok
test dag::tests::test_valid_edge_references ... ok
test dag::tests::test_valid_graph ... ok
test dag::tests::test_valid_zit_5node_passes ... ok
test ledger::tests::test_free_mib_unknown_device ... ok
test ledger::tests::test_init_from ... ok
test ledger::tests::test_update ... ok
test ledger::tests::test_would_fit_false ... ok
test ledger::tests::test_would_fit_true ... ok
test nodes::tests::test_all_nine_types_present ... ok
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok
test queue::tests::test_cancel_skipped_on_pop ... ok
test queue::tests::test_enqueue_pop_order ... ok
test job_store::tests::test_list_jobs_before_cursor ... ok
test job_store::tests::test_list_jobs_all ... ok
test job_store::tests::test_insert_and_get ... ok
test job_store::tests::test_list_jobs_limit ... ok
test job_store::tests::test_list_jobs_status_filter ... ok
test job_store::tests::test_update_status ... ok
test scheduler::tests::test_select_auto_tie_break_device_index ... ok
test scheduler::tests::test_select_cpu_not_available ... ok
test scheduler::tests::test_cancel_broadcasts_event ... ok
test scheduler::tests::test_submit_valid_job ... ok
test scheduler::tests::test_complete ... ok
test scheduler::tests::test_dispatch_sends_execute ... ok
test scheduler::tests::test_image_ready_broadcasts_event ... ok
test scheduler::tests::test_select_auto_all_busy ... ok
test scheduler::tests::test_progress_broadcasts_event ... ok
test scheduler::tests::test_select_auto_ranked_by_free_mib ... ok
test scheduler::tests::test_select_auto_single_idle ... ok
test scheduler::tests::test_select_preference_idle ... ok
test scheduler::tests::test_select_preference_not_found ... ok
test scheduler::tests::test_select_cpu ... ok
test scheduler::tests::test_select_preference_busy ... ok
test scheduler::tests::test_submit_persists_settings ... ok
test scheduler::tests::test_submit_broadcasts_event ... ok
test scheduler::tests::test_submit_invalid_graph ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.57s
```

Full workspace test suite: 244 tests passed, 0 failed.

## Format Gate

```
(No output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.09s

# 2. Mock-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.88s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.09s

# 4. Real-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.46s
```

All four cross-checks exited 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **WorkerEvent::Cancelled field name**: The plan specified `WorkerEvent::Cancelled { job_id, reason: _ }` but the actual IPC definition in `anvilml-ipc/src/messages.rs` is `Cancelled { job_id: Uuid }` (no `reason` field). Implementation uses `WorkerEvent::Cancelled { job_id }` matching the actual definition.
- **Drain loop fix**: The existing `test_image_ready_broadcasts_event` test has the same drain-loop pattern (break on target event, then receive again). This pattern works for that test because `handle_image_ready` does not broadcast additional events. For the new Progress and Cancelled handlers, the drain loop was fixed to capture the matched event (`let jpe = loop { ... break e; }`) to avoid a second blocking `rx.recv()` call.

## Blockers

None.
