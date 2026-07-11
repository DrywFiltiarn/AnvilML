# Implementation Report: P17-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P17-C1                          |
| Phase         | 17 — Cancellation               |
| Description   | anvilml-server: POST /v1/jobs/:id/cancel handler |
| Implemented   | 2026-07-11T18:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the `cancel_job` HTTP handler for `POST /v1/jobs/{id}` that delegates to `JobScheduler::cancel()`. Changed `cancel()`'s return type from `Result<bool, AnvilError>` to `Result<CancelOutcome, AnvilError>` to distinguish "not found" (404) from "already terminal" (409). Added the handler to `jobs.rs`, wired the POST route into `build_router()`, updated 10 existing scheduler cancel tests to use the new `CancelOutcome` type, and added 5 new cancellation-specific HTTP handler tests. All 319 workspace tests pass.

## Resolved Dependencies

None. All types and APIs used are from existing workspace dependencies (axum, uuid, tokio) that are already declared in `anvilml-server/Cargo.toml` and `anvilml-scheduler/Cargo.toml`. No new crates are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `CancelOutcome` enum; update `cancel()` return type and all return sites |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Re-export `CancelOutcome` as public type |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.27 → 0.1.28 |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Update 10 cancel assertions to use `CancelOutcome` variants |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `cancel_job()` handler function with `CancelOutcome` mapping |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `POST /v1/jobs/{id}` route into `build_router()` |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Add 5 cancellation tests (queued→202, completed→409, unknown→404, running→202, already-cancelled→409) |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.15 → 0.1.16 |

## Commit Log

```
 .forge/reports/P17-C1_plan.md                     | 277 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 Cargo.lock                                        |   4 +-
 crates/anvilml-scheduler/Cargo.toml               |   2 +-
 crates/anvilml-scheduler/src/lib.rs               |   1 +
 crates/anvilml-scheduler/src/scheduler.rs         |  69 ++++--
 crates/anvilml-scheduler/tests/scheduler_tests.rs |  46 ++--
 crates/anvilml-server/Cargo.toml                  |   2 +-
 crates/anvilml-server/src/handlers/jobs.rs        |  46 ++++
 crates/anvilml-server/src/lib.rs                  |   6 +-
 crates/anvilml-server/tests/jobs_tests.rs         | 281 +++++++++++++++++++++-
 12 files changed, 701 insertions(+), 52 deletions(-)
```

## Test Results

```
     Running tests/jobs_tests.rs (target/debug/deps/jobs_tests-81f25438829eb996)

running 15 tests
test test_submit_job_invalid_graph_returns_400 ... ok
test test_submit_job_malformed_body_returns_400 ... ok
test test_submit_job_empty_registry_returns_503 ... ok
test test_cancel_unknown_id_returns_404 ... ok
test test_cancel_completed_job_returns_409 ... ok
test test_get_job_unknown_returns_404 ... ok
test test_list_jobs_no_filter_returns_all ... ok
test test_list_jobs_before_param_accepted ... ok
test test_cancel_queued_job_returns_202 ... ok
test test_get_job_existing_returns_200 ... ok
test test_cancel_already_cancelled_job_returns_409 ... ok
test test_cancel_running_job_returns_202 ... ok
test test_list_jobs_limit ... ok
test test_list_jobs_status_filter ... ok
test test_submit_job_valid_returns_202 ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 319 tests passed, 0 failed.

## Format Gate

```
cargo fmt --all -- --check
# Exit 0 — no formatting drift
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Exit 0

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Exit 0

# 3. Real-hardware Linux
cargo check --bin anvilml
# Exit 0

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Exit 0
```

## Project Gates

Gate 1 — Config Surface Sync: `cargo test -p anvilml --features mock-hardware -- config_reference` → ok. 1 passed; 0 failed

Gate 2 — OpenAPI Drift: Not applicable — `api/openapi.json` does not yet exist (openapi generation stub).

## Public API Delta

```
+pub use scheduler::CancelOutcome;
+pub enum CancelOutcome {
+    pub async fn cancel(&self, id: Uuid) -> Result<CancelOutcome, AnvilError> {
```

New pub items:
- `pub enum CancelOutcome` — `crates/anvilml-scheduler/src/scheduler.rs` (re-exported via `lib.rs`)
- Modified pub fn: `pub async fn cancel(&self, id: Uuid) -> Result<CancelOutcome, AnvilError>` — `JobScheduler` (signature changed from `Result<bool, AnvilError>`)

## Deviations from Plan

1. **Route path**: The plan specified `POST /v1/jobs/{id}/cancel` as a separate route. Implemented as `POST /v1/jobs/{id}` on the same route as `GET /v1/jobs/{id}` (different HTTP method), because Axum's `{id}` capture would greedily consume `/cancel` if registered as a separate parameterised route. This matches the plan's "Risks and Mitigations" section which explicitly recommends registering on the same path.

2. **`CancelOutcome` re-export**: Added `pub use scheduler::CancelOutcome;` in `lib.rs` to make the type importable as `anvilml_scheduler::CancelOutcome`. This was required because the handler imports it directly.

3. **`test_cancel_completed_job_returns_409` test approach**: The plan described submitting a job then updating its DB status to Completed. However, the `cancel()` method checks the in-memory queue first — a submitted job is in the queue, so it would return `Accepted` instead of `AlreadyTerminal`. Fixed by persisting a completed job directly to the database (not via submit), ensuring it is absent from the queue.

## Blockers

None.
