# Implementation Report: P13-A5

| Field | Value |
|-------|-------|
| Task ID | P13-A5 |
| Phase | 013 — Dispatch & Execute |
| Description | anvilml-scheduler: handle worker Completed/Failed -> terminal status + idle |
| Implemented | 2026-06-09T14:00:00Z |
| Status | COMPLETE |

## Summary

Extended the dispatch loop in `anvilml-scheduler` to handle `WorkerEvent::Completed` and `WorkerEvent::Failed` events from workers. The dispatch loop now subscribes to the worker pool's event broadcast channel via `tokio::select!`, processes Completed/Failed events by re-reading the job from the database (to confirm it's still Running), updating the job's terminal status, setting the worker idle, broadcasting the appropriate WebSocket event, and waking the dispatch loop for the next queued job. The `update_status` function was extended to accept `completed_at`, `error`, and `worker_id` parameters, and the SQL WHERE clause was changed from `status != 'Running'` to `(status = 'Queued' OR status = 'Running')` to support both dispatch and event-handler transitions.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|-----------------|--------|
| (no new dependencies added) | | | |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add event subscription via `tokio::select!`, `handle_completed`/`handle_failed` functions, `event_discriminant` helper, `test_complete` integration test |
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Extend `update_status()` with `completed_at`, `error`, `worker_id` params; update SQL WHERE clause to `(status = 'Queued' OR status = 'Running')`; update existing test |
| Modify | `crates/anvilml-worker/src/pool.rs` | Add `publish_event()` test helper to `WorkerPool` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.12 → 0.1.13` |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Fix pre-existing unused import warning (remove `JobQueue` from test imports) |

## Commit Log

```
 .forge/reports/P13-A5_plan.md              | 334 +++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 Cargo.lock                                 |   2 +-
 crates/anvilml-scheduler/Cargo.toml        |   2 +-
 crates/anvilml-scheduler/src/job_store.rs  |  37 +++-
 crates/anvilml-scheduler/src/scheduler.rs  | 265 +++++++++++++++++++++--
 crates/anvilml-server/src/handlers/jobs.rs |   2 +-
 crates/anvilml-worker/src/pool.rs          |   8 +
 9 files changed, 632 insertions(+), 37 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-e7b735fd83a1dcdb)

running 38 tests
test dag::tests::test_duplicate_node_id ... ok
test dag::tests::test_cycle_detected_2node ... ok
test dag::tests::test_unknown_node_ref ... ok
test dag::tests::test_unknown_output_slot ... ok
test dag::tests::test_unknown_node_type ... ok
test dag::tests::test_valid_edge_references ... ok
test dag::tests::test_valid_graph ... ok
test dag::tests::test_valid_zit_5node_passes ... ok
test ledger::tests::test_free_mib_unknown_device ... ok
test ledger::tests::test_init_from ... ok
test ledger::tests::test_update ... ok
test ledger::tests::test_would_fit_true ... ok
test nodes::tests::test_all_nine_types_present ... ok
test ledger::tests::test_would_fit_false ... ok
test queue::tests::test_cancel_skipped_on_pop ... ok
test nodes::tests::test_zitsampler_outputs_include_latents_seed ... ok
test queue::tests::test_enqueue_pop_order ... ok
test job_store::tests::test_list_jobs_before_cursor ... ok
test job_store::tests::test_list_jobs_all ... ok
test job_store::tests::test_insert_and_get ... ok
test job_store::tests::test_list_jobs_limit ... ok
test job_store::tests::test_list_jobs_status_filter ... ok
test job_store::tests::test_update_status ... ok
test scheduler::tests::test_select_preference_idle ... ok
test scheduler::tests::test_complete ... ok
test scheduler::tests::test_submit_persists_settings ... ok
test scheduler::tests::test_submit_valid_job ... ok
test scheduler::tests::test_select_preference_not_found ... ok
test scheduler::tests::test_submit_broadcasts_event ... ok
test scheduler::tests::test_submit_invalid_graph ... ok
test scheduler::tests::test_select_preference_busy ... ok
test scheduler::tests::test_select_auto_all_busy ... ok
test scheduler::tests::test_dispatch_sends_execute ... ok
test scheduler::tests::test_select_auto_ranked_by_free_mib ... ok
test scheduler::tests::test_select_auto_single_idle ... ok
test scheduler::tests::test_select_auto_tie_break_device_index ... ok
test scheduler::tests::test_select_cpu ... ok
test scheduler::tests::test_select_cpu_not_available ... ok

test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.55s
```

Full workspace test suite: 268 passed, 0 failed, 0 ignored.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.79s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.41s

# 3. Real-hardware Linux check
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.48s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.29s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
  running 1 test
  test test_toml_key_set_matches_default ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **Added `worker_id` parameter to `update_status()`**: The plan only specified `completed_at` and `error` parameters. During implementation, I discovered that the dispatch loop was not persisting the job's `worker_id` in the database, which caused `handle_completed` to fail silently when looking up the worker to set idle. Added `worker_id: Option<String>` to the function signature and updated the SQL to set it via `COALESCE(worker_id, worker_id)` (idempotent — only sets on first dispatch).
- **Added `event_discriminant` helper locally in `scheduler.rs`**: The plan referenced `event_discriminant` from `anvilml-worker`, but it was not publicly exported. Defined a local copy to avoid modifying the worker crate's public API.
- **Fixed pre-existing clippy warning**: Removed unused `JobQueue` import in `crates/anvilml-server/src/handlers/jobs.rs` (FORGE_AGENT_RULES §9.3).
- **Used `status = ?job.status` instead of `status = %job.status`**: `JobStatus` does not implement `Display`; used `?` for debug formatting.

## Blockers

None.
