# Plan Report: P13-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-A3                                      |
| Phase       | 013 — Job Queue & Persistence               |
| Description | anvilml-scheduler: scheduler.rs JobScheduler submit and persistence |
| Depends on  | P13-A1, P13-A2                              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-19T23:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `JobScheduler` in `crates/anvilml-scheduler/src/scheduler.rs` — the central scheduling object that owns the job queue, VRAM ledger, node registry reference, SQLite database pool, and event broadcaster. Provide `pub async fn submit()` (validate graph → create Job with Queued status → INSERT to SQLite → push to queue → broadcast `WsEvent::JobQueued`), `pub async fn get_job()` (query by UUID from SQLite), and `pub async fn list_jobs()` (query with optional status/limit/before filters). This task establishes the data path for job persistence; the dispatch loop that consumes from the queue is added in Phase 014.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/scheduler.rs` with `JobScheduler` struct and methods:
  - `JobScheduler::new(queue, ledger, node_registry, db, broadcaster)` constructor
  - `pub async fn submit(&self, req: SubmitJobRequest) -> Result<SubmitJobResponse, AnvilError>`
  - `pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError>`
  - `pub async fn list_jobs(&self, status: Option<JobStatus>, limit: Option<u32>, before: Option<DateTime<Utc>>) -> Result<Vec<Job>, AnvilError>`
- Update `crates/anvilml-scheduler/src/lib.rs` to declare `pub mod scheduler; pub use scheduler::JobScheduler;`
- Create `crates/anvilml-scheduler/tests/scheduler_tests.rs` with ≥ 5 tests
- Add `sqlx` dependency to `anvilml-scheduler/Cargo.toml` (runtime dependency for SQLite queries)
- Bump `anvilml-scheduler` crate version from `0.1.6` to `0.1.7`

### Out of Scope
- The dispatch loop that pops jobs from the queue and assigns them to workers (Phase 014)
- HTTP handlers for job endpoints (P13-B1)
- Cancel endpoint wiring
- Job status transitions beyond `Queued` (Phase 014+)
- VRAM reservation logic during submission (the ledger is owned but not yet used for VRAM checks — `would_fit` is called in the dispatch loop, not in submit)

## Existing Codebase Assessment

The `anvilml-scheduler` crate already has three complete modules: `queue.rs` (`JobQueue` with FIFO push/pop/cancel), `ledger.rs` (`VramLedger` with per-device VRAM reservation tracking), and `dag.rs` (`validate_graph` with six independent validation checks plus `ValidatedGraph` newtype). The `types.rs` module defines `GraphError` and re-exports `ValidatedGraph`.

The `lib.rs` declares `pub mod types`, `pub mod dag`, `pub mod ledger`, `pub mod queue`, and re-exports `NodeTypeRegistry` from `anvilml_core`. It does not yet declare `scheduler` — this task adds that module.

Established patterns to follow:
- **Error handling**: Uses `Result<T, AnvilError>` with `?` propagation. `AnvilError` variants used: `Db(sqlx::Error)`, `InvalidGraph(Vec<String>)`, `Internal(String)`.
- **Database pattern**: `anvilml-registry/src/db.rs` shows the pattern — `open_in_memory()` for tests, `sqlx::query` with named parameters, WAL mode enabled by the registry's pool. The `jobs` table schema is defined in `database/migrations/001_initial.sql`.
- **Test style**: Tests in `crates/anvilml-scheduler/tests/` use `#[tokio::test]` for async code, `#[test]` for sync code. Helper functions (like `make_job`) create test fixtures concisely. Tests import types directly from `anvilml_core` and `anvilml_scheduler` modules.
- **Logging**: Uses `tracing::info!` and `tracing::debug!` with structured fields (`key = %value`). Mandatory INFO log points include "Job dispatched", "Job completed", "Job failed". The submit flow needs a "Job queued" or similar INFO log.
- **Async discipline**: Uses `tokio::sync::Mutex` (not `std::sync::Mutex`) for shared state held across await points. The task context explicitly calls for `Arc<Mutex<JobQueue>>` — this must be `tokio::sync::Mutex`.
- **Doc comments**: Every `pub` item has a `///` doc comment describing what it does, arguments, and return values.

The `jobs` table already exists in the migration (created by P13-A1's predecessor work). Columns match `Job` struct fields exactly: `id` (TEXT PK), `status` (TEXT NOT NULL), `graph` (TEXT NOT NULL), `settings` (TEXT NOT NULL), `created_at` (TEXT NOT NULL), `started_at` (TEXT), `completed_at` (TEXT), `worker_id` (TEXT), `error` (TEXT), `queue_position` (INTEGER).

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sqlx    | 0.9.0           | Cargo.lock (workspace) | runtime-tokio, sqlite, json |

**Note:** `sqlx` is already declared in the workspace dependencies at version `0.9.0` with features `["runtime-tokio", "sqlite", "json"]`. This task adds `sqlx` as a direct dependency of `anvilml-scheduler` (it was previously only available transitively through `anvilml-registry`). No new external crates are introduced. The `chrono` dev-dependency already exists in the scheduler's `Cargo.toml` for test use.

## Approach

1. **Add `sqlx` dependency to `anvilml-scheduler/Cargo.toml`.**
   Add `sqlx = { workspace = true }` to the `[dependencies]` section. This is needed for `SqlitePool` and `sqlx::query` in the scheduler. The workspace dependency already has `["runtime-tokio", "sqlite", "json"]` features, which are exactly what this task needs.

2. **Create `crates/anvilml-scheduler/src/scheduler.rs`.**
   Implement the `JobScheduler` struct and its methods:

   ```rust
   pub struct JobScheduler {
       queue: Arc<tokio::sync::Mutex<JobQueue>>,
       ledger: Arc<tokio::sync::Mutex<VramLedger>>,
       node_registry: Arc<NodeTypeRegistry>,
       db: SqlitePool,
       broadcaster: Arc<EventBroadcaster>,
   }
   ```

   Use `tokio::sync::Mutex` (not `std::sync::Mutex`) because the queue and ledger will be held across `.await` points in Phase 014's dispatch loop. This is a hard constraint from the task context.

   **`new()` constructor:** Takes all five fields as arguments. No async work — just stores them. Add `#[tracing::instrument]` for observability.

   **`submit()` method:**
   - Call `validate_graph(&req.graph, &self.node_registry).await` to validate the graph. On `Err`, return `AnvilError::InvalidGraph(errors)`.
   - Generate a new UUID for the job ID: `Uuid::new_v4()`.
   - Create a `Job` struct with `status: JobStatus::Queued`, `created_at: Utc::now()`, and all other fields set to defaults/None. The `queue_position` is set to the current queue length + 1.
   - INSERT the job into SQLite: `INSERT INTO jobs (id, status, graph, settings, created_at, started_at, completed_at, worker_id, error, queue_position) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)` using `sqlx::query` with positional parameters. The `graph` and `settings` fields are serialised as JSON strings via `serde_json::to_string`. Timestamps are serialised as ISO 8601 strings via `chrono`'s `to_rfc3339()`.
   - Push the job to the queue: `self.queue.lock().await.push(job.clone())`.
   - Broadcast the event: `self.broadcaster.broadcast(WsEvent::JobQueued { job_id, queue_position })`.
   - Return `SubmitJobResponse { job_id, queue_position }`.

   **`get_job()` method:**
   - Query `SELECT * FROM jobs WHERE id = ?` using `sqlx::query_as` with the UUID as a hex string.
   - Map the row to a `Job` struct. Since `sqlx::query_as` requires a type that implements `FromRow`, we need a helper struct or use `query` + manual deserialization. Given that `Job` derives `Serialize/Deserialize` but not `FromRow`, use `sqlx::query` with `fetch_optional` and manually construct the `Job` from the row columns.
   - Return `Some(job)` if found, `None` if not found (no error — the handler will translate to 404).

   **`list_jobs()` method:**
   - Build a dynamic SQL query with optional `WHERE status = ?`, `ORDER BY created_at DESC`, `LIMIT ?`, and `WHERE created_at < ?` (before filter).
   - Use `sqlx::query` with `fetch_all` and manually construct `Job` objects from each row.
   - Return the vector of jobs.

3. **Update `crates/anvilml-scheduler/src/lib.rs`.**
   Add `pub mod scheduler;` and `pub use scheduler::JobScheduler;` after the existing module declarations.

4. **Create `crates/anvilml-scheduler/tests/scheduler_tests.rs`.**
   Write ≥ 5 tests covering:
   - Submit valid graph → job persisted with Queued status, queue position 1
   - Submit invalid graph → returns AnvilError::InvalidGraph
   - get_job returns the submitted job
   - list_jobs returns all submitted jobs
   - list_jobs filtered by status returns only matching jobs

   Use `anvilml_registry::open_in_memory()` for the test database pool. Use `NodeTypeRegistry::new().await` for the registry. Populate the registry with at least `LoadModel` so graph validation passes.

5. **Bump `anvilml-scheduler` version** from `0.1.6` to `0.1.7` in `Cargo.toml`.

### Logging

Per ENVIRONMENT.md §9 mandatory log points:
- `submit()`: `tracing::info!(job_id = %job_id, queue_position = pos, "job queued")` — this maps to the "Scheduler: Job dispatched" INFO log point (the task says "notify dispatch" which is the queue notification).
- `get_job()` miss: `tracing::debug!(job_id = %id, "job not found in database")` — per §11.5 mandatory DEBUG log point for scheduler.
- `list_jobs()`: `tracing::debug!(count = jobs.len(), "list jobs returned {} jobs")` — routine debug logging.

Per FORGE_AGENT_RULES §11.6: Apply `#[tracing::instrument]` to `submit()` and `get_job()` — they represent meaningful units of work.

### Documentation

Per FORGE_AGENT_RULES §12: Every `pub` item needs a `///` doc comment. Every decision point in function bodies needs an inline `//` comment explaining why.

## Public API Surface

| Item | Type | Path | Description |
|------|------|------|-------------|
| `JobScheduler` | struct | `anvilml_scheduler::JobScheduler` | Central scheduler owning queue, ledger, registry, DB, broadcaster |
| `JobScheduler::new` | fn | `anvilml_scheduler::JobScheduler::new(queue, ledger, node_registry, db, broadcaster)` | Constructor |
| `JobScheduler::submit` | async fn | `pub async fn submit(&self, req: SubmitJobRequest) -> Result<SubmitJobResponse, AnvilError>` | Validate graph, persist to SQLite, enqueue, broadcast |
| `JobScheduler::get_job` | async fn | `pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError>` | Query job by UUID from SQLite |
| `JobScheduler::list_jobs` | async fn | `pub async fn list_jobs(&self, status: Option<JobStatus>, limit: Option<u32>, before: Option<DateTime<Utc>>) -> Result<Vec<Job>, AnvilError>` | Query jobs with optional filters |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/scheduler.rs` | JobScheduler struct and methods |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod scheduler; pub use scheduler::JobScheduler;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `sqlx` dependency; bump version 0.1.6 → 0.1.7 |
| CREATE | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | ≥ 5 unit tests for scheduler |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_submit_valid_graph` | Submit a valid graph → job persisted in SQLite with Queued status, queue_position=1, and broadcast event sent | In-memory DB with migrations applied; registry populated with LoadModel; valid graph JSON | Valid graph with LoadModel node | `Ok(SubmitJobResponse { job_id, queue_position: 1 })`; `get_job(job_id)` returns `Some(job)` with status `Queued` | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_submit_invalid_graph` | Submit a graph with unknown node type → returns `AnvilError::InvalidGraph` | Registry populated with LoadModel only; graph references "NonExistent" | Invalid graph | `Err(AnvilError::InvalidGraph(_))`; job NOT persisted in SQLite | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_get_job_returns_job` | `get_job()` returns the correct job for a valid UUID | One job submitted via `submit()` | UUID of submitted job | `Ok(Some(job))` with matching id and status `Queued` | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_get_job_missing_returns_none` | `get_job()` returns `None` for a UUID that was never submitted | No jobs in DB | Random UUID | `Ok(None)` — no error, just not found | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_list_jobs_returns_all` | `list_jobs()` returns all submitted jobs | Three jobs submitted via `submit()` | No filters | `Ok(vec![job1, job2, job3])` with length 3 | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_list_jobs_filter_by_status` | `list_jobs(status=Some(Queued))` returns only Queued jobs | Three jobs submitted; one manually updated to Running in DB | `status=Some(JobStatus::Queued)` | `Ok(vec![...])` with only Queued jobs | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 |

## CI Impact

No CI changes required. The new `scheduler_tests.rs` file is picked up automatically by `cargo test --workspace --features mock-hardware`. No new file type, gate, or test module requires CI configuration changes. The `sqlx` dependency is a runtime dependency (not a dev-only one), but it uses the same workspace feature flags already configured for CI.

## Platform Considerations

None identified. The SQLite operations use `sqlx` which is cross-platform (works on Linux, Windows, macOS). The `jobs` table uses TEXT for timestamps (ISO 8601) and TEXT for UUIDs (hex strings), avoiding platform-specific path or encoding issues. The `#[cfg(unix)]` / `#[cfg(windows)]` guards are not needed — all code paths are platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `Job` struct does not implement `sqlx::FromRow`, requiring manual column-to-field mapping for every query. This increases code volume and risk of column-name mismatches between the migration schema and the Rust struct. | High | Medium | Use `sqlx::query` (not `query_as`) with manual row-to-struct conversion. Map each column by name using `.try_get::<_, String>("column_name")`. Write a helper function `fn row_to_job(row: &sqlx::postgres::PgRow) -> Job` (adapted for SQLite) to centralise the mapping logic. Test the mapping explicitly in `test_submit_valid_graph` by asserting on all fields. |
| `sqlx::query` with positional parameters requires type conversions that may fail silently or at runtime rather than compile time (since `sqlx`'s compile-time checking requires `--offline` mode or a live database). Column name mismatches won't be caught. | Medium | High | Use named parameters (`WHERE id = :id`) with `sqlx::query` to get compile-time column name checking. The `sqlx` macro-based approach catches typos at build time. Test with a live in-memory database (`open_in_memory()`) which applies all migrations, so column names match exactly. |
| The `EventBroadcaster` type is re-exported from `anvilml_ipc`, not defined in `anvilml_scheduler`. Adding `anvilml-ipc` as a dependency would create a cycle: `anvilml-scheduler` depends on `anvilml-worker` which depends on `anvilml-ipc`. Need to verify the correct import path. | Low | Medium | `EventBroadcaster` is `pub use anvilml_ipc::EventBroadcaster` in `anvilml-ipc/src/lib.rs`. Since `anvilml-scheduler` already depends on `anvilml-worker` (which depends on `anvilml-ipc`), we can access it via `anvilml_ipc::EventBroadcaster` — but this would create a direct dependency on `anvilml-ipc` which is allowed (the dependency graph permits it). Alternatively, re-export it from `anvilml_core` if it exists there. Check: `EventBroadcaster` is in `anvilml_ipc`, not `anvilml_core`. Since the scheduler already imports from `anvilml_core::NodeTypeRegistry` (re-exported), we can add `anvilml-ipc` as a direct dependency. The dependency graph: `anvilml-scheduler → anvilml-worker → anvilml-ipc` is fine; adding `anvilml-scheduler → anvilml-ipc` directly does not create a cycle. |
| The `queue_position` field in `Job` is `Option<u32>` but the task says to set it at submit time. After dispatch (Phase 014), it becomes `None`. The SQLite column is `INTEGER`. Need to handle `None` → SQL NULL mapping correctly. | Low | Low | Use `queue_position.map(|p| p as i64)` in the INSERT query to convert `Option<u32>` to `Option<i64>`, which `sqlx` maps to SQL NULL automatically. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler` exits 0 with ≥ 5 tests
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (all tests, not just scheduler)
- [ ] `head -1 .forge/reports/P13-A3_plan.md` prints `# Plan Report: P13-A3`
- [ ] `grep "^## " .forge/reports/P13-A3_plan.md` shows 12 section headings
- [ ] `wc -l .forge/reports/P13-A3_plan.md` returns > 40 lines
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (no formatting drift)
