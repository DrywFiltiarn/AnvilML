# Implementation Report: P12-A4

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P12-A4                                          |
| Phase       | 012 — Job Submission & Queue                    |
| Description | anvilml-server: wire POST /v1/jobs to scheduler.submit + GET /v1/jobs/:id |
| Implemented | 2026-06-07T19:45:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Wired the `POST /v1/jobs` endpoint to call `JobScheduler::submit()` (replacing the stub that returned a fake UUID) and added a new `GET /v1/jobs/{id}` endpoint that retrieves a persisted job from SQLite via `job_store::get_job()`. The scheduler is constructed in `main.rs` using an in-memory queue, workers snapshot, database pool, broadcast channel, and a Notify handle. All existing tests continue to pass with the optional scheduler field.

## Resolved Dependencies

No new dependencies were added. The `anvilml-scheduler` crate was already a dependency of both `anvilml-server` and (now) `backend`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `scheduler: Option<Arc<JobScheduler>>` field, update both constructors to accept it, update Clone impl |
| Modify | `backend/Cargo.toml` | Add `anvilml-scheduler` dependency |
| Modify | `backend/src/main.rs` | Construct `JobScheduler`, pass to `AppState::new_with_hardware()` |
| Modify | `crates/anvilml-server/Cargo.toml` | Patch version bump `0.1.1 → 0.1.2` |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `.route("/v1/jobs/{id}", get(...))` route, update existing test constructors |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Rewrite `submit_job` to call scheduler, add `get_job` handler, add 4 tests |
| Modify | `crates/anvilml-server/src/ws/stats_tick.rs` | Update test constructor call with scheduler param |
| Modify | `crates/anvilml-server/tests/api_models.rs` | Update test constructor call with scheduler param |
| Modify | `crates/anvilml-server/tests/api_ws_events.rs` | Update test constructor call with scheduler param |
| Bump   | `backend/Cargo.toml` | Patch version bump `0.1.1 → 0.1.2` |

## Commit Log

```
 backend/Cargo.toml                           |   3 +-
 backend/src/main.rs                          |  18 ++
 crates/anvilml-server/Cargo.toml             |   2 +-
 crates/anvilml-server/src/handlers/jobs.rs   | 296 ++++++++++++++++++++++++---
 crates/anvilml-server/src/lib.rs             |  23 ++-
 crates/anvilml-server/src/state.rs           |   9 +
 crates/anvilml-server/src/ws/stats_tick.rs   |   1 +
 crates/anvilml-server/tests/api_models.rs    |   1 +
 crates/anvilml-server/tests/api_ws_events.rs |   2 +-
 Cargo.lock                                   |   5 +-
 10 files changed, 326 insertions(+), 44 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-...)

running 13 tests
test tests::rescan_returns_202 ... ok
test tests::env_returns_200_with_stub_report ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test tests::workers_endpoint_returns_200 ... ok
test tests::health_returns_200 ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test handlers::jobs::tests::get_job_returns_404_when_missing ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
test handlers::jobs::tests::get_job_returns_200_with_queued_job ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs

running 3 tests
test list_models_kind_filter_diffusion ... ok
test list_models_kind_filter_no_match ... ok
test list_models_returns_scanned_models ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs

running 1 test
test ws_connect_broadcast_receive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Format Gate

```
(exit code 0 — no formatting drift)
```

## Platform Cross-Check

All four checks passed (exit code 0):

1. `cargo check --workspace --features mock-hardware` — Finished in 1.47s
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished in 3.03s
3. `cargo check --bin anvilml` — Finished in 2.00s
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished in 1.83s

## Project Gates

```
     Running tests/config_reference.rs (target/debug/deps/config_reference-...)

running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **Scheduler field is `Option<Arc<JobScheduler>>` instead of non-optional `Arc<JobConstructor>`**: The plan specified panicking on `None`, but this would break all existing tests that don't involve job functionality. Changed to `Option` with graceful 503 handling in handlers when scheduler/db is not configured, while production code (main.rs) always provides a real scheduler.
- **Added `anvilml-scheduler` dependency to `backend/Cargo.toml`**: The plan assumed this was already available, but it wasn't listed as a backend dependency. Added it explicitly.
- **Fixed pre-existing test constructors**: All existing tests in `lib.rs`, `stats_tick.rs`, `api_models.rs`, and `api_ws_events.rs` needed the new `scheduler: None` parameter added to their `AppState::new()` / `AppState::new_with_hardware()` calls.

## Blockers

None.
