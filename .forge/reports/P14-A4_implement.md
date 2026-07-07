# Implementation Report: P14-A4

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P14-A4                             |
| Phase         | 14 — Dispatch & Execute            |
| Description   | anvilml-scheduler: worker selection algorithm, real dispatch |
| Implemented   | 2026-07-07T17:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Replaced `dispatch_one()`'s always-false stub with the full two-step worker selection algorithm from `ANVILML_DESIGN.md §12.5`: first check if a job's `device_preference` matches an `Idle` worker, otherwise rank all `Idle` workers by `vram_free_mib` descending and pick the top candidate. On a successful match, reserves VRAM via the ledger, transitions the job to `Running`, persists the updated job to the database, sends `WorkerMessage::Execute` to the selected worker, and returns `true`. Added a `devices` field and `devices()` accessor to `WorkerPool` so the scheduler can access each worker's device metadata for selection decisions. Added 8 new integration tests (≥6 required). Bumped `anvilml-worker` patch version from `0.1.27` to `0.1.28` and `anvilml-scheduler` from `0.1.12` to `0.1.13`.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source         |
|--------|-------------------|------------------|----------------|
| crate  | anvilml-ipc       | local path dep   | local code     |

No new external crates were added. The only new dependency in `Cargo.toml` is the path dependency `anvilml-ipc` on `anvilml-scheduler`, which was already present in the workspace.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/pool.rs` | Added `devices: Vec<GpuDevice>` field, populate in `spawn_all_impl()`, added `devices()` accessor method |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Added `anvilml-ipc` path dependency; bumped version 0.1.12 → 0.1.13 |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Replaced `dispatch_one()` stub with full 2-step selection algorithm; added `dispatch_one_test()` test helper |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Added 8 new tests for worker selection algorithm |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bumped version 0.1.27 → 0.1.28 |
| Modify | `docs/TESTS.md` | Added 8 new test catalogue entries |

## Commit Log

```
 .forge/reports/P14-A4_plan.md                     | 421 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   4 +-
 .forge/state/state.json                           |  11 +-
 Cargo.lock                                        |   5 +-
 crates/anvilml-scheduler/Cargo.toml               |   3 +-
 crates/anvilml-scheduler/src/scheduler.rs         | 244 ++++++++++-
 crates/anvilml-scheduler/tests/scheduler_tests.rs | 467 ++++++++++++++++++++++
 crates/anvilml-worker/Cargo.toml                  |   2 +-
 crates/anvilml-worker/src/pool.rs                 |  24 ++
 docs/TESTS.md                                     |  96 +++++
 10 files changed, 1252 insertions(+), 25 deletions(-)
```

## Test Results

```
     Running tests/scheduler_tests.rs (target/debug/deps/scheduler_tests-00404e15ddef743d)

running 19 tests
test test_cancel_unknown_id_returns_false ... ok
test test_cancel_queued_job_returns_true ... ok
test test_dispatch_one_no_op_without_idle ... ok
test test_submit_empty_registry_returns_workers_unavailable ... ok
test test_dispatch_one_returns_false_when_no_idle ... ok
test test_get_job_unknown_id_returns_none ... ok
test test_get_job_returns_persisted_job ... ok
test test_dispatch_one_no_transition_without_idle ... ok
test test_submit_invalid_graph_returns_validation_error ... ok
test test_submit_valid_persists_and_queues ... ok
test test_two_submits_get_distinct_ids ... ok
test test_dispatch_loop_returns_join_handle ... ok
test test_device_preference_wins_over_vram_ranking ... ok
test test_device_preference_none_falls_back_to_vram_ranking ... ok
test test_no_idle_workers_leaves_job_queued ... ok
test test_multiple_queued_jobs_get_distinct_workers ... ok
test test_submit_wakes_dispatch_loop ... ok
test test_vram_ranking_picks_highest_free_idle ... ok
test test_dispatch_loop_survives_multiple_wakes ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 287 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
=== Check 1 passed ===
cargo check --workspace --features mock-hardware — Finished (dev profile)

=== Check 2 passed ===
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu — Finished (dev profile)

=== Check 3 passed ===
cargo check --bin anvilml — Finished (dev profile)

=== Check 4 passed ===
cargo check --bin anvilml --target x86_64-pc-windows-gnu — Finished (dev profile)
```

All four platform cross-checks passed.

## Project Gates

```
Gate 1 — Config Surface Sync:
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 2 — OpenAPI Drift: Not triggered (this task does not modify handler signatures, utoipa annotations, or AppState fields).

## Public API Delta

```
+    pub async fn dispatch_one_test(&self, job: &Job, workers: &anvilml_worker::WorkerPool) -> bool {
+    pub fn devices(&self) -> &[GpuDevice] {
```

Two new `pub` items introduced:
1. `devices(&self) -> &[GpuDevice]` on `WorkerPool` — returns device metadata for dispatch decisions.
2. `dispatch_one_test(&self, job: &Job, workers: &anvilml_worker::WorkerPool) -> bool` on `JobScheduler` — test-util gated, exposes the private `dispatch_one()` for integration tests.

Both match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

- **Version bump adjustment:** The plan specified bumping `anvilml-worker` from `0.1.6` to `0.1.7`, but the actual version was `0.1.27`. Bumped to `0.1.28` (current + 1 patch). The `anvilml-scheduler` bump from `0.1.12` to `0.1.13` matched the plan exactly.
- **`CapabilitySource::Mock` does not exist:** The `CapabilitySource` enum only has `PyTorch`, `DeviceTable`, and `Fallback` variants (no `Mock`). Changed test fixtures to use `CapabilitySource::DeviceTable` instead.
- **`anvilml-ipc` added as a dependency:** The plan did not explicitly list adding `anvilml-ipc` to `anvilml-scheduler/Cargo.toml`, but it is required because `dispatch_one()` constructs and sends `WorkerMessage::Execute` via `workers.transport().send()`.

## Blockers

None.
