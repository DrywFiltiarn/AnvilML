# Implementation Report: P17-A1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P17-A1                          |
| Phase         | 17 — Cancellation               |
| Description   | anvilml-scheduler: JobScheduler::cancel() dispatches by current status |
| Implemented   | 2026-07-11T00:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Extended `JobScheduler::cancel()` in `crates/anvilml-scheduler/src/scheduler.rs` from a thin queue-only delegate to a status-aware dispatcher that handles four cases: (1) Queued jobs get cancelled via `queue.cancel()` plus immediate database status update to `Cancelled`, returning `Ok(true)`; (2) Running jobs return `Ok(true)` without sending IPC (the actual `WorkerMessage::CancelJob` send is stubbed with a `// defers_to: P17-A2` comment); (3) terminal states (Completed/Failed/Cancelled) return `Ok(false)` as a no-op; (4) unknown IDs return `Ok(false)`. Added four new integration tests (the fifth, `test_cancel_unknown_id_returns_false`, already existed from a prior session) and one additional test for already-cancelled queued jobs. Bumped the crate version from 0.1.25 to 0.1.26. All 28 scheduler tests pass, full workspace test suite exits 0, clippy zero warnings, all four platform cross-checks clean.

## Resolved Dependencies

None. This task modifies existing code in the `anvilml-scheduler` crate and uses only already-declared dependencies. No new crates or versions are introduced.

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| (none) | —         | —                | —              |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Extended `cancel()` with status-aware branching (Queued/Running/Queued-no-queue/terminal/unknown) |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Added 4 new integration tests for cancel status branches |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.25 → 0.1.26 |
| Modify | `docs/TESTS.md` | Added entries for 4 new cancel tests |

## Commit Log

```
 .forge/reports/P17-A1_plan.md                     | 224 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  11 +-
 Cargo.lock                                        |   2 +-
 crates/anvilml-scheduler/Cargo.toml               |   2 +-
 crates/anvilml-scheduler/src/scheduler.rs         | 120 +++++++++--
 crates/anvilml-scheduler/tests/scheduler_tests.rs | 232 ++++++++++++++++++++++
 docs/TESTS.md                                     |  48 +++++
 8 files changed, 615 insertions(+), 30 deletions(-)
```

## Test Results

```
     Running tests/scheduler_tests.rs (target/debug/deps/scheduler_tests-4710ec7ea9509f93)

running 28 tests
test test_cancel_queued_job_sets_cancelled_status ... ok
test test_cancel_running_job_returns_true_no_ipc ... ok
test test_cancel_queued_job_returns_true ... ok
test test_cancel_unknown_id_returns_false ... ok
test test_cancel_already_cancelled_queued_job_returns_false ... ok
test test_busy_worker_excluded_from_ranking ... ok
test test_cancel_terminal_job_returns_false ... ok
test test_dispatch_one_returns_false_when_no_idle ... ok
test test_dispatch_one_no_op_without_idle ... ok
test test_submit_empty_registry_returns_workers_unavailable ... ok
test test_submit_invalid_graph_returns_validation_error ... ok
test test_submit_valid_persists_and_queues ... ok
test test_get_job_unknown_id_returns_none ... ok
test test_get_job_returns_persisted_job ... ok
test test_dispatch_one_test_wrapper_collapses_failed_to_false ... ok
test test_dispatch_one_no_transition_without_idle ... ok
test test_dispatch_one_reverts_worker_idle_after_send_failure ... ok
test test_two_submits_get_distinct_ids ... ok
test test_ranking_selection_deterministic_and_workers_end_idle ... ok
test test_dispatch_loop_returns_join_handle ... ok
test test_dispatch_one_dispatched_via_real_dealer_peer ... ok
test test_device_preference_wins_over_vram_ranking ... ok
test test_device_preference_none_falls_back_to_vram_ranking ... ok
test test_submit_wakes_dispatch_loop ... ok
test test_vram_ranking_picks_highest_free_idle ... ok
test test_multiple_queued_jobs_get_distinct_workers ... ok
test test_no_idle_workers_leaves_job_queued ... ok
test test_dispatch_loop_survives_multiple_wakes ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
```

All 28 scheduler tests pass (4 new + 24 pre-existing). Full workspace test suite: 368 tests, 0 failures.

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0, no drift detected)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.68s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 56.35s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 58.84s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 01s
```

All four cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
    Running tests/config_reference.rs
    running 1 test
    test tests::config_reference_matches_defaults ... ok
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Gate 1 passes. No config fields were added/renamed/removed by this task.

## Public API Delta

```
(git diff HEAD -- crates/anvilml-scheduler/src/scheduler.rs | grep '^+.*pub ')
(no output)
```

No new `pub` items introduced. The existing `pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>` signature remains unchanged — only its internal behavior was extended.

## Deviations from Plan

1. **Five planned tests, four actually new**: The plan called for 5 new tests. `test_cancel_unknown_id_returns_false` already existed in the test file from a prior session, so only 4 new test functions were added. The existing test was verified to work correctly with the new `cancel()` implementation (it tests unknown-ID → `Ok(false)`, which the new implementation handles identically to the old).

2. **Added `JobStatus::Queued` arm in the DB query branch**: The approved plan's approach only showed three match arms (Running, terminal states, None). However, `JobStatus::Queued` is a legitimate case that can occur when a job was popped from the in-memory queue by the dispatch loop but not yet dispatched — in that case, the DB query finds a `Queued`-status job that wasn't in the queue. This arm updates the DB status to `Cancelled` and returns `Ok(true)`, preserving the idempotent-cancel semantics.

3. **`defers_to: P17-A2` comment**: Added the required `// defers_to: P17-A2 — IPC send of WorkerMessage::CancelJob` marker comment at the Running branch stub site, as mandated by `FORGE_AGENT_RULES.md §9.7` and the task's `defers_to` field.

## Blockers

None.
