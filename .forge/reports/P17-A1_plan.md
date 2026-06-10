# Plan Report: P17-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-A1                                      |
| Phase       | 017 — Job & Artifact Management             |
| Description | anvilml-server: DELETE /v1/jobs/:id (terminal only, with artifacts) |
| Depends on  | P16-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-10T13:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Add single-job deletion support to the AnvilML server: a `DELETE /v1/jobs/:id` endpoint that removes a terminal job's on-disk artifacts and its database row, while rejecting deletion of Running or Queued jobs with a 409 Conflict.

## Scope

### In Scope
- Add `delete_for_job(job_id: &str) -> Result<u32>` to `crates/anvilml-server/src/artifact/store.rs` — deletes on-disk artifact files and artifact DB rows for a given job_id, returns count of deleted artifact rows.
- Add `delete_job(State<Arc<App>>, Path<Uuid>)` handler to `crates/anvilml-server/src/handlers/jobs.rs` — reads the job, rejects Running/Queued with 409, otherwise calls `artifact_store.delete_for_job()` then deletes the job row, returns 204.
- Wire `DELETE /v1/jobs/{id}` route in `crates/anvilml-server/src/lib.rs` `build_router()`.
- Add unit tests for `delete_for_job` in `artifact/store.rs`.
- Add unit tests for `delete_job` handler in `handlers/jobs.rs`.

### Out of Scope
- Bulk clear endpoint (`DELETE /v1/jobs?status=...`) — handled by P17-A2.
- Integration test file (`backend/tests/api_delete.rs`) — handled by P17-A3.
- Any changes to the scheduler crate, worker pool, or IPC protocol.
- Any changes to the SQLite schema (the `jobs` and `artifacts` tables already exist from prior phases).

## Approach

1. **Add `delete_for_job` to `artifact/store.rs`:**
   - Query `SELECT hash FROM artifacts WHERE job_id = ?` for the given job_id.
   - For each returned hash, build the on-disk path using `get_path()` logic (`{artifact_dir}/{hash[0..2]}/{hash}.png`) and `tokio::fs::remove_file`.
   - After file deletion, execute `DELETE FROM artifacts WHERE job_id = ?` and return the count of deleted rows.
   - If the DB delete fails, the files are already gone (best-effort cleanup) — log a warning.
   - Log at DEBUG level: number of files deleted, rows deleted.
   - Signature: `pub async fn delete_for_job(&self, job_id: &str) -> Result<u32, ArtifactError>`

2. **Add `delete_job` handler to `handlers/jobs.rs`:**
   - Accept `State<Arc<App>>` and `Path<Uuid>`.
   - Read the job via `scheduler_get_job(pool, job_id)`. Return 404 if not found.
   - If job status is `Queued` or `Running`, return 409 with `{"error": "job_active", "message": "..."}`.
   - Call `state.artifact_store.delete_for_job(&job_id.to_string())` — log on failure but continue.
   - Execute `DELETE FROM jobs WHERE id = ?` via a direct sqlx query (or add `delete_job` to `job_store.rs`).
   - Return 204 No Content.
   - Add utoipa path annotation for the DELETE endpoint.

3. **Wire the route in `lib.rs`:**
   - Add `.route("/v1/jobs/{id}", delete(handlers::jobs::delete_job))` — note: the existing `get` route for `/v1/jobs/{id}` already exists, so we use `.route("/v1/jobs/{id}", get(handlers::jobs::get_job).delete(handlers::jobs::delete_job))`.

4. **Add tests:**
   - In `artifact/store.rs`: test `delete_for_job` with in-memory SQLite — insert artifacts for a job, call `delete_for_job`, verify DB rows gone and files removed from temp dir.
   - In `handlers/jobs.rs`: test `delete_job` — submit job, set to Completed in DB, DELETE returns 204, GET returns 404; submit job, DELETE while Queued returns 409.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/artifact/store.rs` | Add `delete_for_job()` method + unit tests |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `delete_job()` handler + unit tests |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `DELETE /v1/jobs/{id}` route in `build_router()` |
| Bump   | `crates/anvilml-server/Cargo.toml` | Patch version `0.1.9 → 0.1.10` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/artifact/store.rs` (mod tests) | `delete_for_job_removes_files_and_rows` | Inserts artifacts for a job, calls `delete_for_job`, verifies DB rows = 0, files removed from disk, returns correct count |
| `crates/anvilml-server/src/artifact/store.rs` (mod tests) | `delete_for_job_empty_returns_zero` | Calls `delete_for_job` for a job with no artifacts, verifies returns 0 |
| `crates/anvilml-server/src/handlers/jobs.rs` (mod tests) | `delete_job_returns_204_for_completed_job` | Submits job, sets status to Completed in DB, DELETE returns 204, GET returns 404 |
| `crates/anvilml-server/src/handlers/jobs.rs` (mod tests) | `delete_job_returns_409_for_queued_job` | Submits job (Queued), DELETE returns 409 with `job_active` error |
| `crates/anvilml-server/src/handlers/jobs.rs` (mod tests) | `delete_job_returns_404_when_missing` | DELETE nonexistent job UUID returns 404 |

## CI Impact

No CI workflow files are modified. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy`, format check, platform cross-checks) will automatically exercise the new code. The new unit tests are in the `anvilml-server` crate and run as part of the standard test suite. No OpenAPI drift gate is triggered since no existing handler signatures change — only a new handler is added.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::fs::remove_file` fails for a file that was already deleted (race condition) | Low | Minor — one artifact file remains on disk | Catch `io::ErrorKind::NotFound` per-file and continue; log at DEBUG |
| DB error during artifact deletion leaves orphan files on disk | Low | Disk space leak | Delete files first, then DB — if DB fails, files are already gone (best-effort) |
| Route conflict: existing `GET /v1/jobs/{id}` and new `DELETE /v1/jobs/{id}` in same `.route()` call | Low | Compile error | Use axum's chained router syntax: `.route("/v1/jobs/{id}", get(...).delete(...))` which is already used elsewhere in the codebase |
| Missing `JobNotCancellable` semantic mismatch for delete (currently used for cancel) | Low | Minor — error variant name doesn't perfectly describe delete scenario | Reuse `JobNotCancellable` since it serves the same purpose (rejecting non-terminal jobs); a dedicated `JobActive` variant would require modifying `anvilml-core` which is out of scope |

## Acceptance Criteria

- [ ] `delete_for_job` method exists on `ArtifactStore` with signature `async fn delete_for_job(&self, job_id: &str) -> Result<u32, ArtifactError>`
- [ ] `delete_for_job` deletes all on-disk artifact files for the given job_id before deleting DB rows
- [ ] `delete_job` handler returns 204 for terminal jobs (Completed, Failed, Cancelled)
- [ ] `delete_job` handler returns 409 for non-terminal jobs (Queued, Running)
- [ ] `delete_job` handler returns 404 when job UUID does not exist
- [ ] After successful DELETE, GET /v1/jobs/:id returns 404
- [ ] After successful DELETE, artifact files are removed from disk
- [ ] DELETE route is wired in `build_router()` as `DELETE /v1/jobs/{id}`
- [ ] All unit tests in `artifact/store.rs` and `handlers/jobs.rs` pass (`cargo test -p anvilml-server --features mock-hardware`)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `crates/anvilml-server/Cargo.toml` patch version bumped from 0.1.9 to 0.1.10
