# Plan Report: P14-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-A1                                      |
| Phase       | 14 — Dispatch & Execute                     |
| Description | anvilml-scheduler: JobScheduler struct + submit() |
| Depends on  | P13-A1, P13-A2, P13-A3, P12-A1, P12-A2, P12-A3 |
| Project     | anvilml                                     |
| Planned at  | 2026-07-07T12:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the `JobScheduler` struct and its `submit()` method in `crates/anvilml-scheduler/src/scheduler.rs`. The scheduler owns an in-memory `JobQueue`, a `VramLedger`, a `JobStore` for persistence, a `NodeTypeRegistry` for graph validation, and a `Notify` for waking the dispatch loop. `submit()` enforces the "no workers = reject" guard (empty registry → `WorkersUnavailable` at 503), validates the graph, constructs a `Queued` job, persists it via `JobStore::upsert()`, enqueues it, notifies the dispatch loop, and returns the job ID. This is the first concrete async method in the scheduler crate and establishes the pattern for all subsequent scheduler methods.

## Scope

### In Scope
- `crates/anvilml-scheduler/src/scheduler.rs` — new file: `JobScheduler` struct, `new()` constructor, `submit()` method.
- `crates/anvilml-scheduler/src/lib.rs` — add `pub mod scheduler;` and `pub use scheduler::JobScheduler;`.
- `crates/anvilml-scheduler/Cargo.toml` — add `tokio` dependency (for `tokio::sync::Mutex` and `tokio::sync::Notify`).
- `crates/anvilml-scheduler/tests/scheduler_tests.rs` — ≥4 integration tests.

### Out of Scope
- `cancel()` and `get_job()` — deferred to P14-A2 (confirmed: P14-A2's description states "Complete the scheduler's read/cancel surface, including the authoritative lookup path for jobs that have already left the in-memory queue").
- The dispatch loop (`start_dispatch_loop()`, `dispatch_one()`) — deferred to P14-A3.
- Worker selection algorithm — deferred to P14-A4.
- Marking assigned worker Busy — deferred to P14-A5.

## Existing Codebase Assessment

The `anvilml-scheduler` crate already has three fully-implemented modules from Phase 12 and 13: `queue.rs` (JobQueue with FIFO push/pop/cancel), `ledger.rs` (VramLedger with per-device reserve/release/free_mib), and `dag.rs` (validate_graph with collect-all-errors DAG validation). The `types.rs` module provides `ValidatedGraph` (construction-gated newtype) and `GraphError` enum. No `scheduler.rs` exists yet — this task creates it from scratch.

The established patterns across all existing modules are:
- **Naming**: `pub fn method_name(&self, ...)` or `&mut self` for mutators; `snake_case` throughout.
- **Error handling**: Returns `Result<T, AnvilError>`; uses `?` for propagation; no `.unwrap()` in non-test code.
- **Logging**: `#[tracing::instrument]` on public async methods; structured fields (`field = %value`); `DEBUG` for internal state changes; `INFO` for lifecycle events.
- **Documentation**: Every `pub` item has a `///` doc comment with description, arguments, and return/error info.
- **Test placement**: Integration tests in `tests/*.rs` as separate test crates (not inline `#[cfg(test)]`).

The design doc (§12.1) specifies `tokio::sync::Mutex` for `queue` and `ledger` because they are held across `.await` points during `job_store.upsert()`. This is a hard requirement, not style — confirmed by `ANVILML_DESIGN.md §4.7` and `TASKS_PHASE014.md`'s known constraints.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP  | sync, macros           |

The `tokio` version 1.52.3 matches the version used by `anvilml-worker` (the crate's transitive dependency). This ensures a single version of tokio across the workspace, avoiding duplicate compilation. Features needed: `sync` (for `Mutex` and `Notify`), `macros` (for `#[tokio::test]` in tests). The `rt-multi-thread` feature is not needed here — the scheduler methods are called from within an already-running multi-thread runtime; they don't spawn their own runtime.

## Approach

1. **Add `tokio` dependency to `Cargo.toml`**. Add `tokio = { version = "1.52.3", features = ["sync", "macros"] }` under `[dependencies]`. This is the minimum feature set: `sync` provides `Mutex` and `Notify`; `macros` enables `#[tokio::test]` for the test crate.

2. **Create `scheduler.rs`** with the `JobScheduler` struct and its methods:

   **Struct definition:**
   ```rust
   pub struct JobScheduler {
       queue: tokio::sync::Mutex<JobQueue>,
       ledger: tokio::sync::Mutex<VramLedger>,
       job_store: Arc<JobStore>,
       node_registry: Arc<NodeTypeRegistry>,
       dispatch_notify: Arc<Notify>,
   }
   ```
   - `tokio::sync::Mutex` for `queue` and `ledger` (held across `.await` during `job_store.upsert()` — hard requirement per §4.7 of ANVILML_DESIGN.md).
   - `std::sync::Arc` for `job_store` and `node_registry` (shared references, no interior mutability needed at this level — the mutexes protect the internal state).
   - `Arc<Notify>` for waking the dispatch loop — `Notify::notify_one()` is called after each `submit()`.

   **Constructor `new()`:**
   ```rust
   pub fn new(
       job_store: JobStore,
       node_registry: Arc<NodeTypeRegistry>,
   ) -> Self {
       Self {
           queue: tokio::sync::Mutex::new(JobQueue::new()),
           ledger: tokio::sync::Mutex::new(VramLedger::new()),
           job_store: Arc::new(job_store),
           node_registry,
           dispatch_notify: Arc::new(Notify::new()),
       }
   }
   ```
   - Takes ownership of `JobStore` (wraps it in `Arc` for sharing).
   - Creates fresh `JobQueue` and `VramLedger` instances inside `tokio::sync::Mutex`.
   - Takes `Arc<NodeTypeRegistry>` by value (the caller constructs it).
   - Creates a fresh `Notify`.

   **`submit()` method:**
   ```rust
   pub async fn submit(
       &self,
       graph: serde_json::Value,
       settings: JobSettings,
   ) -> Result<Uuid, AnvilError>
   ```
   Implementation steps:
   a. **Workers-available check**: `if self.node_registry.is_empty() { return Err(AnvilError::WorkersUnavailable("no workers registered".into())); }` — per §12.2 of ANVILML_DESIGN.md, an empty registry means no worker has reached Ready, so reject before any other work. This is the first operation, before validation.

   b. **Graph validation**: Call `validate_graph(graph, &self.node_registry)`. On error, return `Err(AnvilError::InvalidGraph(...))` by converting the `Vec<GraphError>` into the required format.

   c. **Construct the Job**: Generate a new UUID v4 for the job ID. Create a `Job` with:
      - `id = Uuid::new_v4()`
      - `status = JobStatus::Queued`
      - `graph` = the validated graph (from `ValidatedGraph::0`)
      - `settings` = passed-in settings
      - `created_at = Utc::now()`
      - `started_at`, `completed_at`, `worker_id`, `error` = `None`
      - `queue_position = Some(1)` (first position; will be updated by dispatch loop or subsequent jobs)

   d. **Persist the job**: `self.job_store.upsert(&job).await?` — this is an async operation that acquires a database connection. The `tokio::sync::Mutex` on `queue` must be held across this await because we need to push the job to the in-memory queue atomically after persisting.

   e. **Enqueue the job**: Acquire the queue mutex lock and call `queue.push(job)`.

   f. **Notify the dispatch loop**: `self.dispatch_notify.notify_one()` — wakes a single waiter (the dispatch loop task, when it exists in a later phase).

   g. **Return the job ID**: `Ok(job.id)`.

   The critical sequencing is: validate → construct → persist (async) → enqueue → notify. The queue mutex must be held across the `upsert` await to prevent a race where the dispatch loop (in a future phase) pops the job before it's been enqueued.

3. **Update `lib.rs`** to export the new module:
   - Add `pub mod scheduler;`
   - Add `pub use scheduler::JobScheduler;`

4. **Create integration tests** in `crates/anvilml-scheduler/tests/scheduler_tests.rs`:
   - Test 1: Empty registry returns `WorkersUnavailable` — construct scheduler with empty `NodeTypeRegistry`, call `submit()`, assert error variant.
   - Test 2: Invalid graph returns validation error — populate registry with at least one node type, submit a graph with an unknown node type, assert `AnvilError::InvalidGraph` (not a panic).
   - Test 3: Valid submission persists and queues — populate registry, submit valid graph, assert `Ok(id)`, then verify the job exists in the queue via `queue.lock().await.get(id)` and that it was persisted.
   - Test 4: Two submits get distinct IDs — submit two valid graphs, assert returned IDs are different (`id1 != id2`).

   All tests use `#[tokio::test]` for async test support. Database operations use an in-memory SQLite pool (created via `sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(...)` with `sqlite::memory:`). The `JobStore` wraps this pool.

   The `test-util` feature (already declared in Cargo.toml) provides `ValidatedGraph::_test_new()` for constructing test graphs.

5. **Version bump**: Increment `anvilml-scheduler` patch version from `0.1.9` to `0.1.10` in `Cargo.toml`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `struct JobScheduler` | `anvilml_scheduler::JobScheduler` | `pub struct JobScheduler { queue: tokio::sync::Mutex<JobQueue>, ledger: tokio::sync::Mutex<VramLedger>, job_store: Arc<JobStore>, node_registry: Arc<NodeTypeRegistry>, dispatch_notify: Arc<Notify> }` |
| `fn new` | `anvilml_scheduler::JobScheduler::new` | `pub fn new(job_store: JobStore, node_registry: Arc<NodeTypeRegistry>) -> Self` |
| `fn submit` | `anvilml_scheduler::JobScheduler::submit` | `pub async fn submit(&self, graph: serde_json::Value, settings: JobSettings) -> Result<Uuid, AnvilError>` |

No other public items are introduced. `cancel()` and `get_job()` are deferred to P14-A2.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/scheduler.rs` | `JobScheduler` struct, `new()`, `submit()` |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod scheduler;` and `pub use scheduler::JobScheduler;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `tokio` dependency; bump patch version `0.1.9 → 0.1.10` |
| CREATE | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | ≥4 integration tests for `submit()` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/scheduler_tests.rs` | `test_submit_empty_registry_returns_workers_unavailable` | An empty `NodeTypeRegistry` causes `submit()` to return `WorkersUnavailable` | Scheduler constructed with empty registry | Valid graph JSON, empty settings | `Err(AnvilError::WorkersUnavailable(_))` | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_submit_empty_registry_returns_workers_unavailable` exits 0 |
| `tests/scheduler_tests.rs` | `test_submit_invalid_graph_returns_validation_error` | An invalid graph (unknown node type) returns `InvalidGraph` error, not a panic | Scheduler with one registered node type; graph references unknown type | Graph with node type `"NonExistentNode"` | `Err(AnvilError::InvalidGraph(_))` | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_submit_invalid_graph_returns_validation_error` exits 0 |
| `tests/scheduler_tests.rs` | `test_submit_valid_persists_and_queues` | A valid submission returns `Ok(id)`, the job is persisted to DB, and enqueued in memory | Scheduler with registered node type; valid PassThrough graph | Valid graph JSON | `Ok(uuid)`; job retrievable from queue and DB | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_submit_valid_persists_and_queues` exits 0 |
| `tests/scheduler_tests.rs` | `test_two_submits_get_distinct_ids` | Two sequential submissions produce different UUIDs | Scheduler with registered node type; two valid graphs | Two valid graph JSONs | `id1 != id2` | `cargo test -p anvilml-scheduler --test scheduler_tests -- test_two_submits_get_distinct_ids` exits 0 |

## CI Impact

The `rust-linux` and `rust-windows` CI jobs run `cargo test --workspace --features mock-hardware`, which includes `crates/anvilml-scheduler`. Adding a new test binary (`scheduler_tests`) under `tests/` is automatically picked up by `cargo test` — no CI configuration changes required. The new `tokio` dependency is a transitive dependency already used by `anvilml-worker`, so it will resolve to the same version already in `Cargo.lock`.

## Platform Considerations

None identified. The `tokio::sync::Mutex` and `tokio::sync::Notify` types are cross-platform abstractions with no `#[cfg(unix)]` or `#[cfg(windows)]` guards needed. The in-memory SQLite pool used in tests (`sqlite::memory:`) works identically on Linux and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobStore::upsert()` requires a `SqlitePool` with migrations applied — tests that create a fresh in-memory pool may fail if the `jobs` table doesn't exist. | Medium | High | Use `sqlx::migrate!("./../../database/migrations")` or equivalent to run migrations before creating the `JobStore`. This is the same pattern used by `anvilml-registry`'s `db.rs::create_pool()`. |
| The `tokio::sync::Mutex` on `queue` being held across the `job_store.upsert().await` call could cause contention if the dispatch loop (P14-A3) also tries to acquire the queue lock concurrently. | Low | Medium | This is by design — the mutex protects the invariant that a job is only popped after it has been enqueued. The dispatch loop will acquire the same lock. Correctness over performance; the lock is held briefly (one push + one notify). |
| `Utc::now()` in tests is not deterministic — two rapid submissions might produce identical `created_at` values, potentially causing issues if tests rely on ordering. | Low | Low | The test does not assert on `created_at` ordering; it only asserts on ID uniqueness and job presence. If needed, `chrono::Utc::now()` resolution is nanosecond-level, making collisions effectively impossible in sequential test execution. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests` exits 0
- [ ] `cargo clippy --package anvilml-scheduler --features mock-hardware -- -D warnings` exits 0 (zero warnings)
- [ ] `cargo check --package anvilml-scheduler --features mock-hardware` exits 0 (compiles cleanly)
