# Plan Report: P13-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-A1                                            |
| Phase       | 013 — Job Queue & Persistence                     |
| Description | anvilml-scheduler: queue.rs JobQueue FIFO with O(1) cancel |
| Depends on  | none                                              |
| Project     | anvilml                                           |
| Planned at  | 2026-06-19T20:20:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the `JobQueue` data structure in `crates/anvilml-scheduler/src/queue.rs` — a FIFO queue backed by `VecDeque` with an index map (`HashMap<Uuid, usize>`) that enables O(1) cancellation. This is the foundational data structure for the job scheduler; it must support push, pop, cancel, get, list, and len operations with correct FIFO ordering. The acceptance criterion is `cargo test -p anvilml-scheduler --features mock-hardware -- queue` exiting 0 with ≥ 6 tests.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/queue.rs` with `JobQueue` struct and all six public methods
- Create `crates/anvilml-scheduler/tests/queue_tests.rs` with ≥ 6 tests
- Modify `crates/anvilml-scheduler/src/lib.rs` to add `pub mod queue`
- Bump `anvilml-scheduler` patch version from `0.1.4` to `0.1.5`

### Out of Scope
- VRAM ledger (`ledger.rs`) — handled by P13-A2
- JobScheduler with SQLite persistence (`scheduler.rs`) — handled by P13-A3
- HTTP handler wiring — handled by P13-B1
- Any async, I/O, database, or network code — `JobQueue` is pure synchronous logic

## Existing Codebase Assessment

The `anvilml-scheduler` crate currently exports `NodeTypeRegistry` (re-exported from `anvilml_core`), `GraphError` (from `types.rs`), and `ValidatedGraph` + `validate_graph` (from `dag.rs`). The crate already has three test files in `tests/`: `dag_tests.rs`, `node_registry_tests.rs`, and the workspace root Cargo.toml defines all shared dependencies.

The `Job` type is defined in `anvilml-core/src/types/job.rs` as a non-`Default` struct with fields `id: Uuid`, `status: JobStatus`, `graph: serde_json::Value`, `settings: JobSettings`, `created_at: DateTime<Utc>`, `started_at: Option<DateTime<Utc>>`, `completed_at: Option<DateTime<Utc>>`, `worker_id: Option<String>`, `error: Option<String>`, `queue_position: Option<u32>`. It derives `Debug, Clone, Serialize, Deserialize, ToSchema`.

The `JobStatus` enum (in the same file) derives `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema` with variants `Queued`, `Running`, `Completed`, `Failed`, `Cancelled`.

Test patterns in this crate use `#[tokio::test]` for async tests (needed when `NodeTypeRegistry` is involved, which uses `RwLock`), but `JobQueue` is pure sync — no `#[tokio::test]` needed. The test file convention is `tests/{module}_tests.rs`. The `serial_test` crate is available as a dev-dependency in `anvilml-core` but not directly in `anvilml-scheduler` — however, since this task's tests are pure sync with no shared state, `#[serial]` is not required.

The crate's `Cargo.toml` already depends on `anvilml-core`, `tokio` (workspace), `tracing` (workspace), `serde_json` (workspace), `indexmap` (local), and the hardware/registry/worker crates. No new external dependencies are needed — `VecDeque` and `HashMap` are from `std::collections`.

## Resolved Dependencies

No new external dependencies are introduced by this task. All types used (`VecDeque`, `HashMap`, `Uuid`) are either from `std::collections` (no dependency needed) or already available through `anvilml-core` (which re-exports `Uuid` from `uuid = "1.23.3"`).

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| std    | VecDeque| (stdlib)        | n/a            | n/a                    |
| std    | HashMap | (stdlib)        | n/a            | n/a                    |
| crate  | uuid    | 1.23.3          | workspace Cargo.toml | serde, v4      |

Note: `uuid` is already a workspace dependency and is re-exported via `anvilml_core::Uuid`. The `anvilml-scheduler` crate accesses it through `anvilml_core::Uuid`.

## Approach

1. **Create `crates/anvilml-scheduler/src/queue.rs`** with the `JobQueue` struct and all six public methods:

   - `JobQueue { items: VecDeque<Job>, by_id: HashMap<Uuid, usize> }` — private fields, no `pub` struct definition (only pub methods).
   - `impl JobQueue` block with:
     - `pub fn new() -> Self` — returns empty queue.
     - `pub fn push(&mut self, job: Job)` — pushes to back of VecDeque, records index in by_id.
     - `pub fn pop_front(&mut self) -> Option<Job>` — pops front, removes from by_id.
     - `pub fn cancel(&mut self, id: Uuid) -> bool` — lookup in by_id, swap-remove from VecDeque, update displaced index in by_id.
     - `pub fn get(&self, id: &Uuid) -> Option<&Job>` — lookup in by_id, return reference.
     - `pub fn list(&self) -> Vec<&Job>` — return slice of VecDeque as Vec of references.
     - `pub fn len(&self) -> usize` — return `self.items.len()`.
   - All methods get `///` doc comments per FORGE_AGENT_RULES §12.1.
   - The swap-remove in `cancel` needs an inline comment explaining the O(1) strategy: swap the cancelled item with the last item, pop the last, then update the index of the displaced item in `by_id`.

2. **Modify `crates/anvilml-scheduler/src/lib.rs`** — add `pub mod queue;` after the existing `pub mod dag;` line. No other changes needed.

3. **Create `crates/anvilml-scheduler/tests/queue_tests.rs`** with ≥ 6 tests:
   - `test_push_pop_fifo_order` — push three jobs, pop three times, verify FIFO order.
   - `test_pop_empty_returns_none` — pop from empty queue returns None.
   - `test_cancel_returns_true_and_removes` — push a job, cancel it, verify it's gone, len decreased.
   - `test_cancel_returns_false_for_missing_id` — cancel with unknown UUID returns false.
   - `test_get_returns_job_by_id` — push a job, get it by ID, verify fields match.
   - `test_list_returns_all_jobs_in_order` — push three jobs, list returns all three in push order.
   - `test_len_after_operations` — track len through push/pop/cancel operations.

4. **Bump `anvilml-scheduler` version** from `0.1.4` to `0.1.5` in `crates/anvilml-scheduler/Cargo.toml`.

## Public API Surface

```rust
// File: crates/anvilml-scheduler/src/queue.rs

use std::collections::{HashMap, VecDeque};
use uuid::Uuid;
use anvilml_core::Job;

/// A FIFO job queue with O(1) cancellation.
///
/// Backed by a `VecDeque` for FIFO ordering and a `HashMap<Uuid, usize>`
/// index map that enables O(1) lookup and removal by job ID.
pub struct JobQueue {
    /// Jobs in FIFO dispatch order.
    items: VecDeque<Job>,
    /// Maps each job's UUID to its index in `items`.
    by_id: HashMap<Uuid, usize>,
}

impl JobQueue {
    /// Create an empty `JobQueue`.
    pub fn new() -> Self;

    /// Enqueue a job at the back of the FIFO queue.
    ///
    /// The job becomes available for `pop_front` after all previously
    /// enqueued jobs have been popped.
    pub fn push(&mut self, job: Job);

    /// Remove and return the job at the front of the FIFO queue.
    ///
    /// Returns `None` if the queue is empty.
    pub fn pop_front(&mut self) -> Option<Job>;

    /// Cancel (remove) a job by its UUID.
    ///
    /// Returns `true` if the job was found and removed, `false` if no
    /// job with that ID exists in the queue.
    ///
    /// Uses swap-remove for O(1) removal: the cancelled item is swapped
    /// with the last item, then the last item is popped. The index of
    /// the displaced item (if any) is updated in `by_id`.
    pub fn cancel(&mut self, id: Uuid) -> bool;

    /// Look up a job by its UUID without removing it.
    ///
    /// Returns `None` if no job with that ID exists in the queue.
    pub fn get(&self, id: &Uuid) -> Option<&Job>;

    /// Return all jobs in the queue in FIFO dispatch order.
    ///
    /// The returned slice contains references to the internal jobs.
    /// This is a snapshot — subsequent mutations may invalidate the
    /// references (but not the values).
    pub fn list(&self) -> Vec<&Job>;

    /// Return the number of jobs currently in the queue.
    pub fn len(&self) -> usize;
}
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/queue.rs` | `JobQueue` struct and all six public methods |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod queue;` |
| CREATE | `crates/anvilml-scheduler/tests/queue_tests.rs` | ≥ 6 unit tests for JobQueue |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.4 → 0.1.5 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/queue_tests.rs` | `test_push_pop_fifo_order` | Three pushes followed by three pops return jobs in FIFO order | Empty queue | Three `Job` values with distinct UUIDs | Pop order matches push order | `cargo test -p anvilml-scheduler --features mock-hardware -- queue::test_push_pop_fifo_order` exits 0 |
| `crates/anvilml-scheduler/tests/queue_tests.rs` | `test_pop_empty_returns_none` | Popping from an empty queue returns None | Empty queue (new()) | None | `None` | `cargo test -p anvilml-scheduler --features mock-hardware -- queue::test_pop_empty_returns_none` exits 0 |
| `crates/anvilml-scheduler/tests/queue_tests.rs` | `test_cancel_returns_true_and_removes` | Cancelling an existing job returns true and removes it from the queue | Queue has one job | Job UUID | `true`, `len()` decreased by 1, `get()` returns `None` | `cargo test -p anvilml-scheduler --features mock-hardware -- queue::test_cancel_returns_true_and_removes` exits 0 |
| `crates/anvilml-scheduler/tests/queue_tests.rs` | `test_cancel_returns_false_for_missing_id` | Cancelling a non-existent job returns false | Queue has jobs (but not the target) | Unknown UUID | `false`, queue unchanged | `cargo test -p anvilml-scheduler --features mock-hardware -- queue::test_cancel_returns_false_for_missing_id` exits 0 |
| `crates/anvilml-scheduler/tests/queue_tests.rs` | `test_get_returns_job_by_id` | `get()` returns the correct job reference by UUID | Queue has one job | Job UUID | `Some(&job)` with matching fields | `cargo test -p anvilml-scheduler --features mock-hardware -- queue::test_get_returns_job_by_id` exits 0 |
| `crates/anvilml-scheduler/tests/queue_tests.rs` | `test_list_returns_all_jobs_in_order` | `list()` returns all jobs in FIFO push order | Queue has three jobs | Three pushed jobs | `Vec` of length 3, order matches push order | `cargo test -p anvilml-scheduler --features mock-hardware -- queue::test_list_returns_all_jobs_in_order` exits 0 |
| `crates/anvilml-scheduler/tests/queue_tests.rs` | `test_len_after_operations` | `len()` correctly tracks push, pop, and cancel operations | Empty queue | Series of push/pop/cancel calls | `len()` matches expected count at each step | `cargo test -p anvilml-scheduler --features mock-hardware -- queue::test_len_after_operations` exits 0 |

## CI Impact

No CI changes required. The task adds a new test file in `crates/anvilml-scheduler/tests/` which is picked up automatically by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test modules are introduced that would require CI configuration changes.

## Platform Considerations

None identified. The `JobQueue` data structure uses only `std::collections::VecDeque` and `std::collections::HashMap`, which are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `Job` type does not derive `Clone` — swap-remove in `cancel` may need a temporary clone or different removal strategy | Low | Medium | `Job` does derive `Clone` (confirmed in `anvilml-core/src/types/job.rs` line 74). Swap-remove is safe. |
| `Uuid` import path — the scheduler crate may not have direct access to `uuid` crate | Low | Low | `uuid` is a workspace dependency; `anvilml-core` re-exports it via `pub use uuid::Uuid`. The scheduler accesses it through `anvilml_core::Uuid`. |
| Test compilation fails because `Job` requires `created_at: DateTime<Utc>` which needs `chrono` | Low | Low | `chrono` is already a workspace dependency. Creating test jobs uses `chrono::Utc::now()` which is available through `anvilml_core`. |
| `cancel` swap-remove changes the FIFO order of remaining items (displaced item moves to end) | Medium | Low | This is by design — the task explicitly requires O(1) cancel. The displaced item's index is updated in `by_id` so `get()` still works correctly. The FIFO order for `pop_front` is only affected for the displaced item, which is acceptable trade-off for O(1) cancel. Document this in the method doc comment. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- queue` exits 0 with ≥ 6 tests
- [ ] `cargo check -p anvilml-scheduler --features mock-hardware` exits 0
- [ ] `head -1 .forge/reports/P13-A1_plan.md` prints `# Plan Report: P13-A1`
- [ ] `grep "^## " .forge/reports/P13-A1_plan.md` shows all 12 section headings
- [ ] `wc -l .forge/reports/P13-A1_plan.md` returns > 40 lines
- [ ] `crates/anvilml-scheduler/src/queue.rs` exists and contains `pub struct JobQueue` with private fields `items: VecDeque<Job>` and `by_id: HashMap<Uuid, usize>`
- [ ] `crates/anvilml-scheduler/src/lib.rs` contains `pub mod queue;`
- [ ] `crates/anvilml-scheduler/Cargo.toml` version is `0.1.5`
