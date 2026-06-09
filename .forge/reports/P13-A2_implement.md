# Implementation Report: P13-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-A2                                            |
| Phase       | 013 — Dispatch & Execute                          |
| Description | anvilml-scheduler: select_worker (preference/auto/cpu) |
| Implemented | 2026-06-09T10:30:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented the `select_worker` function in `crates/anvilml-scheduler/src/scheduler.rs` that implements GPU/device selection with three modes: force-CPU, user-specified device preference, and auto (ranked by free VRAM). Added 9 unit tests covering all modes and edge cases. Bumped `anvilml-scheduler` crate version from 0.1.10 to 0.1.11.

## Resolved Dependencies

No new dependencies were added. The function uses only existing types: `Job`, `JobSettings`, `WorkerInfo`, `WorkerStatus`, and `VramLedger` from `anvilml-core` and `anvilml-scheduler`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Added `select_worker` function (42 lines) + 9 unit tests (~382 lines) |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.10 → 0.1.11` |

## Commit Log

```
 Cargo.lock                                |   2 +-
 crates/anvilml-scheduler/Cargo.toml       |   2 +-
 crates/anvilml-scheduler/src/scheduler.rs | 424 ++++++++++++++++++++++++++++++
 3 files changed, 426 insertions(+), 2 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-bd267adb703026f5)

running 36 tests
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
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok
test nodes::tests::test_all_nine_types_present ... ok
test queue::tests::test_cancel_skipped_on_pop ... ok
test queue::tests::test_enqueue_pop_order ... ok
test job_store::tests::test_list_jobs_before_cursor ... ok
test job_store::tests::test_list_jobs_all ... ok
test job_store::tests::test_insert_and_get ... ok
test job_store::tests::test_list_jobs_limit ... ok
test job_store::tests::test_list_jobs_status_filter ... ok
test job_store::tests::test_update_status ... ok
test scheduler::tests::test_select_auto_all_busy ... ok
test scheduler::tests::test_select_auto_ranked_by_free_mib ... ok
test scheduler::tests::test_select_auto_single_idle ... ok
test scheduler::tests::test_select_auto_tie_break_device_index ... ok
test scheduler::tests::test_select_cpu ... ok
test scheduler::tests::test_select_cpu_not_available ... ok
test scheduler::tests::test_select_preference_busy ... ok
test scheduler::tests::test_select_preference_idle ... ok
test scheduler::tests::test_select_preference_not_found ... ok
test scheduler::tests::test_submit_valid_job ... ok
test scheduler::tests::test_submit_persists_settings ... ok
test scheduler::tests::test_submit_broadcasts_event ... ok
test scheduler::tests::test_submit_invalid_graph ... ok

test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

All 9 new `select_worker` tests pass:
- `test_select_preference_idle` — device_preference Some(0) returns index 0
- `test_select_preference_busy` — device_preference Some(0) returns None when busy
- `test_select_preference_not_found` — device_preference Some(99) returns None
- `test_select_auto_single_idle` — auto picks the only idle worker
- `test_select_auto_ranked_by_free_mib` — auto picks highest free_mib worker
- `test_select_auto_tie_break_device_index` — auto breaks ties by device_index asc
- `test_select_auto_all_busy` — auto returns None when all busy
- `test_select_cpu` — force-CPU picks the CPU worker
- `test_select_cpu_not_available` — force-CPU returns None when no CPU worker

## Format Gate

```
(exit 0 — no output, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.24s

# 2. Mock-hardware Windows cross-check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.55s

# 3. Real-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.04s

# 4. Real-hardware Windows cross-check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.39s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
     Running tests/config_reference.rs (target/debug/deps/config_reference-b5bef115a626b82d)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- Added `use crate::ledger::VramLedger;` import to `scheduler.rs` (required because `select_worker` references `VramLedger` in its signature and is a free-standing function, not a method). This was necessary to make the code compile — the plan did not explicitly list this import but it is required by the implementation.

## Blockers

None.
