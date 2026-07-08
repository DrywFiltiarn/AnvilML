# Implementation Report: P14-D2

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P14-D2                                      |
| Phase         | 14 — Dispatch & Execute                     |
| Description   | anvilml-server: GET /v1/jobs and GET /v1/jobs/:id handlers |
| Implemented   | 2026-07-08T14:30:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the two read endpoints for the jobs API — `GET /v1/jobs` (list with optional status/limit filters) and `GET /v1/jobs/{id}` (single job lookup) — in `crates/anvilml-server/src/handlers/jobs.rs`, wired them into `build_router()` in `lib.rs`, added a `list_jobs()` delegation method to `JobScheduler` in `anvilml-scheduler`, and added 6 new integration tests in `jobs_tests.rs` (bringing the total from 4 to 10). All 10 tests pass, clippy is clean, format check passes, and all platform cross-checks succeed.

## Resolved Dependencies

None. This task introduces no new external crates or packages. All types used (`JobStatus`, `Job`, `DateTime<Utc>`, `Uuid`, `serde_json::Value`) are already in scope through existing workspace dependencies.

| Type   | Name    | Version verified | Source         | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) |         |                 |                |                        |

Note: The `before` field in `ListJobsParams` was originally planned as `Option<DateTime<Utc>>` but was changed to `Option<String>` because `serde_urlencoded` (axum's query param deserializer) cannot parse `DateTime<Utc>` from query strings. The field is intentionally unused (forward-compatibility only), so this is a Deviation, not a blocker.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Added `list_jobs()` delegation method |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.17 → 0.1.18 |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Added `ListJobsParams` struct, `list_jobs()` and `get_job()` handlers, new imports |
| Modify | `crates/anvilml-server/src/lib.rs` | Registered GET routes for `/v1/jobs` and `/v1/jobs/{id}` in `build_router()` |
| Modify | `crates/anvilml-server/Cargo.toml` | Added `tracing` dependency; added `tracing` to deps (no `chrono` needed); bump patch version 0.1.6 → 0.1.7 |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Added 6 new integration tests |
| Modify | `docs/TESTS.md` | Added 6 new test entries |

## Commit Log

```
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 Cargo.lock                                 |   5 +-
 crates/anvilml-scheduler/Cargo.toml        |   2 +-
 crates/anvilml-scheduler/src/scheduler.rs  |  27 +++
 crates/anvilml-server/Cargo.toml           |   4 +-
 crates/anvilml-server/src/handlers/jobs.rs |  87 ++++++++
 crates/anvilml-server/src/lib.rs           |  13 +-
 crates/anvilml-server/tests/jobs_tests.rs  | 313 ++++++++++++++++++++++++++++-
 docs/TESTS.md                              |  72 +++++++
 10 files changed, 527 insertions(+), 15 deletions(-)
```

## Test Results

```
     Running tests/jobs_tests.rs (target/debug/deps/jobs_tests-6886a93dcfab40c2)

running 10 tests
test test_submit_job_malformed_body_returns_400 ... ok
test test_get_job_unknown_returns_404 ... ok
test test_list_jobs_no_filter_returns_all ... ok
test test_list_jobs_before_param_accepted ... ok
test test_get_job_existing_returns_200 ... ok
test test_submit_job_empty_registry_returns_503 ... ok
test test_submit_job_invalid_graph_returns_400 ... ok
test test_submit_job_valid_returns_202 ... ok
test test_list_jobs_limit ... ok
test test_list_jobs_status_filter ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 287 tests passed, 0 failed, 0 ignored.

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.20s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.77s

# 3. Real-hardware Linux
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.54s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.59s
```

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
→ test tests::config_reference_matches_defaults ... ok
→ test result: ok. 1 passed; 0 failed
```

### Gate 2 — OpenAPI Drift
`api/openapi.json` does not yet exist — gate skipped per the rules (skip only if `api/openapi.json` does not yet exist).

## Public API Delta

New `pub` items from `git diff HEAD -- crates/anvilml-server/src/handlers/jobs.rs crates/anvilml-server/src/lib.rs crates/anvilml-scheduler/src/scheduler.rs | grep "^+.*pub "`:

```
+    pub async fn list_jobs(
+    pub status: Option<JobStatus>,
+    pub limit: Option<u32>,
+    pub before: Option<String>,
```

New `pub(crate)` items:
```
+pub(crate) struct ListJobsParams {
+pub(crate) async fn list_jobs(
+pub(crate) async fn get_job(
```

This matches the plan's Public API Surface table:
- `list_jobs()` handler — `pub(crate) async fn list_jobs(State<AppState>, Query<ListJobsParams>) -> Result<Json<Vec<Job>>, AnvilError>`
- `get_job()` handler — `pub(crate) async fn get_job(State<AppState>, Path<Uuid>) -> Result<Json<Job>, AnvilError>`
- `ListJobsParams` struct — `pub(crate) struct ListJobsParams { status: Option<JobStatus>, limit: Option<u32>, before: Option<String> }`
- `list_jobs()` method on `JobScheduler` — `pub async fn list_jobs(&self, status: Option<JobStatus>, limit: Option<u32>) -> Result<Vec<Job>, AnvilError>`

## Deviations from Plan

1. **Route syntax**: The plan specified `/v1/jobs/:id` but axum 0.8+ requires `{id}` syntax. Changed to `/v1/jobs/{id}` and added an inline comment explaining the axum version difference.

2. **`before` field type**: The plan specified `before: Option<DateTime<Utc>>` but `serde_urlencoded` (axum's query param deserializer) cannot parse `DateTime<Utc>` from query strings. Changed to `Option<String>` since the field is intentionally unused (forward-compatibility only). Added `#[allow(dead_code)]` with an inline comment explaining the reason.

3. **`chrono` dependency**: Removed `chrono` from `anvilml-server` dependencies since `DateTime<Utc>` is no longer used in the handler code.

4. **`tracing` dependency**: Added `tracing = "0.1"` to `anvilml-server` dependencies because the new handlers use `#[tracing::instrument]` and `tracing::info!`/`tracing::debug!` macros.

## Blockers

None.
