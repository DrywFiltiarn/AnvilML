# Plan Report: P13-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-B1                                        |
| Phase       | 13 — Job Queue                                  |
| Description | anvilml-registry: JobStore CRUD, ghost-job reset on startup |
| Depends on  | P13-A1, P6-A9                                   |
| Project     | anvilml                                         |
| Planned at  | 2026-07-07T09:45:00Z                            |
| Attempt     | 1                                               |

## Objective

Implement `JobStore` — the SQLite-backed persistence layer for `Job` records — in the
`anvilml-registry` crate. This provides four operations: `upsert()` to persist a job
(serializing its `graph` and `settings` to JSON TEXT columns), `get()` to fetch a single
job by UUID, `list()` to query jobs with optional status filter and limit, and
`reset_ghost_jobs()` to transition stale `Queued`/`Running` jobs to `Failed` with
`error = "server_restart"` per `ANVILML_DESIGN.md §19.2`. The `jobs` table migration
(003_jobs.sql) is already in place from P13-A1, and the `Job`, `JobStatus`, `JobSettings`
types already exist in `anvilml-core`. This task places persistence in `anvilml-registry`
(mirroring the `ModelStore`/`ModelScanner` split), not in `anvilml-scheduler`.

## Scope

### In Scope
- `crates/anvilml-registry/src/job_store.rs` — `JobStore` struct with `upsert()`, `get()`,
  `list()`, `reset_ghost_jobs()` methods.
- `crates/anvilml-registry/src/lib.rs` — add `pub mod job_store;` and `pub use job_store::JobStore;`.
- `crates/anvilml-registry/Cargo.toml` — bump patch version `0.1.6 → 0.1.7`; add `uuid` to
  main dependencies (with `serde` feature) for `Uuid` deserialization from SQL TEXT.
- `crates/anvilml-registry/tests/job_store_tests.rs` — ≥6 integration tests covering
  upsert+get roundtrip, list with status filter, reset_ghost_jobs correctness, and empty table.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope in full.

## Existing Codebase Assessment

**(a) What already exists:** The `jobs` table migration (`database/migrations/003_jobs.sql`)
already creates the table with columns matching the `Job` struct fields. The `Job`,
`JobStatus`, and `JobSettings` types exist in `anvilml-core/src/types/job.rs` with full
serde derives (`Serialize`, `Deserialize`, `ToSchema`). The `ModelStore` in
`anvilml-registry/src/store.rs` provides the exact pattern to follow: a struct wrapping
`SqlitePool`, async methods using `sqlx::query!`/`sqlx::query_as!`, and a helper row
struct for deserialization. The `create_pool()` function in `db.rs` already runs migrations
and enables WAL mode.

**(b) Established patterns:** `ModelStore` uses `INSERT OR REPLACE` for upsert, `fetch_optional`
for single-get, `fetch_all` with optional WHERE for list. Enum variants are stored as
snake_case TEXT (via `serde_json::to_string` + `trim_matches('"')`) and read back via
`serde_json::from_str::<T>(&format!("\"{}\"", row.text))`. Timestamps are stored as RFC 3339
strings and parsed with `DateTime::parse_from_rfc3339()`. All public methods use
`#[tracing::instrument]` with span fields. `AnvilError::Db` is the error variant for
SQL failures (auto-converted via `#[from]`).

**(c) Gap between design doc and current source:** The design doc (§19.2) specifies the
ghost-job reset behavior precisely: `Queued`/`Running` → `Failed` with literal string
`"server_restart"`. No implementation exists yet — this task is the first to implement it.
The `jobs` table already has an `error` TEXT column, so no schema changes are needed.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sqlx    | 0.9.0           | rust-docs MCP  | sqlite, runtime-tokio, migrate, chrono |
| crate  | uuid    | 1.23            | rust-docs MCP  | serde (for Uuid deserialization from SQL TEXT) |
| crate  | serde_json | 1.0          | rust-docs MCP  | n/a |
| crate  | chrono  | 0.4             | rust-docs MCP  | serde |

No new external crates are introduced. `uuid` is already a dev-dependency but needs to be
promoted to a main dependency with the `serde` feature so that `Uuid` can be deserialized
from SQL TEXT columns during `get()` and `list()`.

## Approach

1. **Add `uuid` with `serde` feature to `anvilml-registry`'s main dependencies** in
   `Cargo.toml`. The `Job` struct's `id: Uuid` field must be deserialized from the `id`
   TEXT column in the `jobs` table. The `sqlx::FromRow` derive on the helper struct needs
   `uuid` with `serde` feature to map SQL TEXT → `Uuid`.

2. **Create `crates/anvilml-registry/src/job_store.rs`** with the following structure:
   - Module-level doc comment describing `JobStore` as the SQLite-backed persistence for
     `Job` records, mirroring the `ModelStore` pattern.
   - `JobStore { pool: SqlitePool }` struct.
   - `impl JobStore` block with:
     - `pub fn new(pool: SqlitePool) -> Self` — constructor (same pattern as `ModelStore::new`).
     - `pub async fn upsert(&self, job: &Job) -> Result<(), AnvilError>` — serialize
       `graph` and `settings` via `serde_json::to_string`, serialize `status` via
       `serde_json::to_string` + `trim_matches('"')`, format timestamps via
       `to_rfc3339()`, then `INSERT OR REPLACE INTO jobs (...) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
       with all fields bound. Include `#[tracing::instrument]` with `fields(id = %job.id)`.
       Log `tracing::debug!(id = %job.id, "upserted job");` on success.
     - `pub async fn get(&self, id: Uuid) -> Result<Option<Job>, AnvilError>` — query
       `SELECT * FROM jobs WHERE id = ?`, bind the UUID, use `fetch_optional` with a
       `JobRow` helper struct, convert to `Job` via a private `row_to_job()` method.
       Include `#[tracing::instrument]` with `fields(id = %id)`.
     - `pub async fn list(&self, status: Option<JobStatus>, limit: Option<u32>) -> Result<Vec<Job>, AnvilError>` —
       build query dynamically: start with `SELECT * FROM jobs`, append `WHERE status = ?`
       if `status.is_some()`, append `ORDER BY created_at ASC LIMIT ?` if `limit.is_some()`.
       Use `fetch_all` with `JobRow` helper, convert via `row_to_job()`. Include
       `#[tracing::instrument]`.
     - `pub async fn reset_ghost_jobs(&self) -> Result<u32, AnvilError>` — per §19.2:
       execute `UPDATE jobs SET status = 'failed', error = 'server_restart' WHERE status IN ('queued', 'running')`,
       then `SELECT changes()` to get the affected count (or use `execute` return count).
       Log `tracing::info!(count = affected, "ghost jobs reset to failed");` if `count > 0`.
       Include `#[tracing::instrument]`.
   - Private `JobRow` helper struct with `#[derive(sqlx::FromRow)]` matching all columns
     of the `jobs` table (same pattern as `ModelMetaRow`). Fields: `id: String`,
     `status: String`, `graph: String`, `settings: String`, `created_at: String`,
     `started_at: Option<String>`, `completed_at: Option<String>`, `worker_id: Option<String>`,
     `error: Option<String>`, `queue_position: Option<i64>`.
   - Private `row_to_job(&self, row: JobRow) -> Job` method — converts raw SQL row to `Job`:
     parse `id` via `Uuid::parse_str`, parse `status` via `serde_json::from_str` from the
     text value, parse `graph` via `serde_json::from_str::<serde_json::Value>`, parse
     `settings` via `serde_json::from_str::<JobSettings>`, parse timestamps via
     `DateTime::parse_from_rfc3339()`.

3. **Update `crates/anvilml-registry/src/lib.rs`** — add `pub mod job_store;` and
   `pub use job_store::JobStore;` after the existing module declarations.

4. **Bump `anvilml-registry` version** in `Cargo.toml`: `0.1.6 → 0.1.7`.

5. **Create `crates/anvilml-registry/tests/job_store_tests.rs`** — ≥6 integration tests
   using the same `make_pool()` pattern from `store_tests.rs` (unique in-memory SQLite
   with migrations applied). Tests:
   - `test_upsert_get_roundtrip` — insert a job, fetch it, assert all fields match.
   - `test_list_no_filter` — insert multiple jobs, list without filter, assert count.
   - `test_list_with_status_filter` — insert mixed-status jobs, filter by one status,
     assert only matching jobs returned.
   - `test_reset_ghost_jobs_queued_becomes_failed` — insert a `Queued` job, call
     `reset_ghost_jobs()`, verify it's now `Failed` with `error = "server_restart"`.
   - `test_reset_ghost_jobs_running_becomes_failed` — insert a `Running` job, call
     `reset_ghost_jobs()`, verify it's now `Failed` with `error = "server_restart"`.
   - `test_reset_ghost_jobs_completed_not_affected` — insert a `Completed` job and a
     `Queued` job, call `reset_ghost_jobs()`, verify only the `Queued` one changed.
   - `test_reset_ghost_jobs_empty_table` — call on empty table, assert return count is 0.
   - `test_get_missing_id_returns_none` — query nonexistent ID, assert `None`.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `JobStore` struct | `anvilml-registry::job_store::JobStore` | `pub struct JobStore { pool: SqlitePool }` |
| `JobStore::new` | `anvilml_registry::JobStore` | `pub fn new(pool: SqlitePool) -> Self` |
| `JobStore::upsert` | `anvilml_registry::JobStore` | `pub async fn upsert(&self, job: &Job) -> Result<(), AnvilError>` |
| `JobStore::get` | `anvilml_registry::JobStore` | `pub async fn get(&self, id: Uuid) -> Result<Option<Job>, AnvilError>` |
| `JobStore::list` | `anvilml_registry::JobStore` | `pub async fn list(&self, status: Option<JobStatus>, limit: Option<u32>) -> Result<Vec<Job>, AnvilError>` |
| `JobStore::reset_ghost_jobs` | `anvilml_registry::JobStore` | `pub async fn reset_ghost_jobs(&self) -> Result<u32, AnvilError>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/job_store.rs` | `JobStore` struct with upsert/get/list/reset_ghost_jobs |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod job_store;` and `pub use job_store::JobStore;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Add `uuid` dep with `serde` feature; bump version `0.1.6 → 0.1.7` |
| CREATE | `crates/anvilml-registry/tests/job_store_tests.rs` | ≥6 integration tests for JobStore |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `job_store_tests.rs` | `test_upsert_get_roundtrip` | Insert a `Job` with all fields populated, then `get()` returns identical values; graph/settings JSON roundtrips correctly through TEXT columns. | `cargo test -p anvilml-registry --test job_store_tests test_upsert_get_roundtrip` exits 0 |
| `job_store_tests.rs` | `test_list_no_filter` | Insert 3 jobs with different statuses, `list(None, None)` returns all 3. | `cargo test -p anvilml-registry --test job_store_tests test_list_no_filter` exits 0 |
| `job_store_tests.rs` | `test_list_with_status_filter` | Insert 5 jobs across 3 statuses, `list(Some(Queued), None)` returns exactly the Queued ones. | `cargo test -p anvilml-registry --test job_store_tests test_list_with_status_filter` exits 0 |
| `job_store_tests.rs` | `test_list_with_limit` | Insert 5 jobs, `list(None, Some(2))` returns at most 2 rows. | `cargo test -p anvilml-registry --test job_store_tests test_list_with_limit` exits 0 |
| `job_store_tests.rs` | `test_reset_ghost_jobs_queued_becomes_failed` | Insert a `Queued` job, call `reset_ghost_jobs()`, verify status is `Failed` and error is `"server_restart"`. | `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_queued_becomes_failed` exits 0 |
| `job_store_tests.rs` | `test_reset_ghost_jobs_running_becomes_failed` | Insert a `Running` job, call `reset_ghost_jobs()`, verify status is `Failed` and error is `"server_restart"`. | `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_running_becomes_failed` exits 0 |
| `job_store_tests.rs` | `test_reset_ghost_jobs_completed_not_affected` | Insert `Completed`, `Cancelled`, and `Queued` jobs; call `reset_ghost_jobs()`; verify only `Queued` changed, others untouched. | `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_completed_not_affected` exits 0 |
| `job_store_tests.rs` | `test_reset_ghost_jobs_empty_table` | Call `reset_ghost_jobs()` on empty table, return value is 0. | `cargo test -p anvilml-registry --test job_store_tests test_reset_ghost_jobs_empty_table` exits 0 |
| `job_store_tests.rs` | `test_get_missing_id_returns_none` | Query nonexistent UUID, `get()` returns `Ok(None)`. | `cargo test -p anvilml-registry --test job_store_tests test_get_missing_id_returns_none` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-registry/tests/` which
is automatically picked up by `cargo test --workspace --features mock-hardware` (the
existing CI test command). The `uuid` dependency is already used as a dev-dependency;
promoting it to a main dependency does not change the dependency graph for CI.

## Platform Considerations

None identified. The `jobs` table uses TEXT for all string fields (UUID, status, graph,
settings, timestamps, error), which are platform-neutral. The `queue_position` INTEGER
column is also platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards needed.
The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx::FromRow` on `JobRow` may require `uuid` with `serde` feature to deserialize TEXT → `Uuid`; without it, sqlx cannot map the `id` TEXT column. | Medium | High | Add `uuid = { version = "1.23", features = ["serde"] }` to main dependencies (not just dev-dependencies). Verify via `cargo check` before writing tests. |
| `graph` and `settings` TEXT columns may contain embedded quotes or unicode from `serde_json::to_string`, causing SQL syntax errors if not properly parameterized. | Low | Medium | Use sqlx parameterized queries (`.bind()`) rather than string interpolation — the `INSERT OR REPLACE INTO jobs ... VALUES (?, ?, ...)` pattern already handles this correctly. |
| `reset_ghost_jobs()` using `UPDATE ... WHERE status IN (...)` then `SELECT changes()` may have a race condition if another transaction modifies rows concurrently. | Low | Medium | SQLite's WAL mode with default isolation (SERIALIZABLE-equivalent for single-writer) prevents concurrent writes; the in-memory test pool uses `max_connections(1)` which eliminates this entirely. |
| `list()` query building with optional WHERE and ORDER BY/LIMIT may produce invalid SQL if `limit` is `Some(0)`. | Low | Low | Add a guard: if `limit == Some(0)`, return early with empty vec. Document the behavior. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-registry --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-registry --test job_store_tests` exits 0 (≥8 tests)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regression)
- [ ] `wc -l crates/anvilml-registry/src/lib.rs` reports ≤ 80 lines
- [ ] `grep "^pub " crates/anvilml-registry/src/lib.rs` contains `pub mod job_store;` and `pub use job_store::JobStore;`
- [ ] `grep -rn "pub struct JobStore" crates/anvilml-registry/src/job_store.rs` finds the struct definition
- [ ] `grep -rn "pub async fn upsert" crates/anvilml-registry/src/job_store.rs` finds the upsert method
- [ ] `grep -rn "pub async fn get" crates/anvilml-registry/src/job_store.rs` finds the get method
- [ ] `grep -rn "pub async fn list" crates/anvilml-registry/src/job_store.rs` finds the list method
- [ ] `grep -rn "pub async fn reset_ghost_jobs" crates/anvilml-registry/src/job_store.rs` finds the reset_ghost_jobs method
- [ ] `grep '"server_restart"' crates/anvilml-registry/src/job_store.rs` finds the literal error string in reset_ghost_jobs
