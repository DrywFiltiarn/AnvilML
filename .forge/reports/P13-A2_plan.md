# Plan Report: P13-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-A2                                            |
| Phase       | 13 — Job Queue                                    |
| Description | anvilml-scheduler: JobQueue in-memory FIFO with O(1) cancel |
| Depends on  | P13-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-07T00:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-scheduler/src/queue.rs` implementing `JobQueue`: a pure in-memory FIFO queue backed by `VecDeque<Job>` with a `HashSet<Uuid>` of cancelled IDs for O(1) lazy cancellation. The `pop_front()` method skips and discards cancelled entries it encounters, which is the mechanism that makes `cancel()` O(1) rather than O(n). This queue is the data structure the future dispatch loop (a later phase) will pop jobs from.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/queue.rs` with `JobQueue` struct and all six methods: `new()`, `push()`, `pop_front()`, `cancel()`, `get()`, `list()`, `len()`.
- Add `uuid` crate dependency (with `v4` feature) to `anvilml-scheduler/Cargo.toml` — needed for `Uuid` type used in `Job.id` and the cancellation `HashSet`.
- Declare `mod queue;` and `pub use queue::JobQueue;` in `crates/anvilml-scheduler/src/lib.rs`.
- Create `crates/anvilml-scheduler/tests/queue_tests.rs` with >=7 integration tests.
- Bump `anvilml-scheduler` patch version from `0.1.7` to `0.1.8`.

### Out of Scope
- DB persistence — handled by P13-B1 (JobStore in anvilml-registry).
- Priority-based ordering — the design doc mentions "sorted by priority+created_at" but the current task context specifies simple FIFO push/pop; priority sorting is deferred to a later task.
- `VramLedger` — handled by P13-A3.
- Dispatch loop — handled in a later phase.

defers_to (from JSON): []

## Existing Codebase Assessment

**What already exists:** `anvilml-scheduler` crate (version 0.1.7) with `lib.rs` (7 lines, re-exports only), `types.rs` (ValidatedGraph newtype + GraphError enum from Phase 12), and `dag.rs` (graph validation). The `tests/` directory exists with `dag_tests.rs` as an integration test file. `Job` from `anvilml-core` is fully defined with all public fields and derives `Clone`/`Debug`/`Serialize`/`Deserialize`, making direct struct-literal construction in tests straightforward.

**Established patterns:** `lib.rs` contains only `pub mod`, `pub use`, and `//!` crate-level doc — no implementation code. Tests go in `crates/{name}/tests/` as separate test crate files (integration style), importing the crate's public API. Doc comments on all `pub` items are mandatory. The `thiserror` crate is used for error enums. The workspace uses `edition.workspace = true` and `rust-version.workspace = true`.

**Gap between design doc and current source:** The design doc §12.1 describes queue.rs as "sorted by priority+created_at" but the task context specifies simple FIFO. The current `lib.rs` has no `mod queue;` declaration yet. No `uuid` dependency exists on `anvilml-scheduler` (it comes transitively through `anvilml-core` but is not directly importable without adding it).

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | uuid    | 1.23.4          | Cargo.lock (project lockfile) | v4 (for test UUID generation) |

Note: `uuid` is already declared in `anvilml-core/Cargo.toml` as `uuid = { version = "1.23.4", features = ["v4", "serde"] }`. This task adds `uuid = { version = "1.23.4", features = ["v4"] }` to `anvilml-scheduler/Cargo.toml` — the `serde` feature is not needed for `JobQueue` itself (it comes via `anvilml-core`'s dependency on `Job`). The `v4` feature is needed for `Uuid::new_v4()` in test code. `VecDeque` and `HashSet` are from the Rust standard library (no external dependency).

## Approach

1. **Add `uuid` dependency to `anvilml-scheduler/Cargo.toml`.** Add `uuid = { version = "1.23.4", features = ["v4"] }` to `[dependencies]`. This gives the crate direct access to `uuid::Uuid` for the `HashSet<Uuid>` in `JobQueue` and `Uuid::new_v4()` in tests.

2. **Create `crates/anvilml-scheduler/src/queue.rs`.** Implement `JobQueue`:
   - `struct JobQueue { jobs: VecDeque<Job>, cancelled: HashSet<Uuid> }` — the `VecDeque` holds jobs in FIFO order; the `HashSet` tracks cancelled IDs for O(1) lookup.
   - `pub fn new() -> Self` — returns empty queue. Add `///` doc comment.
   - `pub fn push(&mut self, job: Job)` — appends to the back of the `VecDeque`. Add `///` doc comment.
   - `pub fn pop_front(&mut self) -> Option<Job>` — loop: peek at front; if the job's `id` is in `cancelled`, remove it and continue; otherwise remove and return `Some(job)`. If the deque is empty, return `None`. The loop naturally handles a run of consecutive cancelled entries. Add `///` doc comment.
   - `pub fn cancel(&mut self, id: Uuid) -> bool` — insert into `cancelled` HashSet; return `true` if the key was not previously present (i.e., it was newly marked), `false` if it was already present. This is O(1) because it only touches the hash set. Add `///` doc comment.
   - `pub fn get(&self, id: Uuid) -> Option<&Job>` — iterate over `jobs` and return a reference to the first job whose `id` matches. Add `///` doc comment.
   - `pub fn list(&self) -> Vec<&Job>` — return a `Vec` of references to all jobs currently in the queue (including cancelled ones that haven't been popped yet — lazy removal means cancelled jobs may still appear in `list()` until `pop_front()` encounters them). Add `///` doc comment.
   - `pub fn len(&self) -> usize` — return `self.jobs.len()`. This counts all jobs including cancelled ones still in the deque. Add `///` doc comment.
   - No `#[tracing::instrument]` needed — this is a pure in-memory data structure with no I/O or async.
   - No logging needed — this is a pure data structure with no side effects.

3. **Update `crates/anvilml-scheduler/src/lib.rs`.** Add `pub mod queue;` and `pub use queue::JobQueue;` after the existing `pub mod dag;` and `pub mod types;` declarations. The file will remain well under 80 lines.

4. **Create `crates/anvilml-scheduler/tests/queue_tests.rs`.** Write >=7 integration tests (see Tests section below). Each test constructs `JobQueue` and `Job` values using the public API. Since `Job` derives `Clone` and has all public fields, test setup uses direct struct literals.

5. **Bump `anvilml-scheduler` version** from `0.1.7` to `0.1.8` in `Cargo.toml` per §12 of ENVIRONMENT.md.

## Public API Surface

All items in `anvilml_scheduler::queue::JobQueue` (module path: `anvilml_scheduler::queue`):

```rust
pub struct JobQueue {
    jobs: VecDeque<Job>,
    cancelled: HashSet<Uuid>,
}

impl JobQueue {
    /// Create a new, empty `JobQueue`.
    pub fn new() -> Self;

    /// Append a job to the back of the queue (FIFO order).
    pub fn push(&mut self, job: Job);

    /// Remove and return the front-most non-cancelled job.
    ///
    /// Lazily discards cancelled entries encountered during the scan.
    /// Returns `None` if the queue is empty or all remaining entries are cancelled.
    pub fn pop_front(&mut self) -> Option<Job>;

    /// Mark a job as cancelled by its ID.
    ///
    /// Returns `true` if the ID was newly marked (not already in the set),
    /// `false` if it was already cancelled. The job remains in the `VecDeque`
    /// until `pop_front()` encounters it.
    pub fn cancel(&mut self, id: Uuid) -> bool;

    /// Return a reference to the job with the given ID, or `None`.
    ///
    /// Searches the entire queue including cancelled entries.
    pub fn get(&self, id: Uuid) -> Option<&Job>;

    /// Return references to all jobs currently in the queue.
    ///
    /// Includes cancelled entries that have not yet been discarded by `pop_front()`.
    pub fn list(&self) -> Vec<&Job>;

    /// Return the number of jobs currently in the queue (including cancelled ones).
    pub fn len(&self) -> usize;
}
```

Re-export in `lib.rs`: `pub use queue::JobQueue;` — so callers use `anvilml_scheduler::JobQueue`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/queue.rs` | `JobQueue` struct and all six methods |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod queue;` and `pub use queue::JobQueue;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `uuid` dependency; bump version 0.1.7 → 0.1.8 |
| CREATE | `crates/anvilml-scheduler/tests/queue_tests.rs` | >=7 integration tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/queue_tests.rs` | `test_fifo_order` | Jobs are returned in push order (FIFO) | Fresh queue | Push Job A, then Job B; call `pop_front()` twice | First pop returns A, second returns B | `cargo test -p anvilml-scheduler --test queue_tests test_fifo_order` exits 0 |
| `tests/queue_tests.rs` | `test_cancel_then_pop_front_skips` | Cancelled jobs are skipped by `pop_front()` (lazy removal) | Fresh queue | Push Job A (id=1), Job B (id=2), cancel(1); pop_front() | First pop returns B (A was skipped), second pop returns None | `cargo test -p anvilml-scheduler --test queue_tests test_cancel_then_pop_front_skips` exits 0 |
| `tests/queue_tests.rs` | `test_cancel_unknown_id_returns_false` | Cancelling a non-existent ID returns false | Fresh queue | cancel(Uuid::new_v4()) | Returns `false` | `cargo test -p anvilml-scheduler --test queue_tests test_cancel_unknown_id_returns_false` exits 0 |
| `tests/queue_tests.rs` | `test_cancel_already_cancelled_returns_false` | Cancelling an already-cancelled ID returns false | Fresh queue | Push Job A, cancel(A), cancel(A) | Second cancel returns `false` | `cargo test -p anvilml-scheduler --test queue_tests test_cancel_already_cancelled_returns_false` exits 0 |
| `tests/queue_tests.rs` | `test_get_returns_job_by_id` | `get()` finds a job by its UUID | Fresh queue | Push Job A, get(A.id) | Returns `Some(&Job)` with matching id | `cargo test -p anvilml-scheduler --test queue_tests test_get_returns_job_by_id` exits 0 |
| `tests/queue_tests.rs` | `test_get_unknown_id_returns_none` | `get()` returns None for unknown ID | Fresh queue | get(Uuid::new_v4()) | Returns `None` | `cargo test -p anvilml-scheduler --test queue_tests test_get_unknown_id_returns_none` exits 0 |
| `tests/queue_tests.rs` | `test_list_returns_all_jobs` | `list()` returns references to all jobs in queue | Fresh queue | Push 3 jobs; call list() | Returns `Vec<&Job>` of length 3 | `cargo test -p anvilml-scheduler --test queue_tests test_list_returns_all_jobs` exits 0 |
| `tests/queue_tests.rs` | `test_len_after_mixed_ops` | `len()` is correct after push/cancel (counts all in VecDeque) | Fresh queue | Push 3 jobs, cancel 1; len() | Returns 3 (cancelled jobs still in deque) | `cargo test -p anvilml-scheduler --test queue_tests test_len_after_mixed_ops` exits 0 |
| `tests/queue_tests.rs` | `test_pop_front_discards_cancelled_and_returns_remaining` | `pop_front()` discards cancelled entries and returns non-cancelled ones | Fresh queue | Push A, B, C; cancel B; pop_front() | Returns B's position is skipped, returns A then C | `cargo test -p anvilml-scheduler --test queue_tests test_pop_front_discards_cancelled_and_returns_remaining` exits 0 |

## CI Impact

No CI changes required. The `rust-linux` and `rust-windows` CI jobs run `cargo test --workspace --features mock-hardware`, which automatically picks up new test files under `crates/anvilml-scheduler/tests/` as they are part of the workspace. Adding the `uuid` dependency does not change any CI job behaviour — it is a path-free registry dependency already resolved in the workspace lockfile.

## Platform Considerations

None identified. `VecDeque`, `HashSet`, and `Uuid` are platform-neutral. The implementation has no `#[cfg(unix)]` or `#[cfg(windows)]` guards. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `uuid` version mismatch — the version in `anvilml-core/Cargo.toml` (1.23.4) may not be the latest on crates.io, but it is the version pinned in this workspace's `Cargo.lock`. If the ACT agent tries to use a different version, `cargo check` will fail due to feature-flag conflicts between duplicate uuid versions. | Low | Medium | Use the exact version string from `anvilml-core/Cargo.toml` (1.23.4). The ACT agent should verify the version against the lockfile before writing. |
| `Job` struct construction in tests — `Job` has many fields (`created_at`, `started_at`, `completed_at`, `worker_id`, `error`, `queue_position`). If any field is missing, compilation fails. | Low | Low | Use `Job { id, status, graph, settings, created_at: chrono::Utc::now(), started_at: None, completed_at: None, worker_id: None, error: None, queue_position: None }` — all non-id fields use defaults or None. The `chrono::Utc::now()` call needs `chrono` as a dev-dependency or through `anvilml-core`. |
| `len()` semantics ambiguity — the task says "len" but doesn't specify whether it counts all jobs or only non-cancelled ones. The design says "sorted by priority+created_at" which implies len might need to be meaningful for dispatch. | Medium | Low | Plan defines `len()` as returning `self.jobs.len()` (all entries including cancelled). This is the natural interpretation — the caller can compare `len()` with `cancelled.len()` if needed. The ACT agent should confirm this matches the task author's intent. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test queue_tests` exits 0 (all 9 tests pass)
- [ ] `wc -l crates/anvilml-scheduler/src/lib.rs` outputs a number ≤ 80
- [ ] `cargo check -p anvilml-scheduler --features mock-hardware` exits 0
- [ ] `cargo clippy -p anvilml-scheduler --features mock-hardware -- -D warnings` exits 0
