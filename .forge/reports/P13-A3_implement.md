# Implementation Report: P13-A3

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P13-A3                                      |
| Phase         | 013 — Job Queue & Persistence               |
| Description   | anvilml-scheduler: scheduler.rs JobScheduler submit and persistence |
| Implemented   | 2026-06-20T00:00:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented `JobScheduler` in `crates/anvilml-scheduler/src/scheduler.rs` with three public methods: `submit()` (validate graph → persist to SQLite → enqueue → broadcast), `get_job()` (query by UUID from SQLite), and `list_jobs()` (query with optional status/limit/before filters). Added `sqlx` and `anvilml-ipc` dependencies to the scheduler crate. Created 8 integration tests covering valid submission, invalid graph rejection, job retrieval, listing with filters, and pagination. Updated `lib.rs` to expose the new module. All workspace tests pass with zero failures.

## Resolved Dependencies

| Type   | Name         | Version resolved | Source          |
|--------|-------------|------------------|-----------------|
| crate  | sqlx        | 0.9.0            | Cargo.lock (workspace) |
| crate  | anvilml-ipc | path             | Local workspace dependency |
| crate  | chrono      | 0.4.45           | Workspace dependency |

No new external crates were added — `sqlx` and `chrono` were already declared in the workspace dependencies. `anvilml-ipc` is a local workspace path dependency.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/scheduler.rs` | JobScheduler struct with new(), submit(), get_job(), list_jobs() methods and helper functions |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Added `pub mod scheduler;` and `pub use scheduler::JobScheduler;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Added `sqlx`, `anvilml-ipc`, `chrono` dependencies; added `serial_test`, `sqlx` dev-dependencies; bumped version 0.1.6 → 0.1.7 |
| CREATE | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | 8 integration tests for JobScheduler |
| MODIFY | `docs/TESTS.md` | Added 8 test entries for new scheduler tests |

## Commit Log

```
 .forge/reports/P13-A3_plan.md                     | 185 ++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 Cargo.lock                                        |   5 +-
 crates/anvilml-scheduler/Cargo.toml               |   7 +-
 crates/anvilml-scheduler/src/lib.rs               |   3 +
 crates/anvilml-scheduler/src/scheduler.rs         | 513 ++++++++++++++++++++++
 crates/anvilml-scheduler/tests/scheduler_tests.rs | 445 +++++++++++++++++++
 docs/TESTS.md                                     |  72 +++
 9 files changed, 1238 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/scheduler_tests.rs (target/debug/deps/scheduler_tests-c5b210120dbbbefa)

running 8 tests
test test_get_job_missing_returns_none ... ok
test test_list_jobs_filter_by_status ... ok
test test_get_job_returns_job ... ok
test test_list_jobs_returns_all ... ok
test test_list_jobs_with_before_filter ... ok
test test_list_jobs_with_limit ... ok
test test_submit_invalid_graph ... ok
test test_submit_valid_graph ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace: all 200+ tests passed across all crates.
```

## Format Gate

```
cargo fmt --all -- --check
# Exits 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.17s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.19s

# 3. Real-hardware Linux:
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.39s

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.99s
```

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
# No diff — openapi.json is up to date
```

## Public API Delta

```
+pub mod scheduler;
+pub use scheduler::JobScheduler;
+pub struct JobScheduler {
+    pub fn new(
+    pub async fn submit(&self, req: SubmitJobRequest) -> Result<SubmitJobResponse, AnvilError> {
+    pub async fn get_job(&self, id: Uuid) -> Result<Option<Job>, AnvilError> {
+    pub async fn list_jobs(
```

New public items:
- `JobScheduler` struct — `anvilml_scheduler::JobScheduler`
- `JobScheduler::new` — constructor
- `JobScheduler::submit` — async fn, validates and persists jobs
- `JobScheduler::get_job` — async fn, queries by UUID
- `JobScheduler::list_jobs` — async fn, queries with filters

## Deviations from Plan

- **Dependency addition**: Added `anvilml-ipc` as a direct dependency (not just `sqlx`) because `EventBroadcaster` is not re-exported from `anvilml-worker` and `anvilml-scheduler` needs direct access to it.
- **Added `chrono` dependency**: Required for `DateTime<Utc>` type in the `list_jobs` method signature and timestamp handling.
- **`list_jobs` SQL approach**: Used `sqlx::QueryBuilder` with `push`/`push_bind` instead of `sqlx::query` with positional parameters. This was necessary because `sqlx::query` requires `SqlSafeStr` which only implements for `&'static str`, and dynamic SQL strings need either `AssertSqlSafe` wrapping or `QueryBuilder`.
- **`row_to_job` uses `SqliteRow`**: Used concrete `sqlx::sqlite::SqliteRow` type instead of generic `impl Row` because `try_get` requires a concrete database type for column index resolution.
- **`#[expect(dead_code)]` on `ledger` field**: The `ledger` field is owned but not yet used — VRAM checks during submission are out of scope for Phase 013. The dispatch loop in Phase 014 will use it. Documented with `reason` attribute.
- **Test SQL status string**: Fixed test to use lowercase `'failed'` in SQL UPDATE to match the `status_to_string` output used in `list_jobs` filters.

## Blockers

None.
