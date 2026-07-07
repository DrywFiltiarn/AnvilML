# Plan Report: P14-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P14-A2                                            |
| Phase       | 14 — Dispatch & Execute                           |
| Description | anvilml-scheduler: JobScheduler cancel()/get_job() |
| Depends on  | P14-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-07T13:50:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add two public methods to `JobScheduler` in `crates/anvilml-scheduler/src/scheduler.rs`:
`cancel(&self, id: Uuid) -> Result<bool, AnvilError>` which delegates to the in-memory
queue's `cancel()` method, and `get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError>`
which delegates to the `JobStore`'s `get()` method. These methods complete the scheduler's
read/cancel surface so that callers can cancel queued jobs and look up any job (including
terminal-state jobs that have already left the in-memory queue) from the authoritative
database. Acceptance: four new tests in `scheduler_tests.rs` (cancel queued, cancel unknown,
get persisted, get unknown), bringing the file to ≥8 tests total, with
`cargo test -p anvilml-scheduler --test scheduler_tests` exiting 0.

## Scope

### In Scope
- `crates/anvilml-scheduler/src/scheduler.rs`: add `pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>` and `pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError>`.
- `crates/anvilml-scheduler/tests/scheduler_tests.rs`: add four new `#[tokio::test]` functions (see Tests section).
- `crates/anvilml-scheduler/Cargo.toml`: bump patch version from `0.1.10` to `0.1.11`.

### Out of Scope
None. `defers_to (from JSON): []` — this task has an empty defers_to field and implements
its full scope. No functionality is deferred.

## Existing Codebase Assessment

**What already exists.** `JobScheduler` (in `scheduler.rs`, 219 lines) was created in P14-A1
with the struct shape (queue, ledger, job_store, node_registry, dispatch_notify fields all
wrapped correctly in `Mutex`/`Arc`) and the `submit()` method (218 lines of doc comments +
implementation). `JobQueue::cancel(&mut self, id: Uuid) -> bool` already exists in
`queue.rs` (line 89) — it inserts the ID into the `cancelled` HashSet and returns whether
the ID was newly inserted. `JobStore::get(&self, id: Uuid) -> Result<Option<Job>, AnvilError>`
already exists in `job_store.rs` (line 144) — it runs a single `SELECT` against the `jobs`
table and returns `Ok(None)` for missing IDs or `Ok(Some(job))` after deserializing the row.

**Established patterns.** All `JobScheduler` methods are `pub async fn`, use `#[tracing::instrument]`
with `skip(self)` and a `fields(...)` clause naming relevant IDs, return `Result<T, AnvilError>`,
and carry extensive `///` doc comments that describe steps, errors, and rationale. Tests live in
`tests/scheduler_tests.rs` as an integration test crate (not inline `#[cfg(test)]`), use the
`create_job_store()` and `make_registry()` helpers, and follow a consistent pattern: construct
scheduler → call method → assert on result. The mutex on `queue` is `tokio::sync::Mutex` (not
`std::sync::Mutex`) because it may be held across `.await` points.

**Gap between design and source.** There is no gap — the design doc (§12.5) specifies that
`cancel()` delegates to the queue and `get_job()` delegates to the DB, which is exactly what
the existing `JobQueue::cancel()` and `JobStore::get()` provide. No new types or APIs are
needed.

## Resolved Dependencies

No new external dependencies are introduced. The task reuses existing crates:
- `anvilml-core::Job` — already a dependency
- `anvilml-core::AnvilError` — already a dependency
- `anvilml_registry::JobStore` — already a dependency
- `uuid::Uuid` — already a dependency

| Type   | Name                | Version verified | MCP source | Feature flags confirmed |
|--------|---------------------|-----------------|------------|------------------------|
| crate  | (none new)          | n/a             | n/a        | n/a                    |

## Approach

1. **Add `cancel()` method to `JobScheduler`** in `scheduler.rs`, after the `submit()` method
   (before the closing `}` of `impl JobScheduler`).

   Signature:
   ```rust
   pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>
   ```

   Implementation: acquire the queue mutex lock, call `queue.cancel(id)`, return
   `Ok(result)`. The `queue.cancel()` method already returns `bool` (true = newly cancelled,
   false = already cancelled or unknown). No database update is needed — cancellation is an
   in-memory queue operation per the design doc's O(1) cancel semantics.

   ```rust
   /// Cancel a queued job by its ID.
   ///
   /// Delegates to the in-memory `JobQueue::cancel()` which marks the job as cancelled
   /// (O(1) via HashSet insertion). The job remains in the queue until `pop_front()`
   /// encounters it and discards it — this is the lazy removal that gives cancel() its
   /// O(1) guarantee.
   ///
   /// Returns `Ok(true)` if the ID was newly marked as cancelled, `Ok(false)` if the
   /// ID was already cancelled or not present in the queue. The job may have already
   /// left the queue (e.g. if it completed or was dispatched), in which case the method
   /// still returns `Ok(false)` — the authoritative state for terminal jobs is the
   /// database, not the in-memory queue.
   ///
   /// # Arguments
   ///
   /// * `id` — The job UUID to cancel.
   ///
   /// # Errors
   ///
   /// This method does not return errors; it always returns `Ok(bool)`. It is declared
   /// as `Result<bool, AnvilError>` for API consistency with `get_job()` and to allow
   /// future error propagation (e.g. if database cancellation logging is added).
   #[tracing::instrument(skip(self), fields(job_id = %id))]
   pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError> {
       let mut queue = self.queue.lock().await;
       let cancelled = queue.cancel(id);
       if cancelled {
           tracing::info!(job_id = %id, "cancelled job in queue");
       }
       Ok(cancelled)
   }
   ```

   Rationale for `Result<bool, AnvilError>` return type (instead of bare `bool`): API
   consistency with `get_job()` which must return `Result<Option<Job>, AnvilError>` because
   it performs async database I/O that can fail. Keeping the error variant on both methods
   allows a future caller to handle both with a single `?` operator.

2. **Add `get_job()` method to `JobScheduler`** in `scheduler.rs`, after `cancel()`.

   Signature:
   ```rust
   pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError>
   ```

   Implementation: call `self.job_store.get(id).await` and return the result directly.
   The `job_store.get()` method already handles the "not found" case by returning
   `Ok(None)` — no additional wrapping logic needed.

   ```rust
   /// Look up a job by its ID from the database.
   ///
   /// Delegates to `JobStore::get()` which queries the `jobs` table. This is the
   /// authoritative source for all jobs, including those that have already left the
   /// in-memory queue (Completed, Failed, Cancelled). A job that is currently in the
   /// queue is also queryable here since it was persisted before being enqueued.
   ///
   /// # Arguments
   ///
   /// * `id` — The job UUID to look up.
   ///
   /// # Errors
   ///
   /// Returns `AnvilError::Db` if the database query fails (e.g. connection error).
   #[tracing::instrument(skip(self), fields(job_id = %id))]
   pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError> {
       let job = self.job_store.get(id).await?;
       if job.is_some() {
           tracing::debug!(job_id = %id, "retrieved job from database");
       }
       Ok(job)
   }
   ```

   Rationale for delegating to `job_store` (not the in-memory queue): a Completed or
   Failed job has already been popped from the queue by the dispatch loop, but must still
   be queryable. The database is the authoritative source for terminal-state jobs.

3. **Add four tests** to `crates/anvilml-scheduler/tests/scheduler_tests.rs`.

   Each test uses the existing `create_job_store()` and `make_registry()` helpers.
   Tests are annotated with `///` doc comments per the project's test documentation
   obligation (`ANVILML_DESIGN.md §17.1`, `ENVIRONMENT.md §11.4`).

4. **Bump `anvilml-scheduler` version** from `0.1.10` to `0.1.11` in
   `crates/anvilml-scheduler/Cargo.toml` per `ENVIRONMENT.md §12`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `cancel` | `anvilml_scheduler::JobScheduler` | `pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>` |
| `get_job` | `anvilml_scheduler::JobScheduler` | `pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError>` |

Both methods are new `pub async fn` items on an existing struct. No new types, traits,
or re-exports are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `cancel()` and `get_job()` methods with doc comments and tracing |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add 4 new test functions (≥8 total) |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_cancel_queued_job_returns_true` | Cancelling a job that is currently in the in-memory queue returns `Ok(true)` | Registry populated with PassThrough; one job submitted and still in queue | Valid graph submit → job_id, then `cancel(job_id)` | `Ok(true)` — the ID was newly marked as cancelled | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_queued_job_returns_true` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_cancel_unknown_id_returns_false` | Cancelling a job ID that was never submitted returns `Ok(false)` | Registry populated; no jobs submitted (or submitted IDs all popped) | `cancel(nonexistent_uuid)` | `Ok(false)` — ID not in cancelled set | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_cancel_unknown_id_returns_false` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_get_job_returns_persisted_job` | Looking up a job that was submitted and persisted returns `Ok(Some(job))` with correct fields | Registry populated; one job submitted (persists to DB before enqueue) | `get_job(submitted_job_id)` | `Ok(Some(job))` where `job.id == submitted_job_id` and `job.status == Queued` | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_get_job_returns_persisted_job` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_get_job_unknown_id_returns_none` | Looking up a job ID that was never submitted returns `Ok(None)` | Registry populated; no jobs submitted | `get_job(nonexistent_uuid)` | `Ok(None)` — no row in DB for that ID | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_get_job_unknown_id_returns_none` exits 0 |

## CI Impact

No CI changes required. The tests run via `cargo test --workspace --features mock-hardware`
which already includes `anvilml-scheduler`. No new file types, new gates, or new test
modules are introduced — the four tests live in the existing `scheduler_tests.rs` file.

## Platform Considerations

None identified. The methods operate on in-memory data structures (`JobQueue`) and
database-backed persistence (`JobStore`), both of which are platform-neutral. No
`#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in
`ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobStore::get()` returns an error if the `jobs` table does not exist (migration not applied to test pool) | Low | High | The existing `create_job_store()` helper already applies migrations via `sqlx::migrate!()` before returning — this is the same pattern used by all 4 existing P14-A1 tests. Verify the helper is unchanged. |
| `cancel()` returns `Ok(false)` for a job that was already popped from the queue (e.g. completed) — callers might expect `Ok(true)` if the job was previously in the queue but is no longer there | Medium | Low | This is the correct behaviour per the design doc: cancellation is an in-memory queue operation, and a job that has left the queue cannot be cancelled there. The `get_job()` method provides the authoritative lookup for terminal-state jobs. Document this clearly in the doc comment. |
| Test `test_cancel_unknown_id_returns_false` could pass spuriously if the cancel method panics or returns a non-Ok result | Low | Medium | The test uses `.await.expect()` on the Result and then asserts `== false`. If cancel panics, the test fails. Use `assert!(result.is_ok())` before checking the boolean value for defensive clarity. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests` exits 0 (≥8 tests total)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
