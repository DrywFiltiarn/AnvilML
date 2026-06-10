# Plan Report: P17-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P17-A2                                            |
| Phase       | 017 — Job & Artifact Management                   |
| Description | anvilml-server: DELETE /v1/jobs bulk clear by status |
| Depends on  | P17-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-10T14:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add bulk job deletion by status to the AnvilML server API. Implement a `delete_by_status` helper in the scheduler's job store and a `clear_jobs` HTTP handler that, for each matched terminal job, deletes its artifacts and DB row. Wire `DELETE /v1/jobs?status=...` into the router. Never delete Running or Queued jobs.

## Scope

### In Scope
- Add `delete_by_status(pool, status_filter) -> Vec<Uuid>` to `anvilml-scheduler::job_store`
- Add `ClearJobsQuery` struct (`status: Option<String>`) to `anvilml-server::handlers::jobs`
- Add `clear_jobs` async handler that:
  - Accepts `?status=completed|failed|cancelled|all` (defaults to `all`)
  - Calls `delete_by_status` to get matching job IDs
  - Iterates matched IDs, calls `artifact_store.delete_for_job` then `DELETE FROM jobs` per job
  - Returns `{ "removed": <count> }` with 200 OK
  - Validates status parameter, returns 400 for invalid values
- Wire `DELETE /v1/jobs` route in `build_router` (adds `.delete()` to the existing `/v1/jobs` route)
- Add unit tests in `handlers/jobs.rs` mirroring the existing test pattern
- Bump `anvilml-server` crate patch version: 0.1.10 → 0.1.11

### Out of Scope
- Integration test file (covered by P17-A3)
- `DELETE /v1/jobs/:id` handler (covered by P17-A1)
- Any changes to the scheduler's dispatch loop or worker pool
- WebSocket event broadcasting on bulk delete (future enhancement)
- OpenAPI spec regeneration (handled by CI drift gate, not this task's direct work)

## Approach

1. **Add `delete_by_status` to `job_store.rs`**
   - Signature: `pub async fn delete_by_status(pool: &SqlitePool, status_filter: Option<&str>) -> Result<Vec<Uuid>, sqlx::Error>`
   - Parse `status_filter`: `None` or `"all"` → WHERE status IN ('Completed','Failed','Cancelled'); exact match → WHERE status = ?
   - SQL: `SELECT id FROM jobs WHERE (status IN ('Completed','Failed','Cancelled') OR ? IS NULL) ORDER BY created_at DESC`
   - Return the list of Uuids — the handler will do the actual deletion per-job to preserve atomicity and artifact cleanup
   - Add at end of file before the `#[cfg(test)]` block

2. **Add `ClearJobsQuery` and `clear_jobs` handler to `handlers/jobs.rs`**
   - Define `ClearJobsQuery` struct with `status: Option<String>` (Deserialize, Default)
   - Define `ClearJobsResponse` struct (Serialize, ToSchema) with `removed: u32`
   - Implement `clear_jobs` async handler:
     a. Extract `pool` from `state.db`, return 503 if absent
     b. Parse `status` query param: `completed|failed|cancelled|all` (case-insensitive); 400 for invalid
     c. Call `delete_by_status(pool, parsed_filter)` — 500 on DB error
     d. For each Uuid: call `state.artifact_store.delete_for_job(id_str)` (log warn on failure, continue), then `DELETE FROM jobs WHERE id = ?` (log error on failure, count the failure)
     e. Return `{ "removed": <count> }` with 200 OK
     f. Log at INFO: `bulk_delete: cleared N jobs`
   - Add `#[utoipa::path]` annotation for OpenAPI generation

3. **Wire DELETE route in `lib.rs`**
   - Change the existing `/v1/jobs` route from:
     ```rust
     .route("/v1/jobs", get(handlers::jobs::list_jobs).post(handlers::jobs::submit_job))
     ```
     to:
     ```rust
     .route("/v1/jobs", get(handlers::jobs::list_jobs).post(handlers::jobs::submit_job).delete(handlers::jobs::clear_jobs))
     ```

4. **Add unit tests in `handlers/jobs.rs`**
   - `clear_jobs_returns_200_for_completed_jobs`: submit jobs → set to Completed → DELETE ?status=completed → verify `{removed:N}` → verify list returns fewer
   - `clear_jobs_removes_artifacts`: insert job with artifact → set to Completed → DELETE → verify artifact file gone
   - `clear_jobs_skips_running_jobs`: submit job (Queued) → DELETE ?status=all → verify running job untouched
   - `clear_jobs_rejects_invalid_status`: DELETE ?status=running → 400
   - `clear_jobs_defaults_to_all`: submit mixed-status jobs → DELETE with no param → only terminal deleted

5. **Bump version**
   - Edit `crates/anvilml-server/Cargo.toml`: `version = "0.1.10"` → `version = "0.1.11"`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Add `delete_by_status(pool, status_filter) -> Result<Vec<Uuid>, sqlx::Error>` function |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `ClearJobsQuery`, `ClearJobsResponse`, `clear_jobs` handler, and 5 unit tests |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `.delete(handlers::jobs::clear_jobs)` to `/v1/jobs` route |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/src/handlers/jobs.rs` | `clear_jobs_returns_200_for_completed_jobs` | Bulk delete with `?status=completed` returns correct count and removes only completed jobs |
| `crates/anvilml-server/src/handlers/jobs.rs` | `clear_jobs_removes_artifacts` | On-disk artifact files are deleted alongside job rows |
| `crates/anvilml-server/src/handlers/jobs.rs` | `clear_jobs_skips_running_jobs` | Running/Queued jobs are never deleted by bulk clear |
| `crates/anvilml-server/src/handlers/jobs.rs` | `clear_jobs_rejects_invalid_status` | Invalid status parameter returns 400 |
| `crates/anvilml-server/src/handlers/jobs.rs` | `clear_jobs_defaults_to_all` | No status param deletes all terminal jobs (Completed+Failed+Cancelled) |

## CI Impact

The route change on `/v1/jobs` requires the OpenAPI drift gate (`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json`) to be run after implementation, since new utoipa annotations and a new route are added. All existing tests continue to pass since the GET and POST routes on `/v1/jobs` remain unchanged. The `anvilml-scheduler` crate gains a new `pub` function but no existing public API changes, so no downstream breakage expected.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| SQL injection via status parameter | Low | High | Use parameterized queries only; status is validated against a whitelist before being passed to SQL |
| Partial deletion (some jobs deleted, others fail) | Medium | Medium | Iterate all matched jobs; log failures per-job but continue processing; return total count of successfully deleted jobs |
| Artifact store failure blocks job deletion | Low | Medium | `delete_for_job` is best-effort; log warn on failure but continue to job row deletion |
| Concurrent modifications during bulk delete | Low | Low | Each job deletion is a separate DB transaction; SQLite serializes writes naturally |
| OpenAPI spec drift | Medium | Low | Run `cargo run -p anvilml-openapi` after implementation to regenerate; commit updated `openapi.json` |

## Acceptance Criteria

- [ ] `delete_by_status` function exists in `anvilml-scheduler::job_store` with correct signature and returns matching Uuids
- [ ] `DELETE /v1/jobs?status=completed` returns `{ "removed": N }` with 200 and removes only completed jobs + their artifacts
- [ ] `DELETE /v1/jobs?status=all` removes all terminal jobs (Completed, Failed, Cancelled) but never Running or Queued
- [ ] `DELETE /v1/jobs?status=running` returns 400 with error message
- [ ] `DELETE /v1/jobs` with no status param defaults to `all` behavior
- [ ] `cargo test --workspace --features mock-hardware` passes with zero failures
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes with zero warnings
- [ ] `anvilml-server` version bumped to 0.1.11 in Cargo.toml
