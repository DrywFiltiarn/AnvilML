# Implementation Report: P13-B1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P13-B1                          |
| Phase         | 13 — Job Queue                  |
| Description   | anvilml-registry: JobStore CRUD, ghost-job reset on startup |
| Implemented   | 2026-07-07T12:30:00Z            |
| Status        | COMPLETE                          |

## Summary

Implemented `JobStore` — the SQLite-backed persistence layer for `Job` records in the
`anvilml-registry` crate. This provides four operations: `upsert()` to persist a job
(serializing `graph` and `settings` to JSON TEXT columns), `get()` to fetch a single
job by UUID, `list()` to query jobs with optional status filter and limit, and
`reset_ghost_jobs()` to transition stale `Queued`/`Running` jobs to `Failed` with
`error = "server_restart"`. Added `uuid` with `serde` feature to the crate's main
dependencies, bumped the crate version from 0.1.6 to 0.1.7, and wrote 9 integration
tests covering all four operations.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | uuid      | 1.23.4           | rust-docs MCP  |

The `uuid` crate was already present as a dev-dependency with `v4` feature. It is now
promoted to a main dependency with the `serde` feature (in addition to `v4` from
dev-dependencies) to enable `Uuid` deserialization from SQL TEXT columns during `get()`
and `list()`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/job_store.rs` | `JobStore` struct with upsert/get/list/reset_ghost_jobs methods |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Added `pub mod job_store;` and `pub use job_store::JobStore;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Added `uuid = { version = "1.23", features = ["serde"] }` to main deps; bumped version 0.1.6 → 0.1.7 |
| CREATE | `crates/anvilml-registry/tests/job_store_tests.rs` | 9 integration tests for JobStore CRUD operations |
| MODIFY | `docs/TESTS.md` | Added 9 test entries for job_store_tests |

## Commit Log

```
 .forge/reports/P13-B1_plan.md                    | 210 ++++++++++++
 .forge/state/CURRENT_TASK.md                     |   6 +-
 .forge/state/state.json                          |  13 +-
 Cargo.lock                                       |   2 +-
 crates/anvilml-registry/Cargo.toml               |   3 +-
 crates/anvilml-registry/src/job_store.rs         | 370 +++++++++++++++++++++
 crates/anvilml-registry/src/lib.rs               |   2 +
 crates/anvilml-registry/tests/job_store_tests.rs | 401 +++++++++++++++++++++++
 docs/TESTS.md                                    | 108 ++++++
 9 files changed, 1104 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/job_store_tests.rs (target/debug/deps/job_store_tests-1d609d62121d441a)

running 9 tests
test test_get_missing_id_returns_none ... ok
test test_reset_ghost_jobs_queued_becomes_failed ... ok
test test_upsert_get_roundtrip ... ok
test test_list_no_filter ... ok
test test_list_with_limit ... ok
test test_reset_ghost_jobs_empty_table ... ok
test test_reset_ghost_jobs_running_becomes_failed ... ok
test test_list_with_status_filter ... ok
test test_reset_ghost_jobs_completed_not_affected ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: all tests passed (200+ tests across all crates, 0 failures).

## Format Gate

```
cargo fmt --all -- --check
```
Exited 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.00s

# 2. Mock-hardware Windows:
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.98s

# 3. Real-hardware Linux:
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.68s

# 4. Real-hardware Windows:
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.60s
```

All four platform cross-checks passed.

## Project Gates

```
# Gate 1 — Config Surface Sync:
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passed. No config changes were made in this task.

## Public API Delta

```
git diff HEAD -- crates/anvilml-registry/src/job_store.rs crates/anvilml-registry/src/lib.rs | grep "^+.*pub " | head -40
+pub mod job_store;
+pub use job_store::JobStore;
```

New public items:
- `pub mod job_store` — module declaration in `lib.rs`
- `pub use job_store::JobStore` — re-export in `lib.rs`
- `pub struct JobStore` — struct in `job_store.rs`
- `pub fn new(pool: SqlitePool) -> Self` — constructor
- `pub async fn upsert(&self, job: &Job) -> Result<(), AnvilError>` — persist a job
- `pub async fn get(&self, id: Uuid) -> Result<Option<Job>, AnvilError>` — fetch a job
- `pub async fn list(&self, status: Option<JobStatus>, limit: Option<u32>) -> Result<Vec<Job>, AnvilError>` — query jobs
- `pub async fn reset_ghost_jobs(&self) -> Result<u32, AnvilError>` — reset stale jobs

All public items match the plan's `## Public API Surface` table exactly.

## Deviations from Plan

- **Dynamic SQL refactoring**: The plan described building the `list()` query dynamically
  using `format!()`. During implementation, this was found to conflict with sqlx's
  `SqlSafeStr` requirement (static `&'static str` only). The query was refactored to use
  a `match` over four static query strings, following the same pattern as
  `ModelStore::list`. This is a strictly safer approach that avoids dynamic SQL entirely.
- **Test helper fix**: The `test_job()` helper was initially setting `error =
  Some("test failure")` for `Completed` status jobs. This was corrected to only set error
  for `Failed` status, and the `test_reset_ghost_jobs_completed_not_affected` assertion
  was updated accordingly. This was discovered during test execution.

## Blockers

None.
