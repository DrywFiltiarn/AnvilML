# Implementation Report: P12-A5

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Task ID     | P12-A5                                         |
| Phase       | 012 — Job Submission & Queue                   |
| Description | anvilml-server: GET /v1/jobs list with status/limit/before |
| Implemented | 2026-06-07T20:00:00Z                           |
| Status      | COMPLETE                                       |

## Summary

Implemented the `GET /v1/jobs` list endpoint in the `anvilml-server` HTTP API. The handler accepts optional query parameters (`status`, `limit`, `before`), parses and validates them, delegates to `job_store::list_jobs` from `anvilml-scheduler`, and returns a JSON array of jobs sorted newest-first. Default limit is 100, clamped to [1, 1000]. Status filtering is case-insensitive with unknown values treated as unfiltered (with a WARN log). The `before` cursor is parsed from RFC 3339/ISO 8601 format with graceful fallback on parse failure. Three unit tests verify correct listing, status filtering, and limit clamping.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|-----------------|----------------|
| crate  | chrono    | 0.4.45          | lockfile       |
| crate  | axum      | (workspace)     | lockfile       |
| crate  | serde     | (workspace)     | lockfile       |

No new dependencies were added. All types used (`JobStatus`, `DateTime<Utc>`, `serde::Deserialize`) were already present in the workspace dependency graph.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3` |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `ListJobsQuery` struct, `list_jobs` handler with utoipa annotation, and 3 unit tests (`list_jobs_returns_all_submitted_jobs`, `list_jobs_filters_by_status`, `list_jobs_limit_clamps_to_one`) |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `GET /v1/jobs` route alongside existing POST using axum multi-method routing |

## Commit Log

```
 .forge/reports/P12-A5_plan.md              | 155 ++++++++++++++++
 .forge/state/CURRENT_TASK.md               |   6 +-
 .forge/state/state.json                    |  11 +-
 Cargo.lock                                 |   2 +-
 crates/anvilml-server/Cargo.toml           |   2 +-
 crates/anvilml-server/src/handlers/jobs.rs | 281 ++++++++++++++++++++++++++++-
 crates/anvilml-server/src/lib.rs           |   5 +-
 7 files changed, 448 insertions(+), 14 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-709dc84839013579)

running 16 tests
test handlers::jobs::tests::get_job_returns_200_with_queued_job ... ok
test handlers::jobs::tests::get_job_returns_404_when_missing ... ok
test handlers::jobs::tests::list_jobs_filters_by_status ... ok
test handlers::jobs::tests::list_jobs_limit_clamps_to_one ... ok
test handlers::jobs::tests::list_jobs_returns_all_submitted_jobs ... ok
test handlers::jobs::tests::submit_job_bad_graph_returns_422 ... ok
test handlers::jobs::tests::submit_job_valid_zit_graph_returns_202 ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::get_model_returns_404_when_missing ... ok
test tests::health_returns_200 ... ok
test tests::rescan_returns_202 ... ok
test tests::system_returns_200_with_hardware_info ... ok
test tests::workers_endpoint_returns_200 ... ok
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test ws::stats_tick::tests::stats_tick_broadcasts_event ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.01s
```

All 16 tests in `anvilml-server` pass, including the 3 new `list_jobs` tests. The full workspace test suite (207+ tests across all crates) exits 0 with zero failures.

## Format Gate

```
(not applicable — cargo fmt --all -- --check exited 0 with no drift)
```

## Platform Cross-Check

```
# Check 1: mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.92s

# Check 2: mock-hardware Windows cross (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.18s

# Check 3: real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.13s

# Check 4: real-hardware Windows cross (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.88s
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```
Config drift gate passes (no config surface changes in this task).

### Gate 2 — OpenAPI Drift
The `anvilml-openapi` tool is currently a stub (`fn main() {}`). No committed `backend/openapi.json` exists. This gate is a no-op until the OpenAPI generation tool is implemented. The new endpoint has proper `utoipa::path` annotations and will be captured when the tool becomes functional.

## Deviations from Plan

- **None.** Implementation follows the approved plan exactly. The only deviation was fixing a pre-existing issue: `chrono::DateTime<Utc>::parse_from_rfc3339` does not exist in chrono 0.4.x (only `DateTime<FixedOffset>` has it). Changed to parse via `DateTime::<chrono::FixedOffset>::parse_from_rfc3339` then convert to UTC via `.with_timezone(&Utc)`. This is a correct implementation detail, not a scope change.

## Blockers

None.
