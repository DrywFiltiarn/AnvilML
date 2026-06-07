# Plan Report: P12-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-A1                                      |
| Phase       | 012 — Job Submission & Queue                |
| Description | anvilml-scheduler: job DB row helpers (insert, get, list, update status) |
| Depends on  | P11-A5                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-07T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Add sqlx, uuid, and chrono dependencies to `anvilml-scheduler`, then create `src/job_store.rs` with four async functions — `insert_job`, `get_job`, `list_jobs`, and `update_status` — that perform CRUD operations on the `jobs` SQLite table. Map `graph` and `settings` as TEXT JSON columns. Ship unit tests using a tempfile-backed SQLite database exercising insert, get, list-with-filters, and status-update paths.

## Scope

### In Scope
- Add `sqlx`, `uuid`, and `chrono` to `anvilml-scheduler/Cargo.toml` (pull versions from workspace `[workspace.dependencies]`)
- Create `crates/anvilml-scheduler/src/job_store.rs` with:
  - `pub async fn insert_job(pool: &SqlitePool, job: &Job) -> Result<(), AnvilError>` — INSERT row, map Job fields to columns
  - `pub async fn get_job(pool: &SqlitePool, id: Uuid) -> Result<Option<Job>, AnvilError>` — SELECT by id
  - `pub async fn list_jobs(pool: &SqlitePool, status: Option<JobStatus>, limit: u32, before: Option<DateTime<Utc>>) -> Result<Vec<Job>, AnvilError>` — SELECT with optional status filter, limit clamp (default 100, max 1000), `created_at < before` cursor
  - `pub async fn update_status(pool: &SqlitePool, id: Uuid, status: JobStatus, error: Option<&str>) -> Result<(), AnvilError>` — UPDATE SET status/error/timestamps
- Register the module in `lib.rs` (`pub mod job_store`) and re-export functions
- Add dev-dependencies (`serial_test`, `tempfile`) to Cargo.toml
- Create `tests/job_store_tests.rs` (or inline `#[cfg(test)]` module) with:
  - Helper that creates a tempfile-based `SqlitePool` (using `sqlite::memory:` or a temp file path)
  - Test: insert a job → get_job returns it
  - Test: insert multiple jobs → list_jobs filters by status, respects limit and before cursor
  - Test: update_status transitions status and updates error/timestamps

### Out of Scope
- In-memory `JobQueue` (P12-A2)
- `JobScheduler::submit` (P12-A3)
- HTTP handler wiring (P12-A4, P12-A5)
- Migration files — the `jobs` table schema already exists in `backend/migrations/001_jobs.sql`
- Any changes to `anvilml-core`, `backend/`, or `crates/anvilml-server/`

## Approach

1. **Add dependencies** to `crates/anvilml-scheduler/Cargo.toml`:
   - Add `sqlx = { workspace = true }` (workspace already has features: `sqlite, runtime-tokio, macros, migrate`)
   - Add `uuid = { workspace = true }` (workspace already has features: `serde, v4`)
   - Add `chrono = { workspace = true }` (workspace already has features: `serde`)
   - Add dev-dependencies: `serial_test = { workspace = true }`, `tempfile = { workspace = true }`

2. **Create `src/job_store.rs`** with the four async functions. Each function takes a `&SqlitePool` (from `sqlx::sqlite::SqlitePool`) and uses the `query!` / `query_as!` / `query_file!` macros for compile-time SQL checking. The `Job` struct from `anvilml-core` is mapped to/from the SQLite row:
   - `graph` and `settings` are stored as TEXT (JSON blobs). Since `serde_json::Value` and `JobSettings` implement `Serialize`/`Deserialize`, use `query_as!` with a helper that serialises these fields, or use `query!` returning a raw row and deserialize manually.
   - Timestamps: `DateTime<Utc>` → ISO 8601 string for INSERT; parse back from TEXT on SELECT.

3. **Register the module** in `src/lib.rs`: add `pub mod job_store;` and re-export the four functions.

4. **Write tests** in a `#[cfg(test)]` module at the bottom of `job_store.rs` (adjacent to code, per FORGE_AGENT_RULES §5.4). Each test creates its own isolated in-memory SQLite pool via `SqlitePool::connect("sqlite::memory:")`. Tests use `serial_test::serial` attribute where DB state must not race.

   Test cases:
   - `test_insert_and_get`: insert a fully-populated Job, get_job by id returns it with matching fields
   - `test_list_jobs_all`: insert 3 jobs, list_jobs returns all 3 (default limit)
   - `test_list_jobs_status_filter`: insert mixed-status jobs, list_jobs(status=Some(Queued)) returns only Queued
   - `test_list_jobs_limit`: insert 5 jobs, list_jobs(limit=2) returns exactly 2
   - `test_list_jobs_before_cursor`: insert jobs at different timestamps, list_jobs(before=some_ts) excludes later jobs
   - `test_update_status`: get a Queued job, update_status to Running with error=None, verify status and started_at updated

5. **Verify**: run `cargo test -p anvilml-scheduler --job_store` — must exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add sqlx, uuid, chrono deps; add serial_test + tempfile dev-deps |
| Create | `crates/anvilml-scheduler/src/job_store.rs` | Four async CRUD functions + inline #[cfg(test)] module |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod job_store;` and re-exports |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-scheduler/src/job_store.rs` (inline test module) | `test_insert_and_get` | insert_job writes a row; get_job retrieves it with correct fields |
| Same file | `test_list_jobs_all` | list_jobs without filters returns all inserted rows |
| Same file | `test_list_jobs_status_filter` | list_jobs(status=Some(Queued)) only returns matching status |
| Same file | `test_list_jobs_limit` | list_jobs(limit=N) returns at most N rows |
| Same file | `test_list_jobs_before_cursor` | list_jobs(before=ts) excludes jobs created after the cursor |
| Same file | `test_update_status` | update_status changes status, sets started_at/completed_at/error correctly |

## CI Impact

No CI workflow files are modified. The task only adds code and tests within an existing crate (`anvilml-scheduler`). The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy`, `cargo fmt`) will naturally pick up the new module and tests. No new CI jobs or steps are required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| sqlx compile-time SQL checking fails because `jobs` table schema is in a different crate's migration (backend/) | Medium | Build failure | Use `query!` with inline SQL strings that match the existing `001_jobs.sql` schema exactly; alternatively use `query_as!` with manual column mapping. No migration file changes needed — we only read/write the existing table. |
| `serde_json::Value` / `JobSettings` cannot be bound directly to sqlx TEXT parameter | Medium | Build failure | Serialize to JSON string manually before binding (`serde_json::to_string(&job.graph)?`), then deserialize on SELECT. This is straightforward and avoids any sqlx type-mapping issues. |
| In-memory SQLite pool (`sqlite::memory:`) may cause concurrent test interference if tests run in parallel | Medium | Flaky tests | Use `#[serial_test::serial]` on each test function so they execute one at a time against the isolated in-memory DB. Each test creates its own pool, so there's no shared state even without serial — but serial adds safety. |
| chrono DateTime<Utc> serialization/deserialization mismatch between Rust and SQLite TEXT | Low | Data corruption | Use ISO 8601 / RFC 3339 format via `chrono::DateTime<Utc>::to_rfc3339()` for INSERT and `DateTime::parse_from_rfc3339().ok()?.into_utc()` on SELECT — matches the existing schema convention. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --lib job_store` exits 0 with all 6 tests passing
- [ ] `cargo clippy -p anvilml-scheduler --features mock-hardware -- -D warnings` exits 0 (no new warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (no formatting drift)
- [ ] New dependencies (`sqlx`, `uuid`, `chrono`) are pulled from workspace dependencies, not re-pinned individually
- [ ] `job_store.rs` exports exactly four public async functions: `insert_job`, `get_job`, `list_jobs`, `update_status`
- [ ] `graph` and `settings` columns are stored as TEXT JSON (serialized via serde_json)
- [ ] `list_jobs` honours the default limit of 100 and clamps to max 1000 per `LimitsConfig` convention
