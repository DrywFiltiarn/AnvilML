# Implementation Report: P14-A5

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P14-A5                                            |
| Phase         | 14 — Dispatch & Execute                           |
| Description   | anvilml-scheduler: dispatch_one marks the assigned worker Busy |
| Implemented   | 2026-07-07T18:15:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Completed implementation of P14-A5: extended `JobScheduler::dispatch_one()` to mark the selected worker's status as `Busy` immediately after worker selection and before VRAM reservation. Added a `#[cfg(feature = "test-utils")]` helper `set_up_test_workers()` to `WorkerPool` so integration tests can inject mock handles with controllable status. Added four new integration tests verifying Busy status assignment, deduplication across jobs, Busy exclusion from ranking, and Busy persistence on dispatch failure. Updated the `dispatch_one()` doc comment to remove the "deferred to P14-A5" note. Bumped `anvilml-scheduler` patch version from 0.1.13 to 0.1.14. All 23 scheduler tests pass (19 pre-existing + 4 new), full workspace test suite exits 0 with 267 tests passing.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | anvilml-worker | 0.1.28 (dev-dep, path) | Cargo.lock (workspace path dep, no version pin) |

No new external dependencies introduced. The `anvilml-worker` dev-dependency with `test-utils` feature is a workspace path dependency already declared as a regular dependency of `anvilml-scheduler`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Added `selected.set_status(WorkerStatus::Busy).await` call with DEBUG log after worker selection; updated doc comment to remove "deferred to P14-A5" sentence |
| Modify | `crates/anvilml-worker/src/pool.rs` | Added `set_up_test_workers()` method (test-utils gated) |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Added 4 new integration tests for Busy status transitions |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Added `anvilml-worker` dev-dependency with `test-utils` feature; bumped version 0.1.13 → 0.1.14 |
| Modify | `docs/TESTS.md` | Added 4 entries for new tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 Cargo.lock                                        |   2 +-
 crates/anvilml-scheduler/Cargo.toml               |   6 +-
 crates/anvilml-scheduler/src/scheduler.rs         |  25 +-
 crates/anvilml-scheduler/tests/scheduler_tests.rs | 423 ++++++++++++++++++++++
 crates/anvilml-worker/src/pool.rs                 |  25 ++
 docs/TESTS.md                                     |  48 +++
 8 files changed, 530 insertions(+), 18 deletions(-)
```

## Test Results

```
     Running tests/scheduler_tests.rs (target/debug/deps/scheduler_tests-17b5c24ec9d68101)

running 23 tests
test test_cancel_unknown_id_returns_false ... ok
test test_cancel_queued_job_returns_true ... ok
test test_dispatch_one_no_op_without_idle ... ok
test test_dispatch_one_no_transition_without_idle ... ok
test test_busy_worker_excluded_from_ranking ... ok
test test_dispatch_one_marks_worker_busy ... ok
test test_dispatch_one_busy_worker_excluded_from_next_job ... ok
test test_get_job_returns_persisted_job ... ok
test test_dispatch_one_returns_false_when_no_idle ... ok
test test_dispatch_one_status_busy_survives_vram_failure ... ok
test test_submit_invalid_graph_returns_validation_error ... ok
test test_submit_empty_registry_returns_workers_unavailable ... ok
test test_submit_valid_persists_and_queues ... ok
test test_get_job_unknown_id_returns_none ... ok
test test_two_submits_get_distinct_ids ... ok
test test_dispatch_loop_returns_join_handle ... ok
test test_device_preference_wins_over_vram_ranking ... ok
test test_device_preference_none_falls_back_to_vram_ranking ... ok
test test_submit_wakes_dispatch_loop ... ok
test test_no_idle_workers_leaves_job_queued ... ok
test test_vram_ranking_picks_highest_free_idle ... ok
test test_multiple_queued_jobs_get_distinct_workers ... ok
test test_dispatch_loop_survives_multiple_wakes ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 23 scheduler tests pass, including the 4 new tests:
- `test_dispatch_one_marks_worker_busy` — verifies Busy status after dispatch
- `test_dispatch_one_busy_worker_excluded_from_next_job` — verifies no worker dispatched twice
- `test_busy_worker_excluded_from_ranking` — verifies Busy worker excluded from idle list
- `test_dispatch_one_status_busy_survives_vram_failure` — verifies Busy persists on dispatch failure

Full workspace test suite: 267 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no formatting drift)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
--- CHECK 1: PASS ---

# Check 2: Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.18s
--- CHECK 2: PASS ---

# Check 3: Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.84s
--- CHECK 3: PASS ---

# Check 4: Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.66s
--- CHECK 4: PASS ---
```

All four platform cross-checks passed.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. Gate 2 (OpenAPI drift) not triggered — no handler signatures changed. Gate 3 (Node parity) not triggered — no node types modified. Gate 4 (Mock/Real parity markers) not triggered — no node `execute()` or arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` modified.

## Public API Delta

```
+    pub fn set_up_test_workers(&mut self, workers: Vec<(WorkerHandle, GpuDevice)>) {
```

One new `pub` item introduced: `WorkerPool::set_up_test_workers()` in `crates/anvilml-worker/src/pool.rs`. This is gated by `#[cfg(feature = "test-utils")]` and is only available during test builds (enabled via the `anvilml-worker` dev-dependency in `anvilml-scheduler`'s Cargo.toml). No other public signatures changed.

## Deviations from Plan

None. All implementation matches the approved plan exactly:
- `set_up_test_workers()` added to `WorkerPool` with `#[cfg(feature = "test-utils")]` gate
- `dispatch_one()` extended with `set_status(WorkerStatus::Busy)` call at the correct insertion point (after worker_id/device_index extraction, before VRAM reservation)
- DEBUG log `dispatch_one_worker_marked_busy` added
- Doc comment updated to remove the "deferred to P14-A5" sentence
- Four new tests added and all passing
- Cargo.toml updated with dev-dependency and version bump

The implementation required two minor adjustments during coding:
1. `Mutex::new::<Option<JoinHandle<()>>>(None)` was replaced with a type annotation on the binding (`let join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None))`) because `tokio::sync::Mutex::new()` does not accept generic arguments — the type is inferred from the binding annotation.
2. The `WorkerHandle` was cloned before passing to `set_up_test_workers()` (creating a `handle_for_read` binding) because the handle is moved into the pool and the original is needed to read the status after dispatch.

## Blockers

None.
