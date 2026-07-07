# Plan Report: P14-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P14-A5                                            |
| Phase       | 14 — Dispatch & Execute                           |
| Description | anvilml-scheduler: dispatch_one marks the assigned worker Busy |
| Depends on  | P14-A4                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-07T17:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Close the worker-status gap left by P14-A4: after `dispatch_one()` selects a worker and before reserving VRAM, call `WorkerHandle::set_status(WorkerStatus::Busy)` so that the worker's own status reflects the assignment immediately. Without this, a second concurrent dispatch cycle in the same wake could select the same worker again. The acceptance is four new tests verifying Busy status assignment, deduplication across jobs in one wake, Busy exclusion from ranking, and the full scheduler test suite exiting 0 with ≥21 tests.

## Scope

### In Scope
- Extend `dispatch_one()` in `crates/anvilml-scheduler/src/scheduler.rs` to call `selected.set_status(WorkerStatus::Busy)` immediately after worker selection and before VRAM reservation.
- Add a `#[cfg(feature = "test-utils")]` method `set_up_test_workers()` to `WorkerPool` in `crates/anvilml-worker/src/pool.rs` so integration tests can inject mock handles with controllable status.
- Add four new tests in `crates/anvilml-scheduler/tests/scheduler_tests.rs`.
- Update the `dispatch_one()` doc comment to remove the "Does NOT mark..." sentence and note that the Busy transition is now performed in this task.
- Add a DEBUG log call for the Busy status transition.
- Bump `anvilml-scheduler` patch version (0.1.13 → 0.1.14).
- Add `anvilml-worker` with `test-utils` feature to `anvilml-scheduler` dev-dependencies.

### Out of Scope
None. `defers_to` is empty (`[]` from JSON). All functionality described in the task context is implemented in full. Idle restoration on completion and dispatch-loop wake on that transition are separate later-phase concerns, explicitly scoped out by the task description itself (not deferred to a `defers_to` target).

## Existing Codebase Assessment

**What already exists:** `JobScheduler::dispatch_one()` (scheduler.rs, lines 278–504) implements the full two-step worker selection algorithm (device preference match → VRAM ranking fallback). After selection, it performs four steps: VRAM reservation, job status transition to Running, database persistence, and `WorkerMessage::Execute` send. The `WorkerHandle::set_status(&self, WorkerStatus)` method exists in `managed.rs` (line 230) as a public async method that acquires a write lock on the shared `Arc<RwLock<WorkerStatus>>`. The `WorkerStatus::Busy` variant exists in `anvilml-core/src/types/worker.rs` (line 38). The `test-util` feature already gates `dispatch_one_test()` in scheduler.rs (line 587). The `test-utils` feature already gates `spawn_all_with_spawner()` in the worker crate.

**Established patterns:** Tests in `scheduler_tests.rs` use `#[tokio::test]`, create fresh `JobStore` via `create_job_store()`, use `make_registry()` for a PassThrough node, and follow the pattern of asserting on job status after dispatch attempts. The `dispatch_one_test()` wrapper (test-util-gated) is the established pattern for exposing private methods to integration tests. The worker crate's `test-utils` feature gates test-only public methods.

**Gap between design doc and source:** The `dispatch_one()` doc comment (line 295–296) explicitly says "Does NOT mark the selected worker's status as `Busy` — that transition is deferred to P14-A5." This is the exact gap this task fills. The source is otherwise complete and consistent with the design.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| crate  | serial_test | 3.5.0         | Cargo.toml (lockfile) | n/a (dev-dependency, already present in other crates) |

No new external dependencies are introduced. `serial_test` is already declared in `anvilml-core`, `anvilml-hardware`, and `backend` dev-dependencies. The `anvilml-worker` `test-utils` feature and `anvilml-scheduler` `test-util` feature are existing workspace features.

## Approach

1. **Add `set_up_test_workers()` to `WorkerPool`** in `crates/anvilml-worker/src/pool.rs`.
   - Gate with `#[cfg(feature = "test-utils")]`.
   - Signature: `pub fn set_up_test_workers(&mut self, workers: Vec<(WorkerHandle, GpuDevice)>)`
   - Implementation: iterate the input, push `handle` into `self.handles`, push `device` into `self.devices`. This keeps handles and devices in sync (index `i` maps to device `i`), matching the invariant established in `spawn_all_impl()`.
   - Rationale: Integration tests in `tests/` compile as a separate crate and cannot access `pub(crate)` items. The `test-utils` feature is the established pattern (used by `spawn_all_with_spawner()`).

2. **Extend `dispatch_one()` in `scheduler.rs`** to mark the selected worker Busy.
   - Insert `selected.set_status(WorkerStatus::Busy).await;` immediately after extracting `worker_id` and `device_index` (line 418 area), and before the VRAM reservation block (line 426).
   - This is the correct insertion point: the worker is definitively selected at this point, and setting Busy before VRAM reservation ensures that if VRAM reservation fails and the dispatch returns false, the worker is still marked Busy (the status will be corrected later by the idle-restoration path in a future task).
   - Add a DEBUG log: `tracing::debug!(worker_id = %worker_id, "dispatch_one_worker_marked_busy");`
   - Rationale: `selected` is a clone of the handle from `idle_workers`, which itself is a clone of the handle from `workers.handles()`. All clones share the same `Arc<RwLock<WorkerStatus>>`, so `set_status` on the clone updates the shared lock that the pool's original handle also reads from.

3. **Update `dispatch_one()` doc comment.**
   - Remove the sentence: "Does NOT mark the selected worker's status as `Busy` — that transition is deferred to P14-A5."
   - Add a note in the "On a successful match" paragraph: "Sets the worker's status to `Busy` immediately (P14-A5)."

4. **Add four new tests** in `crates/anvilml-scheduler/tests/scheduler_tests.rs`.
   - Test 1: `test_dispatch_one_marks_worker_busy` — creates a mock idle worker via `set_up_test_workers`, calls `dispatch_one_test()`, then reads the pool's handle status and asserts it is `Busy`.
   - Test 2: `test_dispatch_one_busy_worker_excluded_from_next_job` — creates two mock idle workers, dispatches two jobs in sequence, verifies each job goes to a distinct worker, and verifies both workers are now `Busy` (no worker was dispatched twice).
   - Test 3: `test_busy_worker_excluded_from_ranking` — creates three workers: one Idle with low VRAM, one Idle with high VRAM, one Busy. Dispatches one job. Verifies the job goes to the high-VRAM Idle worker (the Busy one is excluded from the idle list).
   - Test 4: `test_dispatch_one_status_busy_survives_vram_failure` — creates a mock idle worker, calls `dispatch_one_test()` with a non-existent device index to trigger VRAM reservation path. Verifies the worker status is still `Busy` even if the dispatch returns false (edge case: the status transition happens before VRAM reservation, so it persists regardless).

5. **Update `Cargo.toml`** for `anvilml-scheduler`.
   - Add `anvilml-worker = { path = "../anvilml-worker", features = ["test-utils"] }` to `[dev-dependencies]`.
   - Bump version from `0.1.13` to `0.1.14`.

## Public API Surface

No new `pub` items are added to any crate's public API. The `set_up_test_workers()` method in `WorkerPool` is gated by `#[cfg(feature = "test-utils")]` and is only available during test builds. The internal behavior change to `dispatch_one()` does not alter any public method signatures.

| Item | Location | Change |
|------|----------|--------|
| `WorkerPool::set_up_test_workers()` | `crates/anvilml-worker/src/pool.rs` | NEW (test-utils gated) |
| `JobScheduler::dispatch_one()` | `crates/anvilml-scheduler/src/scheduler.rs` | INTERNAL change: adds `set_status` call; no signature change |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `set_status(WorkerStatus::Busy)` call in `dispatch_one()`, add DEBUG log, update doc comment |
| Modify | `crates/anvilml-worker/src/pool.rs` | Add `set_up_test_workers()` method (test-utils gated) |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add 4 new integration tests |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add `anvilml-worker` dev-dependency with `test-utils` feature; bump patch version 0.1.13 → 0.1.14 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_dispatch_one_marks_worker_busy` | A dispatched worker's status reads `Busy` immediately after `dispatch_one()` returns true. | One mock idle worker set up via `set_up_test_workers()`. A valid Job from `submit()`. | Single job dispatched to one idle worker. | `handle.status().await == WorkerStatus::Busy` after dispatch returns true. | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_dispatch_one_marks_worker_busy` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_dispatch_one_busy_worker_excluded_from_next_job` | Two jobs in the same wake cycle never select the same worker twice. | Two mock idle workers with different VRAM. | Two sequential `dispatch_one_test()` calls. | Both workers have status `Busy`; each job went to a different worker. | Same command, filter `test_dispatch_one_busy_worker_excluded_from_next_job` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_busy_worker_excluded_from_ranking` | A `Busy` worker is excluded from the next cycle's ranking. | Three workers: two Idle (different VRAM), one Busy. | One job with `device_preference = None`. | Job dispatched to the Idle worker with most VRAM; Busy worker untouched. | Same command, filter `test_busy_worker_excluded_from_ranking` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_dispatch_one_status_busy_on_failed_dispatch` | Worker is marked Busy even when dispatch returns false (e.g. VRAM reservation path). | One mock idle worker. | Job dispatched; dispatch_one returns false (e.g. transport failure). | `handle.status().await == WorkerStatus::Busy` despite false return. | Same command, filter `test_dispatch_one_status_busy_on_failed_dispatch` exits 0 |

## CI Impact

No CI changes required. The new tests are part of the existing `scheduler_tests.rs` file, which is already collected by `cargo test --workspace --features mock-hardware`. The `anvilml-worker` dev-dependency addition in `anvilml-scheduler/Cargo.toml` is only active during test builds, so it has no effect on release binaries.

## Platform Considerations

None identified. The change is platform-neutral: `WorkerHandle::set_status()` uses `tokio::sync::RwLock` which is cross-platform, and `WorkerStatus::Busy` is a simple enum variant with no platform-specific behavior. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `set_status()` on a cloned handle may not propagate to the pool's original handles. | Low | High | Verified by code inspection: all `WorkerHandle` clones share the same `Arc<RwLock<WorkerStatus>>` (Clone impl at managed.rs line 167–180 copies `status: Arc::clone(&self.status)`). The clone's `set_status()` writes to the same lock. |
| Test workers with controllable status require invasive changes to `WorkerPool`. | Low | Medium | The `test-utils` feature is already established for test-only public methods. The `set_up_test_workers()` method is minimal: two Vec pushes per entry. No logic changes to production code paths. |
| The `anvilml-worker` dev-dependency in `anvilml-scheduler/Cargo.toml` may introduce transitive dependency bloat in test builds. | Low | Low | `anvilml-worker` is already a regular (non-dev) dependency of `anvilml-scheduler`. Adding it as a dev-dependency with an additional feature flag is a no-op for the dependency graph — it's already pulled in. |
| Existing tests may fail because they create empty pools and now the pool has handles. | Low | High | The `set_up_test_workers()` method is opt-in — existing tests that don't call it continue to use empty pools as before. No existing test code is modified. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests` exits 0 (≥21 total tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `grep -c "set_status.*Busy" crates/anvilml-scheduler/src/scheduler.rs` returns ≥1 (the call is present)
- [ ] `grep -c "set_up_test_workers" crates/anvilml-worker/src/pool.rs` returns ≥1 (the test helper exists)
