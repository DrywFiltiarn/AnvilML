# Plan Report: P12-A2

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P12-A2                                        |
| Phase       | 012 — Job Submission & Queue                  |
| Description | anvilml-scheduler: in-memory JobQueue         |
| Depends on  | P12-A1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-07T13:40:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `crates/anvilml-scheduler/src/queue.rs` — a thread-safe in-memory `JobQueue` that wraps `Mutex<VecDeque<Job>>` and exposes `enqueue`, `cancel_queued`, `pop_next`, and `len`. Register the module in `lib.rs` so downstream crates (scheduler, server) can import it. Provide unit tests verifying FIFO enqueue/pop order and that cancellation causes `pop_next` to skip the cancelled job.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/queue.rs` with the `JobQueue` struct and all four public methods
- Register the `queue` module in `crates/anvilml-scheduler/src/lib.rs` (add `pub mod queue;` and re-export)
- Unit tests under a `#[cfg(test)]` module inside `queue.rs`: enqueue+pop order test, cancel-then-pop skip test

### Out of Scope
- Any changes to `scheduler.rs`, `job_store.rs`, or `dag.rs` (handled by P12-A1, P12-A3)
- Any server-side handler wiring (handled by P12-A4, P12-A5)
- Persistence — this is purely in-memory; the DB layer is separate
- Concurrent dispatch loop or worker interaction

## Approach

1. **Read existing `lib.rs`** to confirm current module declarations and re-exports.
2. **Create `crates/anvilml-scheduler/src/queue.rs`** with:
   - `use std::collections::VecDeque;` (no new dependencies — `VecDeque` is in `std`)
   - `use anvilml_core::types::Job, JobStatus;` (already imported transitively via `anvilml-core`)
   - `pub struct JobQueue { inner: Mutex<VecDeque<Job>>; }`
   - `impl JobQueue`:
     - `fn new() -> Self` — empty queue
     - `fn enqueue(&self, job: Job)` — push_back onto the deque under lock
     - `fn cancel_queued(&self, id: uuid::Uuid) -> bool` — acquire lock, iterate deque in order, find first entry whose `Job::id == id` and whose `status == JobStatus::Queued`, set it to `Cancelled`, return `true`; if not found or already non-Queued, return `false`
     - `fn pop_next(&self) -> Option<Job>` — acquire lock, iterate deque from front, skip any entry with `status == JobStatus::Cancelled` (remove them from the deque), return the first entry with `status == JobStatus::Queued` (remove it from the deque and return it wrapped in `Some`); if no Queued entry remains, return `None`
     - `fn len(&self) -> usize` — acquire lock, return deque.len()
3. **Register the module** in `lib.rs`: add `pub mod queue;` and `pub use queue::JobQueue;` after existing lines.
4. **Write tests** inside `queue.rs` under `#[cfg(test)]`:
   - `test_enqueue_pop_order`: enqueue three jobs, pop three times, verify FIFO order by comparing IDs
   - `test_cancel_skipped_on_pop`: enqueue three jobs, cancel the middle one by ID, pop twice — first returns job #1, second returns job #3 (job #2 was removed as Cancelled)
5. **Verify** with `cargo test -p anvilml-scheduler -- queue` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-scheduler/src/queue.rs` | JobQueue struct + impl + unit tests |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod queue;` and `pub use queue::JobQueue;` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/queue.rs` (inline tests) | `enqueue_pop_order` | FIFO ordering: enqueue A, B, C → pop returns A, then B, then C |
| `crates/anvilml-scheduler/src/queue.rs` (inline tests) | `cancel_skipped_on_pop` | Cancelling middle job causes pop_next to skip it and return the next Queued job |

## CI Impact

No CI changes required. The task only adds a new module and unit tests within an existing crate. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy`) will naturally exercise this new code when run. No new dependencies are introduced (uses only `std::collections::VecDeque` and types already available from `anvilml-core`).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Lock contention on `Mutex<VecDeque<Job>>` under high enqueue/pop throughput in future phases | Low (not an issue for current scope) | Medium | Use `parking_lot::Mutex` instead of `std::sync::Mutex` if performance becomes a concern; not needed now |
| `cancel_queued` modifying an element in-place in a VecDeque requires mutable access — must use `get_mut` or collect indices first | Low | Low | Use `.get_mut(i)` after finding the index; standard VecDeque API supports this |
| `pop_next` removing cancelled entries during iteration could shift indices — must not hold iterator across mutation | Low | Low | Collect skipped indices into a small vec, then reverse-remove from back to front (preserves remaining indices) |

## Acceptance Criteria

- [ ] `crates/anvilml-scheduler/src/queue.rs` exists and compiles
- [ ] `JobQueue::new`, `enqueue`, `cancel_queued`, `pop_next`, `len` are all public methods on `pub struct JobQueue`
- [ ] `cargo test -p anvilml-scheduler -- queue` exits 0 (both unit tests pass)
- [ ] `lib.rs` declares `pub mod queue;` and re-exports `JobQueue`
