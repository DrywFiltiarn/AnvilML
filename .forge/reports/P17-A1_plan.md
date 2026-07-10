# Plan Report: P17-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P17-A1                                            |
| Phase       | 17 — Cancellation                                 |
| Description | anvilml-scheduler: JobScheduler::cancel() dispatches by current status |
| Depends on  | P16-A2                                              |
| Project     | anvilml                                             |
| Planned at  | 2026-07-11T00:00:00Z                               |
| Attempt     | 1                                                   |

## Objective

Replace the queue-only `JobScheduler::cancel()` delegate (Phase 14's P14-A2) with status-aware branching that handles three cases: (1) `Queued` jobs get cancelled via the existing `queue.cancel()` lazy-removal mechanism plus immediate database status update to `Cancelled`, (2) `Running` jobs return `Ok(true)` without sending any IPC signal (deferred to P17-A2), and (3) already-terminal or unknown jobs return `Ok(false)` as a no-op. This gives the HTTP handler (Phase 17's P17-C1) the information it needs to return 202 vs 409 vs 404 status codes. Five integration tests in `tests/scheduler_tests.rs` verify all branches, and `cargo test -p anvilml-scheduler --test scheduler_tests` exits 0.

## Scope

### In Scope
- Modify `JobScheduler::cancel()` in `crates/anvilml-scheduler/src/scheduler.rs` to branch on job status.
- For `Queued` jobs: call `queue.cancel()`, then update the job's database status to `Cancelled` via `job_store.upsert()`.
- For `Running` jobs: return `Ok(true)` without sending IPC (stub the IPC send for P17-A2).
- For terminal states (`Completed`/`Failed`/`Cancelled`) and unknown IDs: return `Ok(false)`.
- Add five integration tests in `crates/anvilml-scheduler/tests/scheduler_tests.rs`.
- Bump `anvilml-scheduler` crate version from `0.1.25` to `0.1.26`.

### Out of Scope
- The actual IPC `CancelJob` send for `Running` jobs — deferred to P17-A2, which states: "Complete JobScheduler::cancel()'s Running branch ... the IPC send P17-A1 deferred: send WorkerMessage::CancelJob{job_id} to the job's assigned worker_id."
- The HTTP handler `POST /v1/jobs/:id/cancel` — separate task P17-C1.
- The Python worker-side executor and dispatch loop changes — tasks P17-B1 through P17-B5.

## Existing Codebase Assessment

**What already exists:** `JobScheduler::cancel()` (scheduler.rs lines 297–327) is currently a thin delegate to `JobQueue::cancel()` which performs O(1) lazy-removal via `HashSet` insertion. The job's database status is never updated by `cancel()` — the queue only tracks which IDs are pending lazy removal. `JobQueue::cancel()` returns `true` if the ID was newly marked, `false` if already cancelled or not present in the queue. `JobStore::get(id)` and `job_store.upsert(&job)` are already available for database reads/writes. `JobStatus` is a five-variant enum (`Queued`, `Running`, `Completed`, `Failed`, `Cancelled`) defined in `anvilml-core`.

**Established patterns:** Tests use `create_job_store()` for in-memory SQLite isolation, `make_registry()` for a PassThrough node, and `make_valid_graph()` for valid graph JSON. The `test-util` feature gates test-only methods like `persist_job_test()` and `dispatch_one_test()`. Tests use `#[tokio::test]` async macros. Database updates use `job_store.upsert(&job).await`. Logging uses `tracing::info!` for lifecycle events and `tracing::debug!` for routine operations. The `#[tracing::instrument]` attribute decorates public methods.

**Gap between design doc and source:** The design doc (`ANVILML_DESIGN.md §20`) specifies that `cancel()` should branch on status, but the current implementation does not — it is purely queue-based. There is no existing code path that reads a job's database status or updates it during cancellation. This gap is exactly what this task fills.

## Resolved Dependencies

None. This task modifies existing code in the `anvilml-scheduler` crate and uses only already-declared dependencies (`anvilml-core`, `anvilml-registry`, `anvilml-ipc`). No new crates or versions are introduced.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) | —    | —               | —          | —                      |

## Approach

### Step 1: Extend `JobScheduler::cancel()` with status branching

Replace the body of `cancel()` (scheduler.rs lines 320–327) with the following logic, preserving the existing `#[tracing::instrument]` attribute and signature (`pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>`):

```rust
#[tracing::instrument(skip(self), fields(job_id = %id))]
pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError> {
    // First, try the queue. This handles Queued jobs and returns false
    // for IDs not in the queue (Running, terminal, or unknown).
    {
        let mut queue = self.queue.lock().await;
        if queue.cancel(id) {
            // The job was in the queue and newly marked as cancelled.
            // Update its database status to Cancelled so get_job() reflects
            // the cancellation immediately, even before pop_front() discards it.
            // This is the Queued branch of the status-aware cancel.
            if let Ok(Some(mut job)) = self.job_store.get(id).await {
                job.status = JobStatus::Cancelled;
                if let Err(e) = self.job_store.upsert(&job).await {
                    tracing::error!(
                        job_id = %id,
                        error = %e,
                        "cancel: failed to persist Cancelled status for queued job"
                    );
                    // Best-effort: if persist fails, the queue-level cancel
                    // still succeeded — the job won't be dispatched.
                    // We return Ok(true) because the cancellation did take
                    // effect at the queue level, which is what matters.
                } else {
                    tracing::info!(job_id = %id, "cancelled queued job");
                }
            }
            return Ok(true);
        }
    }

    // The job was not in the queue (or already marked cancelled there).
    // Check the database to determine its status.
    match self.job_store.get(id).await? {
        Some(job) => {
            // Job exists in the database — branch on its current status.
            match job.status {
                JobStatus::Running => {
                    // Running jobs: return Ok(true) to signal "cancellation requested."
                    // The actual IPC send of WorkerMessage::CancelJob is deferred
                    // to P17-A2. We do NOT change the job's status here — the
                    // event loop (Phase 16) will set it to Cancelled once the
                    // worker's own Cancelled event arrives.
                    tracing::info!(
                        job_id = %id,
                        "cancel: Running job — IPC send deferred to P17-A2"
                    );
                    Ok(true)
                }
                // Terminal states: cancelling a finished job is a no-op, not an error.
                // Return Ok(false) to let the HTTP handler return 409 Conflict.
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
                    tracing::debug!(
                        job_id = %id,
                        status = ?job.status,
                        "cancel: already terminal — no-op"
                    );
                    Ok(false)
                }
            }
        }
        None => {
            // Job not found in the database at all — unknown ID.
            // Return Ok(false) so the HTTP handler can return 404 Not Found.
            tracing::debug!(job_id = %id, "cancel: job not found in database");
            Ok(false)
        }
    }
}
```

**Rationale for each design choice:**
- The queue check comes first because `Queued` is the most common cancel target (jobs spend most of their lifecycle in the queue). The existing `queue.cancel()` call is preserved for its O(1) semantics.
- After the queue check, we query the database for the authoritative status. This handles Running jobs (in DB but not in queue because they've been dispatched), terminal jobs (completed/failed/cancelled), and unknown IDs.
- For the Running branch, we return `Ok(true)` without sending IPC. This matches the task context: "this task only updates the return contract — returns Ok(true), defers the actual IPC send to the next task." The status stays `Running` because the event loop (Phase 16) handles the transition to `Cancelled` when the worker's `Cancelled` event arrives.
- For terminal states, we return `Ok(false)` — cancelling a finished job is a no-op per the idempotent-cancel principle.
- For unknown IDs, we return `Ok(false)` — the job doesn't exist at all.

### Step 2: Write five integration tests

Add the following tests to `crates/anvilml-scheduler/tests/scheduler_tests.rs`:

**Test 1: `test_cancel_queued_job_sets_cancelled_status`**
- Submit a job (which creates a `Queued` job persisted and enqueued).
- Call `cancel(job_id)`.
- Assert `Ok(true)` — the job was newly cancelled.
- Assert the database record shows `status == Cancelled` via `get_job()`.

**Test 2: `test_cancel_running_job_returns_true_without_ipc`**
- Create a `Job` struct manually with `status = JobStatus::Running`, `id = Uuid::new_v4()`.
- Persist it to the database via `job_store.upsert(&job).await` (bypassing `scheduler.submit()` which only creates `Queued` jobs).
- Call `cancel(job_id)`.
- Assert `Ok(true)` — cancellation was accepted.
- Assert the job's status is still `Running` (we don't change it in this task).

**Test 3: `test_cancel_terminal_job_returns_false`**
- Create a `Job` with `status = JobStatus::Completed`, persist it.
- Call `cancel(job_id)`.
- Assert `Ok(false)` — no-op for already-finished job.
- Repeat for `Failed` and `Cancelled` statuses (one test covering all three terminal states, or three separate tests — the task says "cancel on an already-terminal job returns Ok(false)" which implies at least one terminal state, but testing all three is better).

**Test 4: `test_cancel_unknown_id_returns_false`**
- Generate a fresh UUID that was never submitted.
- Call `cancel(unknown_id)`.
- Assert `Ok(false)` — the ID doesn't exist in the queue or database.

**Test 5: `test_cancel_already_cancelled_queued_job_returns_false`**
- Submit a job, cancel it (returns `Ok(true)`).
- Cancel it again with the same ID.
- Assert `Ok(false)` — already cancelled, no-op.

### Step 3: Bump crate version

Bump `crates/anvilml-scheduler/Cargo.toml` version from `0.1.25` to `0.1.26`.

### Step 4: Run tests

Execute `cargo test -p anvilml-scheduler --test scheduler_tests` and confirm it exits 0 with >=5 tests.

## Public API Surface

No new `pub` items are introduced. The existing `pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>` signature remains unchanged — only its internal behavior is extended.

| Item | Location | Change |
|------|----------|--------|
| `JobScheduler::cancel()` | `crates/anvilml-scheduler/src/scheduler.rs` | Internal logic extended with status branching; signature unchanged |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Extend `cancel()` with status-aware branching (Queued/Running/terminal/unknown) |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add 5 new integration tests for cancel status branches |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.25 → 0.1.26 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_cancel_queued_job_sets_cancelled_status` | Cancel on a Queued job calls `queue.cancel()`, persists `status=Cancelled` to DB, returns `Ok(true)` | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_queued_job_sets_cancelled_status` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_cancel_running_job_returns_true_no_ipc` | Cancel on a Running job returns `Ok(true)` without changing status or sending IPC | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_running_job_returns_true_no_ipc` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_cancel_terminal_job_returns_false` | Cancel on Completed/Failed/Cancelled job returns `Ok(false)` — no-op, not an error | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_terminal_job_returns_false` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_cancel_unknown_id_returns_false` | Cancel on a UUID never submitted returns `Ok(false)` — not found in queue or DB | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_unknown_id_returns_false` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_cancel_already_cancelled_queued_job_returns_false` | Cancel on a job already marked cancelled returns `Ok(false)` — idempotent no-op | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_already_cancelled_queued_job_returns_false` exits 0 |

## CI Impact

No CI changes required. This task only modifies source and test files within the existing `anvilml-scheduler` crate. The existing CI jobs (`rust-linux`, `rust-windows`) already run `cargo test --workspace --features mock-hardware` which includes this crate's tests. No new file types, gates, or CI configuration are introduced.

## Platform Considerations

None identified. The `cancel()` method operates on in-memory data structures (`JobQueue`) and database operations (`JobStore`) — both platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobStore::get(id)` may not return a `Job` with the correct status for Running jobs if the job was submitted but not yet dispatched (it would still be `Queued` in the DB). The test for Running jobs must manually construct and persist a `Running`-status job, bypassing `submit()`. | Low | Medium | The test uses `job_store.upsert(&job).await` directly to write a `Running`-status job, independent of the `submit()` flow. This is the same pattern used by existing test helpers like `persist_job_test()`. |
| The `queue.cancel()` call returns `false` for IDs not in the queue, but the same ID might exist in the database as a `Running` or terminal job. If the database query after a `false` queue result fails (e.g. DB error), the method should propagate the error. | Low | Low | The `?` operator on `self.job_store.get(id).await?` propagates any `AnvilError::Db` to the caller, preserving the existing error-handling contract. |
| Logging verbosity: the Running branch logs at INFO level ("cancel: Running job — IPC send deferred to P17-A2"), which may be noisy in production if many running jobs are cancelled. | Low | Low | The INFO log is appropriate for the operational lifecycle event (a cancel request was received for a running job). DEBUG-level detail about the deferred IPC can be added later when P17-A2 implements the actual send. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_queued_job_sets_cancelled_status` exits 0
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_running_job_returns_true_no_ipc` exits 0
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_terminal_job_returns_false` exits 0
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_unknown_id_returns_false` exits 0
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_already_cancelled_queued_job_returns_false` exits 0
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests` exits 0 with >= 5 tests total
- [ ] `cargo clippy -p anvilml-scheduler --features mock-hardware -- -D warnings` exits 0
