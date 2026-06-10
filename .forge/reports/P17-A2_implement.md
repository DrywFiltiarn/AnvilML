# Implementation Report: P17-A2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P17-A2                          |
| Phase         | 017 — Job & Artifact Management |
| Description   | anvilml-server: DELETE /v1/jobs bulk clear by status |
| Implemented   | 2026-06-10T17:15:00Z           |
| Status        | COMPLETE                        |

## Summary

Implemented bulk job deletion by status via `DELETE /v1/jobs?status=...`. Added `delete_by_status` to `anvilml-scheduler::job_store`, a `clear_jobs` handler with `ClearJobsQuery`/`ClearJobsResponse` structs in `handlers/jobs.rs`, wired the DELETE route in `lib.rs`, added 5 unit tests, and bumped `anvilml-server` version from 0.1.10 to 0.1.11. The handler accepts `?status=completed|failed|cancelled|all` (case-insensitive, defaults to `all`), deletes artifacts per-job (best-effort), removes job rows, and returns `{ "removed": N }`.

## Resolved Dependencies

No new dependencies added.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Added `delete_by_status(pool, status_filter) -> Result<Vec<Uuid>, sqlx::Error>` function |
| Modify | `crates/anvilml-server/Cargo.toml` | Bumped patch version 0.1.10 → 0.1.11 |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Added `ClearJobsQuery`, `ClearJobsResponse`, `clear_jobs` handler, and 5 unit tests; added `delete_by_status` import |
| Modify | `crates/anvilml-server/src/lib.rs` | Added `.delete(handlers::jobs::clear_jobs)` to `/v1/jobs` route |

## Commit Log

```
 crates/anvilml-scheduler/src/job_store.rs  |  34 ++
 crates/anvilml-server/Cargo.toml           |   2 +-
 crates/anvilml-server/src/handlers/jobs.rs | 567 ++++++++++++++++++++++++++++-
 crates/anvilml-server/src/lib.rs           |   4 +-
 4 files changed, 603 insertions(+), 4 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-925fc1363201f810)

running 35 tests
test handlers::jobs::tests::cancel_job_returns_404_when_missing ... ok
test artifact::store::tests::delete_for_job_empty_returns_zero ... ok
test handlers::jobs::tests::clear_jobs_rejects_invalid_status ... ok
test handlers::artifacts::tests::list_artifacts_empty_returns_200_with_empty_array ... ok
test artifact::store::tests::list_before_filter ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test artifact::store::tests::list_empty_returns_empty_array ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test artifact::store::tests::list_limit_clamped ... ok
test artifact::store::tests::list_with_job_id_filter ... ok
test handlers::artifacts::tests::list_artifacts_with_job_id_filter ... ok
test handlers::jobs::tests::cancel_job_returns_409_for_completed_job ... ok
test handlers::jobs::tests::cancel_job_returns_202_for_queued_job ... ok
test handlers::jobs::tests::clear_jobs_defaults_to_all ... ok
test handlers::jobs::tests::clear_jobs_removes_artifacts ... ok
test tests::health_returns_200 ... ok
test tests::rescan_returns_202 ... ok
test tests::workers_endpoint_returns_200 ... ok
test artifact::store::tests::delete_for_job_removes_files_and_rows ... ok
test handlers::jobs::tests::delete_job_returns_404_when_missing ... ok
test handlers::jobs::tests::get_job_returns_404_when_missing ... ok
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
test handlers::jobs::tests::clear_jobs_skips_running_jobs ... ok
test handlers::jobs::tests::get_job_returns_200_with_queued_job ... ok
test handlers::jobs::tests::list_jobs_filters_by_status ... ok
test handlers::jobs::tests::delete_job_returns_409_for_queued_job ... ok
test handlers::jobs::tests::list_jobs_limit_clamps_to_one ... ok
test handlers::jobs::tests::delete_job_returns_204_for_completed_job ... ok
test handlers::jobs::tests::list_jobs_returns_all_submitted_jobs ... ok
test tests::env_returns_200_with_stub_report ... ok
test handlers::jobs::tests::clear_jobs_returns_200_for_completed_jobs ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.04s
```

Full workspace: 219 tests passed, 0 failed.

## Format Gate

```
$ cargo fmt --all -- --check
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
$ cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.10s

# 2. Mock-hardware Windows
$ cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.58s

# 3. Real-hardware Linux
$ cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.84s

# 4. Real-hardware Windows
$ cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.95s
```

All four cross-checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
$ cargo test -p backend --features mock-hardware -- config_reference
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### OpenAPI Drift Gate
Not applicable — `backend/openapi.json` is not tracked in the repository. Per the approved plan, OpenAPI spec regeneration is handled by CI drift gate, not this task's direct work.

## Deviations from Plan

- **SQL implementation**: The plan specified a single SQL query with `WHERE (status IN (...) OR ? IS NULL)`. This would match ALL rows (including Queued/Running) when `status_filter` is `None` because `? IS NULL` evaluates to true. Implemented with two separate queries instead: one for terminal statuses (when `status_filter` is `None`) and one for exact status match. This is a correction to ensure the handler never deletes non-terminal jobs.
- **Test fix**: `clear_jobs_removes_artifacts` test required creating the `artifacts` table in the test pool (the `build_test_app` helper only creates the `jobs` table). Added the table creation in the test body.
- **Pre-existing warning fix**: Removed unused `delete` routing import from `lib.rs` — the `.delete()` method is called on the route builder, not via the `axum::routing::delete` function.

## Blockers

None.
