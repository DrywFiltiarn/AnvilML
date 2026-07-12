# Plan Report: P18-E2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-E2                                            |
| Phase       | 18 — HTTP/WebSocket Server Completion             |
| Description | anvilml-server: DELETE /v1/jobs bulk clear handler |
| Depends on  | P18-E1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-12T15:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement the `bulk_clear_jobs` handler for `DELETE /v1/jobs?status=<value>`, completing
the job deletion surface begun by P18-E1's single-job `delete_job` handler. The handler
accepts a `status` query parameter (`completed`, `failed`, `cancelled`, or `all`), finds
all matching terminal jobs, reuses P18-E1's per-job delete logic for each one, and returns
`200 { removed: u32 }`. Invalid status values return `400`. The route is registered in
`build_router()`. This completes the REST route table from `ANVILML_DESIGN.md §13.4`.

## Scope

### In Scope
- `BulkClearParams` struct in `crates/anvilml-server/src/handlers/jobs.rs` with `status: String` query parameter.
- `RemovedCount` response struct in `jobs.rs` with `removed: u32` field.
- `bulk_clear_jobs()` handler function in `jobs.rs` that validates the status value, lists matching terminal jobs, and reuses P18-E1's per-job delete logic.
- Route registration: `.delete(handlers::jobs::bulk_clear_jobs)` on the `/v1/jobs` route in `build_router()`.
- Four new tests in `crates/anvilml-server/tests/jobs_tests.rs`: bulk_clear with each of the four valid status values, and bulk_clear with an invalid status returning 400.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no empty defers_to field and must implement its full scope. No deferred functionality.

## Existing Codebase Assessment

**What already exists:** P18-E1 has already implemented `delete_job()` in `jobs.rs` — a
handler that looks up a job by UUID, verifies it is terminal (Completed/Failed/Cancelled),
deletes all associated artifacts via `ArtifactStore::list()` + `ArtifactStore::delete()`,
then deletes the job row via `JobStore::delete()`. It returns `204 No Content` on success,
`409 Conflict` on non-terminal jobs, and `404 Not Found` on unknown IDs.

The `JobStore::list(status, limit)` method already supports filtering by `JobStatus`, which
is the exact persistence operation needed for bulk clear. The `ArtifactStore` provides
`list(job_id)` and `delete(hash)` methods for artifact cleanup. The `build_router()` in
`lib.rs` already registers GET and POST on `/v1/jobs`; the DELETE method needs to be added
to the same route.

**Established patterns:**
- Handler functions use `pub(crate) async fn`, take `State<AppState>`, and return
  `Result<..., AnvilError>` or `Result<(StatusCode, Json<...>), AnvilError>`.
- Request/response structs are `pub(crate)` with `#[derive(Debug, Serialize/Deserialize)]`.
- Query params use `Query<T>` extractor with `#[derive(Debug, Deserialize)]`.
- Logging uses `#[tracing::instrument]` with structured fields and `tracing::info!`/`debug!`/`warn!`.
- Tests use `make_test_state()` to construct `AppState` with an in-memory SQLite pool,
  `build_router(state)` to create the router, and `router.oneshot(req)` for HTTP calls.
- The `before` query parameter pattern (accepted at HTTP layer, ignored by persistence) is
  used as forward-compatibility — not needed here since `status` is the only parameter.

**Gap between design doc and current source:** The design doc (§13.4) specifies the bulk
clear route as `DELETE /v1/jobs` with `?status=completed|failed|cancelled|all`, returning
`200 { removed: u32 }`. The current `build_router()` registers only `.get().post()` on
`/v1/jobs`. The DELETE method needs to be added. There is no gap — the design doc is
accurate and the current code just hasn't implemented this route yet.

## Resolved Dependencies

None. This task introduces no new external crates or packages. All dependencies used are
already declared in `crates/anvilml-server/Cargo.toml` (axum, serde, uuid, tracing,
anvilml-core, anvilml-registry, anvilml-artifacts).

| Type   | Name   | Version verified | MCP source | Feature flags confirmed |
|--------|--------|-----------------|------------|------------------------|
| (none) | (none) | (none)          | (none)     | (none)                 |

## Approach

### Step 1: Add `BulkClearParams` and `RemovedCount` structs to `jobs.rs`

Add two new structs at the top of `jobs.rs`, following the existing pattern of
`ListJobsParams`/`SubmitJobRequest` and `SubmitJobResponse`:

```rust
/// HTTP query parameters for `DELETE /v1/jobs` (bulk clear endpoint).
///
/// The `status` parameter selects which terminal jobs to remove:
/// - `completed` — only Completed jobs
/// - `failed` — only Failed jobs
/// - `cancelled` — only Cancelled jobs
/// - `all` — all terminal jobs (Completed + Failed + Cancelled)
///
/// Returns `400 Bad Request` for any unrecognized value.
#[derive(Debug, Deserialize)]
pub(crate) struct BulkClearParams {
    /// Filter by job status. Must be one of: completed, failed, cancelled, all.
    pub status: String,
}

/// HTTP response body for `DELETE /v1/jobs` on success.
///
/// Per `ANVILML_DESIGN.md §13.4`: `200 { removed: u32 }`.
#[derive(Debug, Serialize)]
pub(crate) struct RemovedCount {
    /// Number of jobs removed by the bulk clear operation.
    pub removed: u32,
}
```

### Step 2: Extract per-job delete logic into a reusable private helper

The P18-E1 `delete_job` handler contains ~40 lines of artifact deletion + job deletion
logic. To satisfy the task's requirement of "reusing P18-E1's per-job delete logic, not a
divergent implementation," extract this logic into a private async helper function:

```rust
/// Delete a single job and all its associated artifacts.
///
/// This is the core deletion logic shared by both `delete_job` (single-job)
/// and `bulk_clear_jobs` (bulk). It performs three steps:
/// 1. List all artifacts for the job via `artifact_store.list(Some(id))`.
/// 2. Delete each artifact file + DB row, logging warnings on individual
///    failures but continuing with remaining artifacts.
/// 3. Delete the job row via `job_store.delete(id)`.
///
/// Returns the number of artifacts that were deleted (0 if none existed).
/// Errors from artifact deletion are logged but do not abort the deletion;
/// the job row is always deleted regardless of artifact deletion status.
async fn delete_single_job(
    state: &AppState,
    id: Uuid,
    job_store: &JobStore,
) -> Result<u32, AnvilError> {
    // ... existing delete_job body, minus the status check and 404 check ...
}
```

The `delete_job` handler is then refactored to call this helper after performing its
own preconditions (status check, 404 handling). The helper returns `u32` for the artifact
count, which the `delete_job` handler discards and the `bulk_clear_jobs` handler uses
to accumulate the total removed count.

This is a non-obvious structural choice: instead of duplicating the delete logic in
`bulk_clear_jobs`, we extract it into a shared private helper. This ensures both handlers
always stay in sync — any future fix to the delete semantics applies to both.

### Step 3: Implement `bulk_clear_jobs` handler

```rust
/// Bulk-clear terminal jobs matching the given status filter.
///
/// Accepts `DELETE /v1/jobs?status=<value>` where `<value>` is one of:
/// `completed`, `failed`, `cancelled`, or `all`. For each matching job,
/// delegates to `delete_single_job()` to remove the job and its artifacts.
///
/// # Response
///
/// - `200 OK` — `{ removed: u32 }` with the count of jobs removed.
/// - `400 Bad Request` — the `status` query parameter is not one of the
///   four recognized values.
///
/// # State Access
///
/// Reads from `state.db` via `JobStore` and from `state.artifact_store`
/// for artifact deletion.
#[tracing::instrument(skip(state), fields(status))]
pub(crate) async fn bulk_clear_jobs(
    State(state): State<AppState>,
    Query(params): Query<BulkClearParams>,
) -> Result<Json<RemovedCount>, AnvilError> {
    // Validate the status parameter — must be one of the four allowed values.
    // "all" is a special case that matches all terminal statuses; the other
    // three map directly to JobStatus variants.
    let job_status = match params.status.as_str() {
        "completed" => Some(JobStatus::Completed),
        "failed" => Some(JobStatus::Failed),
        "cancelled" => Some(JobStatus::Cancelled),
        "all" => None, // None = no status filter → all terminal jobs
        other => {
            tracing::warn!(
                status = %other,
                "bulk_clear rejected: unrecognized status value"
            );
            return Err(AnvilError::BadRequest(format!(
                "invalid status: {other}; must be completed, failed, cancelled, or all"
            )));
        }
    };

    // List all jobs matching the status filter. When status is "all" (None),
    // the list() call returns every job — but we only want terminal ones.
    // Since the API contract says bulk clear operates on terminal jobs only,
    // a None filter here means "no filter" which is correct because there
    // are no non-terminal jobs that should match "all" in normal operation.
    // However, to be safe, we filter out Queued/Running jobs in the loop
    // below.
    let job_store = JobStore::new(state.db.clone());
    let jobs = job_store.list(job_status, None).await?;

    let mut removed: u32 = 0;

    for job in &jobs {
        // Skip non-terminal jobs — only Completed, Failed, Cancelled are
        // eligible for bulk deletion. Queued and Running jobs must be
        // cancelled first. This is a safety guard: if "all" returns any
        // non-terminal jobs (shouldn't happen in normal operation), we
        // skip them rather than deleting active work.
        if matches!(job.status, JobStatus::Queued | JobStatus::Running) {
            tracing::debug!(
                job_id = %job.id,
                status = ?job.status,
                "bulk_clear skipping non-terminal job"
            );
            continue;
        }

        // Delegate to the shared delete_single_job helper. This ensures
        // bulk clear uses the exact same artifact-deletion-and-job-deletion
        // logic as the single-job handler — no divergence.
        match delete_single_job(&state, job.id, &job_store).await {
            Ok(_) => removed += 1,
            Err(e) => {
                tracing::warn!(
                    job_id = %job.id,
                    error = %e,
                    "bulk_clear: failed to delete job — continuing with remaining jobs"
                );
                // Continue processing remaining jobs even if one fails.
                // The removed count reflects only successfully deleted jobs.
            }
        }
    }

    tracing::info!(removed, status = %params.status, "bulk clear completed");
    Ok(Json(RemovedCount { removed }))
}
```

**Rationale for the "all" handling:** When `status=all`, we pass `None` to `list()`,
which returns all jobs in the database. The loop then filters out non-terminal jobs
with the `matches!` guard. This is correct because:
1. In normal operation, non-terminal jobs should not exist in the database without an
   active worker — but ghost jobs can exist after a crash (handled by `reset_ghost_jobs`).
2. Filtering at the handler level is safer than trying to construct a SQL `IN` clause,
   which would require static query strings for all three terminal statuses.
3. The per-job delete logic already checks terminal status, so this is a double-guard.

**Rationale for continuing on error:** Individual job deletion failures (e.g. artifact
store I/O errors) should not abort the entire bulk operation. The handler logs the error
and continues, returning a count of only successfully removed jobs. This is consistent
with P18-E1's `delete_job` which also continues after individual artifact deletion failures.

### Step 4: Refactor `delete_job` to use `delete_single_job`

Replace the body of `delete_job` (lines 239–295) with a call to `delete_single_job`:

```rust
// After the status check and 404 check pass:
let job_store = JobStore::new(state.db.clone());
let _artifact_count = delete_single_job(&state, id, &job_store).await?;
```

The `_artifact_count` binding is unused by `delete_job` (it only returns 204, not a body),
so it is prefixed with `_` to suppress the dead-code warning.

### Step 5: Register the route in `build_router()`

Update the `/v1/jobs` route in `lib.rs` from:

```rust
.route(
    "/v1/jobs",
    axum::routing::get(handlers::jobs::list_jobs).post(handlers::jobs::submit_job),
)
```

To:

```rust
.route(
    "/v1/jobs",
    axum::routing::get(handlers::jobs::list_jobs)
        .post(handlers::jobs::submit_job)
        .delete(handlers::jobs::bulk_clear_jobs),
)
```

The DELETE method is registered on the same `/v1/jobs` path (not `/v1/jobs/{id}`) because
bulk clear operates on collections, not individual jobs — consistent with `ANVILML_DESIGN.md
§13.4`'s route table.

### Step 6: Write tests in `jobs_tests.rs`

Add four new test functions:

1. **`test_bulk_clear_completed_status`** — Create 3 completed jobs + 1 queued job via
   direct DB persistence. Call `DELETE /v1/jobs?status=completed`. Assert 200 with
   `{ removed: 3 }`. Verify only the 3 completed jobs are gone, the queued job remains.

2. **`test_bulk_clear_failed_status`** — Create 2 failed jobs + 1 completed job. Call
   `DELETE /v1/jobs?status=failed`. Assert 200 with `{ removed: 2 }`. Verify the
   completed job remains.

3. **`test_bulk_clear_cancelled_status`** — Create 2 cancelled jobs + 1 failed job. Call
   `DELETE /v1/jobs?status=cancelled`. Assert 200 with `{ removed: 2 }`. Verify the
   failed job remains.

4. **`test_bulk_clear_all_status`** — Create 1 completed, 1 failed, 1 cancelled, and
   1 queued job. Call `DELETE /v1/jobs?status=all`. Assert 200 with `{ removed: 3 }`.
   Verify the queued job remains.

5. **`test_bulk_clear_invalid_status_returns_400`** — Call `DELETE /v1/jobs?status=unknown`.
   Assert 400 Bad Request. Verify no jobs are deleted.

Each test follows the established `make_test_state()` + `build_router()` + `router.oneshot()`
pattern. Jobs are persisted directly to the database (not via POST submit) to control
their status precisely — the same pattern used by `test_delete_terminal_job_returns_204`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| Struct | `anvilml_server::handlers::jobs::BulkClearParams` | `pub(crate) struct BulkClearParams { pub status: String }` |
| Struct | `anvilml_server::handlers::jobs::RemovedCount` | `pub(crate) struct RemovedCount { pub removed: u32 }` |
| Function | `anvilml_server::handlers::jobs::bulk_clear_jobs` | `pub(crate) async fn bulk_clear_jobs(State<AppState>, Query<BulkClearParams>) -> Result<Json<RemovedCount>, AnvilError>` |
| Function (private) | `anvilml_server::handlers::jobs::delete_single_job` | `async fn delete_single_job(&AppState, Uuid, &JobStore) -> Result<u32, AnvilError>` |

Note: `BulkClearParams` and `RemovedCount` are `pub(crate)` (not `pub`), so they are not
part of the external crate API — they are only visible within the `anvilml-server` crate.
This is consistent with how `ListJobsParams`, `SubmitJobRequest`, and `SubmitJobResponse`
are defined.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/handlers/jobs.rs` | Add `BulkClearParams`, `RemovedCount`, `delete_single_job()` helper, and `bulk_clear_jobs()` handler; refactor `delete_job` to use the helper |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Register `.delete(bulk_clear_jobs)` on `/v1/jobs` route in `build_router()` |
| MODIFY | `crates/anvilml-server/tests/jobs_tests.rs` | Add 5 new test functions for bulk clear |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.25 → 0.1.26 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| jobs_tests.rs | test_bulk_clear_completed_status | DELETE /v1/jobs?status=completed removes only Completed jobs, returns { removed: N } | `cargo test -p anvilml-server --test jobs_tests test_bulk_clear_completed_status` exits 0 |
| jobs_tests.rs | test_bulk_clear_failed_status | DELETE /v1/jobs?status=failed removes only Failed jobs, returns { removed: N } | `cargo test -p anvilml-server --test jobs_tests test_bulk_clear_failed_status` exits 0 |
| jobs_tests.rs | test_bulk_clear_cancelled_status | DELETE /v1/jobs?status=cancelled removes only Cancelled jobs, returns { removed: N } | `cargo test -p anvilml-server --test jobs_tests test_bulk_clear_cancelled_status` exits 0 |
| jobs_tests.rs | test_bulk_clear_all_status | DELETE /v1/jobs?status=all removes all terminal jobs (Completed+Failed+Cancelled), skips non-terminal, returns { removed: N } | `cargo test -p anvilml-server --test jobs_tests test_bulk_clear_all_status` exits 0 |
| jobs_tests.rs | test_bulk_clear_invalid_status_returns_400 | DELETE /v1/jobs?status=unknown returns 400 Bad Request, no jobs deleted | `cargo test -p anvilml-server --test jobs_tests test_bulk_clear_invalid_status_returns_400` exits 0 |

Acceptance command for the full test suite:
`cargo test -p anvilml-server --test jobs_tests` exits 0 (>=25 total tests: 20 existing + 5 new).

## CI Impact

No CI changes required. The new tests run as part of the existing `cargo test --workspace
--features mock-hardware` command, which is executed by the `rust-linux` and `rust-windows`
CI jobs. The handler does not introduce new dependencies, new file types, or new module
boundaries that would change CI behavior.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The handler
uses only database operations and string comparisons — no platform-specific file paths,
no `#[cfg(unix)]`/`#[cfg(windows)]` guards needed. The `JobStore::list()` SQL queries
use parameter binding which is platform-neutral across SQLite's cross-platform implementation.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `AnvilError::BadRequest` may not exist or may not accept a `String` argument — the error enum's `BadRequest` variant needs to be checked for its actual constructor signature before writing the handler. | Low | Medium | Read `anvilml-core/src/error.rs` to confirm `BadRequest` takes a `String` or `&str`. If it takes a different type, use the correct constructor. |
| Bulk clear with `status=all` returns non-terminal jobs from the database (ghost jobs after a crash). The handler's loop-level filter skips them, but this means the count may be lower than expected by a client that assumes "all" means "all rows." | Low | Low | The handler logs skipped non-terminal jobs at DEBUG level. The safety guard is intentional — deleting active work would be a data loss bug. The test `test_bulk_clear_all_status` verifies the guard works. |
| Extracting `delete_single_job` into a shared helper changes the structure of `delete_job`. If the extraction is incomplete, `delete_job` may stop working. | Medium | High | The extraction preserves the exact same logic — only the status check and 404 check remain in `delete_job`, and the body is replaced by a single function call. All 4 existing delete tests (terminal, artifacts, non-terminal queued, non-terminal running, unknown) must pass after the refactor. |
| The `status` query parameter is required (no `Option<String>`), so a request without `?status=` returns a 400 from axum's query deserializer before the handler runs. This is correct behavior per the spec but must be tested. | Low | Low | The `test_bulk_clear_invalid_status_returns_400` test covers the explicit invalid value case. A missing `?status=` parameter is handled by axum (it rejects the request because `status: String` has no default), which also returns 400 — same outcome. |

## Acceptance Criteria

- [ ] `head -1 .forge/reports/P18-E2_plan.md` prints `# Plan Report: P18-E2`
- [ ] `grep "^## " .forge/reports/P18-E2_plan.md` shows all 12 required section headings
- [ ] `wc -l .forge/reports/P18-E2_plan.md` outputs a number > 40
- [ ] `cargo test -p anvilml-server --test jobs_tests` exits 0 with >=25 tests (20 existing + 5 new)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
