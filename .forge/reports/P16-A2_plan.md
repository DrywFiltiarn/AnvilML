# Plan Report: P16-A2

| Field | Value |
|-------|-------|
| Task ID | P16-A2 |
| Phase | 016 — Job Cancellation |
| Description | anvilml-scheduler: JobScheduler::cancel (queued + running) |
| Depends on | P16-A1 |
| Project | anvilml |
| Planned at | 2026-06-10T13:30:00Z |
| Attempt | 1 |

## Objective

Add `async fn cancel(&self, id: Uuid) -> Result<(), AnvilError>` to `JobScheduler` in
`crates/anvilml-scheduler/src/scheduler.rs`. The method reads the job from the database,
rejects cancellation for terminal-status jobs, cancels queued jobs in-memory + updates DB,
and sends a `CancelJob` IPC message to the owning worker for running jobs. Both paths update
the DB status to `Cancelled` and broadcast a `JobCancelled` WebSocket event. A companion
test verifies queued cancel and running cancel both reach `Cancelled` status.

## Scope

### In Scope
- Add `JobNotCancellable(Uuid)` variant to `AnvilError` enum in `anvilml-core/src/error.rs`
- Add `pub async fn cancel(&self, id: Uuid) -> Result<(), AnvilError>` to `JobScheduler` in `scheduler.rs`
  - Read job from DB via `get_job`
  - Terminal status → `Err(JobNotCancellable(id))`
  - Queued → `queue.cancel_queued(id)` + `update_status(Cancelled)` + broadcast `JobCancelled`
  - Running → `workers.send(owner, CancelJob{job_id})` + `update_status(Cancelled)` + broadcast `JobCancelled`
  - `JobNotFound` → `Err(JobNotFound(id))`
- Update existing `handle_cancelled` to also accept `Queued` (for the race where a queued
  job was cancelled by `cancel()` before the dispatch loop tried to pop it)
- Add two unit tests: queued cancel and running cancel
- Add `JobNotCancellable` display + debug formatting + test coverage

### Out of Scope
- HTTP handler for `POST /v1/jobs/:id/cancel` (P16-A3)
- Integration test `api_cancel.rs` (P16-A4)
- Worker-side cooperative cancel logic (P16-A1)
- Any changes to `anvilml-server`, `backend/`, or Python worker code

## Approach

1. **Add `JobNotCancellable` error variant** to `AnvilError` in `crates/anvilml-core/src/error.rs`:
   - Add `JobNotCancellable(Uuid)` variant after `JobNotFound`
   - Add `Display` arm: `"job not cancellable: {id}"`
   - Add test case in `all_variants_display()`

2. **Implement `JobScheduler::cancel`** in `crates/anvilml-scheduler/src/scheduler.rs`:
   ```rust
   #[tracing::instrument(skip(self), fields(job_id = %id))]
   pub async fn cancel(&self, id: Uuid) -> Result<(), AnvilError> {
       // 1. Read job from DB.
       let job = get_job(&self.db, id)
           .await
           .map_err(|e| AnvilError::DbError(format!("failed to read job: {e}")))?;
       let job = job.ok_or_else(|| AnvilError::JobNotFound(id))?;

       // 2. Terminal → reject.
       if !matches!(job.status, JobStatus::Queued | JobStatus::Running) {
           return Err(AnvilError::JobNotCancellable(id));
       }

       let now = Utc::now();
       let worker_id = job.worker_id.clone();

       match job.status {
           JobStatus::Queued => {
               // 3a. Cancel in queue + update DB + broadcast.
               self.queue.cancel_queued(id);
               update_status(&self.db, id, JobStatus::Cancelled, None, None, None, None)
                   .await
                   .ok();
               let _ = self.broadcaster.send(WsEvent::JobCancelled(JobCancelledEvent {
                   event: "job.cancelled".to_string(),
                   timestamp: now,
                   job_id: id,
               }));
               self.dispatch_notify.notify_one();
               tracing::info!(job_id = %id, "job cancelled (queued)");
           }
           JobStatus::Running => {
               // 3b. Send CancelJob IPC + update DB + broadcast.
               if let Some(ref wid) = worker_id {
                   let _ = self.workers.send(wid, WorkerMessage::CancelJob { job_id: id }).await;
               }
               update_status(&self.db, id, JobStatus::Cancelled, None, None, None, None)
                   .await
                   .ok();
               let _ = self.broadcaster.send(WsEvent::JobCancelled(JobCancelledEvent {
                   event: "job.cancelled".to_string(),
                   timestamp: now,
                   job_id: id,
               }));
               self.dispatch_notify.notify_one();
               tracing::info!(job_id = %id, "job cancel requested (running)");
           }
           _ => unreachable!(),
       }

       Ok(())
   }
   ```

3. **Update `handle_cancelled`** to accept `Queued` status (race guard):
   - Change the guard from `JobStatus::Running` to `JobStatus::Running | JobStatus::Queued`
   - This handles the edge case where a queued job was cancelled by `cancel()` before the
     dispatch loop picked it up and emitted `Cancelled`

4. **Add unit tests** in `scheduler.rs` test module:
   - `test_cancel_queued`: Submit → verify Queued → call `cancel()` → verify Cancelled in DB + broadcast event
   - `test_cancel_running`: Submit → wait for Running → call `cancel()` → verify Cancelled in DB + broadcast event + worker sent CancelJob

5. **Logging** (§11.5 mandatory DEBUG — job state transition):
   - `cancel` method: `tracing::info!(job_id = %id, "job cancelled (queued)")` and
     `tracing::info!(job_id = %id, "job cancel requested (running)")`
   - Existing `handle_cancelled` already has `tracing::info!(job_id = %id, "job cancelled")`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/error.rs` | Add `JobNotCancellable(Uuid)` variant + Display + test |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `cancel()` method + update `handle_cancelled` guard + 2 new tests |
| Bump | `crates/anvilml-scheduler/Cargo.toml` | Patch version `0.1.16 → 0.1.17` (§12) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-scheduler/src/scheduler.rs` (tests module) | `test_cancel_queued` | Submit → Queued → `cancel()` → DB Cancelled + broadcast `JobCancelled` + queue cleared |
| `crates/anvilml-scheduler/src/scheduler.rs` (tests module) | `test_cancel_running` | Submit → Running → `cancel()` → DB Cancelled + broadcast `JobCancelled` + worker sent `CancelJob` |
| `crates/anvilml-core/src/error.rs` (tests module) | `all_variants_display` (updated) | `JobNotCancellable` produces valid Display string |

## CI Impact

No CI workflow changes. The task only modifies source files within existing crates.
The test command `cargo test -p anvilml-scheduler --features mock-hardware -- cancel`
will exercise the new tests. The full workspace test suite must also pass.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `update_status` WHERE clause only allows `Queued`/`Running` transitions, but cancelling a queued job sets it to `Cancelled` which is fine since the WHERE clause allows FROM `Queued`. | Low | Low | Verified: the WHERE clause `status = 'Queued' OR status = 'Running'` correctly permits transitions from Queued to Cancelled. |
| Race between `cancel()` and dispatch loop: dispatch loop might pop the job between `cancel_queued()` and `update_status()`. | Low | Medium | The dispatch loop's `pop_next` skips Cancelled entries. If the dispatch loop already popped the job, `update_status` will fail (rows_affected=0) because the status is now `Running` and the WHERE clause still allows it — but the job was already dispatched. The `handle_cancelled` guard update handles this. |
| `workers.send()` fails for running job (worker not found). | Low | Medium | The send result is ignored (`let _ = ...`) — the DB is still updated to Cancelled. The dispatch loop will eventually receive a `Cancelled` or `Failed` event from the worker watchdog. |
| `handle_cancelled` guard change breaks existing `test_cancel_broadcasts_event`. | Low | Medium | The existing test sends `Cancelled` event when job is `Running` — the expanded guard (`Running | Queued`) still matches `Running`, so no breakage. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- cancel` exits 0 with both queued cancel and running cancel tests passing
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regressions)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `JobNotCancellable` error variant exists in `AnvilError` with valid Display output
- [ ] `JobScheduler::cancel` is `pub async` and handles all three cases (not found, terminal, queued/running)
- [ ] Queued cancel: DB status → Cancelled, queue entry removed, `JobCancelled` event broadcast
- [ ] Running cancel: DB status → Cancelled, `CancelJob` IPC sent to owning worker, `JobCancelled` event broadcast
- [ ] Crate version bumped from `0.1.16` to `0.1.17` in `crates/anvilml-scheduler/Cargo.toml`
