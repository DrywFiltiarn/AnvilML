# Implementation Report: P18-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-A4                                            |
| Phase       | 018 — Worker Restart API & Preflight              |
| Description | anvilml: wire graceful shutdown to WorkerPool.shutdown_all |
| Implemented | 2026-06-11T01:15:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Wired the existing `WorkerPool::shutdown_all` into the graceful shutdown path so that on SIGINT/SIGTERM/Ctrl-C the server drains workers cleanly, closes the SQLite connection pool (WAL flush), and exits 0 — replacing the previous stub that force-exited without draining. Added a `shutdown` flag to `AppState` that gates new job submissions, returning 503 `server_shutting_down` once shutdown is initiated.

## Resolved Dependencies

Not applicable — no new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Added `shutdown: Arc<AtomicBool>` field, `set_shutdown()` and `is_shutdown()` methods, updated both constructors and `Clone` impl |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Added shutdown gate in `submit_job` — returns 503 `server_shutting_down` when flag is set |
| Modify | `backend/src/shutdown.rs` | Extended `shutdown_signal` to accept `Arc<App>` + `SqlitePool`, implemented full drain sequence (flag → workers → pool) |
| Modify | `backend/src/main.rs` | Cloned `db` for shutdown handler, wired `shutdown::shutdown_signal(Arc::new(state), db_shutdown)`, removed P18-A4 stub comment |
| Modify | `crates/anvilml-server/Cargo.toml` | Bumped version `0.1.13 → 0.1.14` |
| Modify | `backend/Cargo.toml` | Bumped version `0.1.9 → 0.1.10` |

## Commit Log

```
 .forge/reports/P18-A4_plan.md              | 212 +++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 Cargo.lock                                 |   4 +-
 backend/Cargo.toml                         |   2 +-
 backend/src/main.rs                        |  18 +--
 backend/src/shutdown.rs                    |  33 ++++-
 crates/anvilml-server/Cargo.toml           |   2 +-
 crates/anvilml-server/src/handlers/jobs.rs |  12 ++
 crates/anvilml-server/src/state.rs         |  16 +++
 10 files changed, 294 insertions(+), 24 deletions(-)
```

## Test Results

```
Running unittests src/lib.rs (target/debug/deps/anvilml_server-ab1556247bada666)
running 38 tests
test artifact::store::tests::list_empty_returns_empty_array ... ok
test artifact::store::tests::delete_for_job_empty_returns_zero ... ok
test handlers::jobs::tests::clear_jobs_rejects_invalid_status ... ok
test artifact::store::tests::list_with_job_id_filter ... ok
test handlers::jobs::tests::cancel_job_returns_404_when_missing ... ok
test handlers::artifacts::tests::list_artifacts_empty_returns_200_with_empty_array ... ok
test artifact::store::tests::list_before_filter ... ok
test handlers::jobs::tests::cancel_job_returns_409_for_completed_job ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test handlers::jobs::tests::cancel_job_returns_202_for_queued_job ... ok
test artifact::store::tests::list_limit_clamped ... ok
test handlers::jobs::tests::clear_jobs_defaults_to_all ... ok
test handlers::jobs::tests::clear_jobs_removes_artifacts ... ok
test artifact::store::tests::delete_for_job_removes_files_and_rows ... ok
test handlers::jobs::tests::delete_job_returns_404_when_missing ... ok
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
test handlers::jobs::tests::get_job_returns_404_when_missing ... ok
test handlers::jobs::tests::delete_job_returns_409_for_queued_job ... ok
test handlers::jobs::tests::get_job_returns_200_with_queued_job ... ok
test tests::health_returns_200 ... ok
test handlers::jobs::tests::delete_job_returns_204_for_completed_job ... ok
test tests::rescan_returns_202 ... ok
test handlers::jobs::tests::clear_jobs_skips_running_jobs ... ok
test handlers::jobs::tests::list_jobs_limit_clamps_to_one ... ok
test handlers::jobs::tests::list_jobs_filters_by_status ... ok
test handlers::jobs::tests::list_jobs_returns_all_submitted_jobs ... ok
test tests::restart_worker_returns_404_for_unknown_worker ... ok
test tests::restart_worker_returns_503_when_no_workers ... ok
test tests::workers_endpoint_returns_200 ... ok
test handlers::jobs::tests::clear_jobs_returns_200_for_completed_jobs ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok
test tests::restart_worker_returns_202_for_existing_worker ... ok
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/api_artifact_save.rs (target/debug/deps/api_artifact_save-80b2c5b17d7f53f1)
running 1 test
test artifact_save ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/api_artifact_serve.rs (target/debug/deps/api_artifact_serve-b27a1e7ed78912c5)
running 3 tests
test artifact_serve_404_when_missing ... ok
test artifact_serve_200_with_headers ... ok
test artifact_serve_returns_correct_bytes ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/api_models.rs (target/debug/deps/api_models-af6b3f6e66bb6a92)
running 3 tests
test list_models_returns_scanned_models ... ok
test list_models_kind_filter_no_match ... ok
test list_models_kind_filter_diffusion ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-f87686ec5c584cd7)
running 1 test
test ws_connect_broadcast_receive ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/api_cancel.rs (target/debug/deps/api_cancel-fe0af4180fa607b4)
running 2 tests
test cancel_terminal_job_returns_409 ... ok
test cancel_running_job_returns_202_and_ws_cancelled ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/api_delete.rs (target/debug/deps/api_delete-76da1e7ed78912c5)
running 5 tests
test delete_completed_job_removes_artifact_and_row ... ok
test bulk_delete_by_status_removes_only_matching ... ok
test bulk_delete_all_terminal_jobs ... ok
test delete_running_job_returns_409 ... ok
test delete_nonexistent_job_returns_404 ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/api_ws_lifecycle.rs (target/debug/deps/api_ws_lifecycle-258ef7c510ac93a)
running 1 test
test test_ws_lifecycle_full_job ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/config_reference.rs (target/debug/deps/config_reference-520b8a40fb419b08)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/preflight_check.rs (target/debug/deps/preflight_check-91f42496991e3441)
running 4 tests
test env_endpoint_reflects_failed_preflight ... ok
test env_returns_correct_shape_in_stub_context ... ok
test job_submit_proceeds_in_mock_mode ... ok
test job_submit_rejected_when_preflight_fails ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(cargo fmt --all -- --check exited with code 0 — no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.10 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.97s

# 2. Mock-hardware Windows cross-check
Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.10 (/home/dryw/AnvilML/backend)
Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.65s

# 3. Real-hardware Linux check
Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.10 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.96s

# 4. Real-hardware Windows cross-check
Checking anvilml-server v0.1.14 (/home/dryw/AnvilML/crates/anvilml-server)
Checking backend v0.1.10 (/home/dryw/AnvilML/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.31s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
Running tests/config_reference.rs (target/debug/deps/config_reference-520b8a40fb419b08)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **main.rs state handling**: The plan suggested `Arc::clone(&state)` before `build_router`, but `build_router` takes `App` by value (not `Arc<App>`). Implemented by cloning `state` into `shutdown_state`, passing the clone to `build_router`, and wrapping the original in `Arc` for the shutdown handler. This was necessary because the plan's suggested `Arc::clone(&state)` approach would not compile with the actual `build_router` signature.
- **db clone**: The plan assumed `db.clone()` would work at the shutdown call site, but `db` was already moved into `state` via `App::new_with_hardware`. Fixed by cloning `db` earlier (`let db_shutdown = db.clone()`) before it was consumed.
- **shutdown.rs import**: The plan suggested `use sqlx::SqlitePool` but `sqlx` is not a direct dependency of `backend`. Changed to `use anvilml_registry::SqlitePool` (the re-exported type).

## Blockers

None.
