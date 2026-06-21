# Implementation Report: P17-A2

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P17-A2                                            |
| Phase         | 017 — Cancellation                                |
| Description   | anvilml-server: POST /v1/jobs/:id/cancel + DELETE endpoints |
| Implemented   | 2026-06-21T12:30:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented three new HTTP handlers (`cancel_job`, `delete_job`, `bulk_clear`) and wired them into the axum router. Added `ArtifactStore::delete()` for artifact file and metadata deletion, and `JobScheduler::delete_jobs_by_status()` for bulk DB deletion of terminal jobs. Added five integration tests covering cancel (202/409), delete (204/409), and bulk clear (200 with count) flows. Bumped `anvilml-server` version from 0.1.26 to 0.1.27. All 194 workspace tests pass, format/lint/cross-check/gates all clean.

## Resolved Dependencies

None. All types and methods referenced exist in already-declared workspace crates.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-artifacts/src/store.rs` | Added `pub async fn delete(&self, hash: &str)` method |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Added `pub async fn delete_jobs_by_status(&self, status: &str)` method |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.26 → 0.1.27 |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Added `BulkClearQuery` struct, `cancel_job`, `delete_job`, `bulk_clear` handlers |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Re-export new handlers: `cancel_job`, `delete_job`, `bulk_clear` |
| Modify | `crates/anvilml-server/src/lib.rs` | Mount 3 new routes in `build_router()` |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Added 5 integration tests |
| Modify | `docs/TESTS.md` | Added 5 new test catalogue entries |

## Commit Log

```
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  17 +-
 Cargo.lock                                 |   2 +-
 crates/anvilml-artifacts/src/store.rs      |  74 ++++
 crates/anvilml-scheduler/src/scheduler.rs  |  64 ++++
 crates/anvilml-server/Cargo.toml           |   2 +-
 crates/anvilml-server/src/handlers/jobs.rs | 222 +++++++++++-
 crates/anvilml-server/src/handlers/mod.rs  |   3 +
 crates/anvilml-server/src/lib.rs           |  16 +-
 crates/anvilml-server/tests/jobs_tests.rs  | 561 +++++++++++++++++++++++++++++
 docs/TESTS.md                              |  45 +++
 11 files changed, 996 insertions(+), 16 deletions(-)
```

## Test Results

```
     Running tests/jobs_tests.rs (target/debug/deps/jobs_tests-7b089d44fac8f9de)

running 10 tests
test test_delete_non_terminal_job_returns_409 ... ok
test test_submit_job_returns_503_when_no_workers ... ok
test test_list_jobs_returns_queued_jobs ... ok
test test_get_job_returns_404_for_unknown_id ... ok
test test_delete_terminal_job_returns_204 ... ok
test test_cancel_terminal_job_returns_409 ... ok
test test_submit_job_returns_202_with_valid_graph ... ok
test test_cancel_queued_job_returns_202 ... ok
test test_submit_job_returns_422_with_unknown_node_type ... ok
test test_bulk_clear_returns_removed_count ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

Full workspace test suite: 194 tests passed, 0 failed.

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.69s
--- CHECK 1 PASSED ---

# 2. Mock-hardware Windows:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.05s
--- CHECK 2 PASSED ---

# 3. Real-hardware Linux:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.28s
--- CHECK 3 PASSED ---

# 4. Real-hardware Windows:
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.62s
--- CHECK 4 PASSED ---
```

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Gate 2 — OpenAPI Drift:**
```
(No diff — `git diff --exit-code api/openapi.json` exited 0)
```

## Public API Delta

```
+    pub async fn delete(&self, hash: &str) -> Result<()> {
+    pub async fn delete_jobs_by_status(&self, status: &str) -> Result<u32, AnvilError> {
+pub struct BulkClearQuery {
+    pub status: Option<String>,
+pub async fn cancel_job(
+pub async fn delete_job(
+pub async fn bulk_clear(
+pub use jobs::bulk_clear;
+pub use jobs::cancel_job;
+pub use jobs::delete_job;
```

New `pub` items:
- `ArtifactStore::delete` (fn, `crates/anvilml-artifacts/src/store.rs`)
- `JobScheduler::delete_jobs_by_status` (fn, `crates/anvilml-scheduler/src/scheduler.rs`)
- `BulkClearQuery` (struct, `crates/anvilml-server/src/handlers/jobs.rs`)
- `cancel_job` (fn, `crates/anvilml-server/src/handlers/jobs.rs`)
- `delete_job` (fn, `crates/anvilml-server/src/handlers/jobs.rs`)
- `bulk_clear` (fn, `crates/anvilml-server/src/handlers/jobs.rs`)
- `pub use jobs::bulk_clear` (re-export, `crates/anvilml-server/src/handlers/mod.rs`)
- `pub use jobs::cancel_job` (re-export, `crates/anvilml-server/src/handlers/mod.rs`)
- `pub use jobs::delete_job` (re-export, `crates/anvilml-server/src/handlers/mod.rs`)

## Deviations from Plan

- `JobScheduler::delete_jobs_by_status` uses `sqlx::AssertSqlSafe` with a dynamic SQL string built from a validated whitelist, rather than `QueryBuilder`. This is necessary because `IN ('completed','failed','cancelled')` cannot be parameterised with sqlx's `QueryBuilder` in a simple way, and the status value is validated against a whitelist before being interpolated.
- The `delete_jobs_by_status` method returns `u32` (rows_affected count) rather than using the `value` tuple from the plan's match. The unused `value` discriminant was removed.
- Integration tests use `Arc::clone(&scheduler)` before passing `scheduler` into `AppState::new()` so the scheduler's `db()` method can still be called for manual status updates. This is a necessary pattern because `AppState::new()` consumes the `Arc<JobScheduler>`.
- `Router` uses `.clone()` on every `oneshot()` call in tests (not just the first) because `Router` does not implement `Copy` and `oneshot()` consumes `self`.

## Blockers

None.
