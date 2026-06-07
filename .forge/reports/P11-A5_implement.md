# Implementation Report: P11-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-A5                                      |
| Phase       | 011 — Graph Validation                      |
| Description | anvilml-server: POST /v1/jobs validating graph (422 on invalid) |
| Implemented | 2026-06-07T13:15:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Implemented `POST /v1/jobs` handler in `anvilml-server` that validates submitted DAG graphs via the existing `anvilml_scheduler::validate_graph()` function. Invalid graphs return HTTP 422 with structured error details; valid graphs return HTTP 202 with a placeholder job ID and queue position. Enqueueing, persistence, and worker dispatch are deferred to phase 12 as specified.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|-----------------|----------------|
| crate  | utoipa    | 5.5.0           | lockfile (workspace) |
| crate  | uuid      | 1.23.2          | lockfile (workspace) |

Both `utoipa` and `uuid` are workspace-level dependencies already declared in root `Cargo.toml`. Added to `anvilml-server/Cargo.toml` as new direct dependencies to enable handler compilation (proc macros like `#[utoipa::path]` require the crate to be a direct dependency).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/handlers/jobs.rs` | New handler module with `submit_job`, utoipa annotations, `ErrorInline` error type, and 2 unit tests |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod jobs;` |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `.route("/v1/jobs", post(handlers::jobs::submit_job))` to `build_router()` |
| Modify | `crates/anvilml-server/Cargo.toml` | Added `utoipa` and `uuid` dependencies; bumped patch version 0.1.0 → 0.1.1 |

## Commit Log

```
 .forge/reports/P11-A5_plan.md              | 135 +++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  13 +-
 Cargo.lock                                 |   4 +-
 crates/anvilml-server/Cargo.toml           |  18 ++-
 crates/anvilml-server/src/handlers/jobs.rs | 233 +++++++++++++++++++++++++++++
 crates/anvilml-server/src/handlers/mod.rs  |   1 +
 crates/anvilml-server/src/lib.rs           |   1 +
 8 files changed, 393 insertions(+), 18 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-c629841958f108a4)

running 11 tests
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::health_returns_200 ... ok
test tests::rescan_returns_202 ... ok
test tests::workers_endpoint_returns_200 ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test tests::system_returns_200_with_hardware_info ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.00s

     Running tests/api_models.rs (target/debug/deps/api_models-d0c12d35cc2718ac)

running 3 tests
test list_models_kind_filter_diffusion ... ok
test list_models_kind_filter_no_match ... ok
test list_models_returns_scanned_models ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-bfa522e783c0f93f)

running 1 test
test ws_connect_broadcast_receive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

   Doc-tests anvilml_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite (all crates): **198 tests passed, 0 failed**.

## Format Gate

```
(not applicable — exit 0, no output)
```

`cargo fmt --all -- --check` exited 0 with no formatting drift detected.

## Platform Cross-Check

**Check 1 — mock-hardware Linux:**
```
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.38s
```

**Check 2 — mock-hardware Windows cross (`x86_64-pc-windows-gnu`):**
```
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.71s
```

**Check 3 — real-hardware Linux:**
```
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.40s
```

**Check 4 — real-hardware Windows cross (`x86_64-pc-windows-gnu`):**
```
    Checking anvilml-server v0.1.1 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.1 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.59s
```

All four checks exited 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

No config surface changes were made in this task (no `ServerConfig` fields added/removed), so the gate passes trivially.

## Deviations from Plan

- **Response type change:** The plan specified returning `(StatusCode::ACCEPTED, Json(SubmitJobResponse { ... }))` for success. Due to Rust's type system requiring both match arms to have identical tuple types, and the error arm returning `Json<serde_json::Value>`, the success arm was changed to return `Json(json!({"job_id": ..., "queue_position": 0}))`. The JSON payload is semantically equivalent — `job_id` is a string (UUID serialized) and `queue_position` is `0`. This is functionally identical but avoids the type mismatch.
- **Added `utoipa` and `uuid` dependencies to `anvilml-server/Cargo.toml`:** These were not listed in the plan's "Files Affected" table but are necessary scaffolding — `#[utoipa::path]` requires `utoipa` as a direct dependency (proc macros), and `Uuid::new_v4()` requires the `uuid` crate. Both are workspace-level dependencies already declared in root `Cargo.toml`.
- **Removed unused imports:** The plan's test module imported `JobSettings` and `SubmitJobRequest` which were not used in the test code. Removed during implementation to satisfy clippy `-D warnings`.
- **`ErrorInline` struct marked `#[expect(dead_code)]`:** The struct is referenced by utoipa annotations but never constructed directly in handler code, triggering a dead_code warning.

## Blockers

None.
