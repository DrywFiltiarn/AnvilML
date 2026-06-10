# Plan Report: P17-A3

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P17-A3                                        |
| Phase       | 017 — Job & Artifact Management               |
| Description | anvilml: integration test for job + artifact deletion |
| Depends on  | P17-A1, P17-A2                                |
| Project     | anvilml                                       |
| Planned at  | 2026-06-10T17:05:00Z                          |
| Attempt     | 1                                             |

## Objective

Create a comprehensive integration test file `backend/tests/api_delete.rs` that verifies the full job and artifact deletion lifecycle implemented in Phase 17 tasks P17-A1 and P17-A2. The tests confirm that single-job deletion (`DELETE /v1/jobs/:id`) removes both the database row and on-disk artifact files, that deletion of non-terminal jobs returns 409, and that bulk deletion (`DELETE /v1/jobs?status=all`) removes all terminal jobs while preserving active ones.

## Scope

### In Scope
- Create `backend/tests/api_delete.rs` with integration tests for:
  1. **Single job delete with artifact**: Submit a job, advance to Completed via direct DB update, insert a fake artifact file + row, assert artifact file exists on disk, `DELETE /v1/jobs/:id` returns 204, assert DB row gone, assert artifact file gone.
  2. **Single job delete — Running job**: Submit a job, advance to Running via DB, `DELETE /v1/jobs/:id` returns 409 with `job_active` error.
  3. **Bulk clear all terminal jobs**: Create multiple terminal jobs (Completed, Failed, Cancelled) with artifacts, `DELETE /v1/jobs?status=all` returns `{ "removed": N }`, verify all terminal jobs and their artifacts are gone, verify Running/Queued jobs remain untouched.
  4. **Bulk clear by specific status**: Create Completed and Failed jobs, `DELETE /v1/jobs?status=completed` removes only completed jobs, Failed jobs remain.
  5. **GET after delete returns 404**: After single delete, `GET /v1/jobs/:id` returns 404.
- Test isolation: each test uses its own temp directory for artifacts and its own in-memory DB pool via `open_in_memory()`.
- Environment variable management using `temp_env::async_with_vars` with unconditional cleanup.

### Out of Scope
- Modifying any production source code (handlers, artifact store, scheduler, router) — those are covered by P17-A1 and P17-A2.
- Tests for edge cases not listed above (e.g. artifact deletion failure during job delete, concurrent deletes).
- WebSocket event assertions for delete operations.
- Python worker integration — tests use direct DB manipulation to simulate job completion instead of spawning a real worker.

## Approach

### Test Infrastructure (shared helpers)

1. **`build_test_app()`** — Reuse the same pattern from `backend/tests/api_cancel.rs`: create an in-memory SQLite pool via `anvilml_registry::open_in_memory()`, a temp directory for artifacts, a `JobScheduler` with a mock `WorkerPool` (`new_test_pool()`), an `EventBroadcaster`, and an `App` via `anvilml_server::App::new(...)`. Returns the `App`, DB pool, scheduler, workers Arc, and broadcaster Arc.

2. **`minimal_zit_graph()`** — Reuse the same minimal valid ZiT 2-node graph from `api_cancel.rs` (ZitLoadPipeline + ZitTextEncode).

3. **`submit_job_via_http(port, graph)`** — Helper that POSTs a job via hyper to `http://127.0.0.1:{port}/v1/jobs`, parses the 202 response, and returns the `Uuid`.

4. **`insert_artifact_on_disk(artifact_dir, job_id, hash)`** — Helper that creates the sharded directory `{artifact_dir}/{hash[..2]}/`, writes a fake PNG file, and inserts an `artifacts` DB row via sqlx.

5. **`advance_job_status(pool, job_id, status)`** — Helper that updates a job's status in the DB using `UPDATE jobs SET status = ?, completed_at = ? WHERE id = ?`.

### Test 1: `delete_completed_job_removes_artifact_and_row`

1. Set env vars via `temp_env::async_with_vars`: `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=8192`, `ANVILML_WORKER_MOCK=1`.
2. Build test app (in-memory DB, temp artifact dir).
3. Start server on random port via `TcpListener::bind("127.0.0.1:0")`.
4. Submit a job via HTTP POST, capture `job_id`.
5. Advance job to `Completed` via direct DB update (`UPDATE jobs SET status = 'Completed', completed_at = ?`).
6. Insert a fake artifact: write `{artifact_dir}/aa/aaa111bbb222ccc333ddd444eee555fff666aaa777bbb888ccc999ddd000.png` with fake bytes, insert `artifacts` row with matching `job_id`.
7. Assert artifact file exists on disk (`std::fs::metadata`).
8. Assert job row exists in DB (`SELECT COUNT(*) FROM jobs WHERE id = ?`).
9. `DELETE /v1/jobs/{job_id}` via hyper → assert 204 No Content.
10. Assert job row gone (`SELECT COUNT(*)` returns 0).
11. Assert artifact file gone (`std::fs::metadata` returns `NotFound`).
12. `GET /v1/jobs/{job_id}` → assert 404.
13. Abort server handle.

### Test 2: `delete_running_job_returns_409`

1. Same env var setup and test app build.
2. Start server on random port.
3. Submit a job via HTTP POST, capture `job_id`.
4. Advance job to `Running` via DB update.
5. `DELETE /v1/jobs/{job_id}` → assert 409 Conflict.
6. Assert response body contains `"error": "job_active"`.
7. Assert job row still exists (not deleted).
8. Abort server handle.

### Test 3: `bulk_delete_all_terminal_jobs`

1. Same env var setup and test app build.
2. Start server on random port.
3. Submit 3 jobs via HTTP POST, capture `job_id_1`, `job_id_2`, `job_id_3`.
4. Submit 1 more job, advance to `Running` (active job that must survive).
5. Advance job 1 to `Completed`, insert artifact file + DB row.
6. Advance job 2 to `Failed`, insert artifact file + DB row.
7. Advance job 3 to `Cancelled`, insert artifact file + DB row.
8. `DELETE /v1/jobs?status=all` via hyper → assert 200, parse body, assert `removed == 3`.
9. Assert all 3 terminal jobs are gone from DB.
10. Assert all 3 artifact files are gone from disk.
11. Assert the Running job still exists in DB with status `Running`.
12. Abort server handle.

### Test 4: `bulk_delete_by_status_removes_only_matching`

1. Same env var setup and test app build.
2. Start server on random port.
3. Submit 2 jobs, capture `job_id_completed`, `job_id_failed`.
4. Advance job 1 to `Completed`, insert artifact + DB row.
5. Advance job 2 to `Failed`, insert artifact + DB row.
6. `DELETE /v1/jobs?status=completed` → assert 200, parse body, assert `removed == 1`.
7. Assert completed job is gone from DB.
8. Assert completed artifact file is gone.
9. Assert failed job still exists in DB with status `Failed`.
10. Assert failed artifact file still exists on disk.
11. Abort server handle.

### Test 5: `delete_nonexistent_job_returns_404`

1. Same env var setup and test app build.
2. Start server on random port.
3. `DELETE /v1/jobs/{nonexistent_uuid}` → assert 404.
4. Assert response body contains `"error": "not_found"`.
5. Abort server handle.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `backend/tests/api_delete.rs` | Integration test file with 5 tests for job + artifact deletion |

No production source files are modified. This task only adds a test file.

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `backend/tests/api_delete.rs` | `delete_completed_job_removes_artifact_and_row` | Single DELETE on Completed job → 204, DB row removed, artifact file removed, GET returns 404 |
| `backend/tests/api_delete.rs` | `delete_running_job_returns_409` | DELETE on Running job → 409 with `job_active` error, job not deleted |
| `backend/tests/api_delete.rs` | `bulk_delete_all_terminal_jobs` | `DELETE /v1/jobs?status=all` removes 3 terminal jobs + artifacts, preserves Running job, returns `{removed:3}` |
| `backend/tests/api_delete.rs` | `bulk_delete_by_status_removes_only_matching` | `DELETE /v1/jobs?status=completed` removes only completed jobs + artifacts, preserved failed job + artifact |
| `backend/tests/api_delete.rs` | `delete_nonexistent_job_returns_404` | DELETE on non-existent job → 404 with `not_found` error |

## CI Impact

No CI changes required. The test file follows the existing naming convention (`api_*.rs` in `backend/tests/`) and will be automatically discovered by `cargo test --workspace --features mock-hardware`. No CI workflow files are modified.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| In-memory DB from `open_in_memory()` may not include migrations for `jobs` and `artifacts` tables | Low | High — tests would fail at SQL level | `open_in_memory()` runs all embedded sqlx migrations (verified in P17-A1 plan). If it fails, fall back to creating a temp-file DB and manually running migration SQL. |
| Test isolation — shared global state between tests | Medium | High — flaky tests in CI | Each test creates its own `App`, DB pool, temp dir, and server instance. Use `#[serial]` on all tests to prevent parallel execution (matching `api_cancel.rs` pattern). Use `temp_env::async_with_vars` for env var isolation. |
| Hyper client connection reuse causing stale connections | Low | Medium | Create a fresh `hyper_util::client::legacy::Client` per request or use `tower::ServiceExt::ready` pattern. The existing `api_cancel.rs` uses the same pattern successfully. |
| Server startup race condition — test sends request before server is ready | Medium | Medium | Add `tokio::time::sleep(Duration::from_millis(100))` after spawning the server (matching `api_cancel.rs` pattern). |
| Fake artifact file path doesn't match actual artifact store sharding | Low | Medium — test passes but doesn't verify real behavior | Use a 64-char hex hash to ensure `hash[..2]` prefix exists. The `delete_for_job` handler queries DB for hashes, not disk, so the file path must match the DB row hash exactly. |

## Acceptance Criteria

- [ ] `backend/tests/api_delete.rs` exists with 5 test functions
- [ ] `cargo test --features mock-hardware --test api_delete` exits 0
- [ ] Test 1: single DELETE on Completed job → 204, DB row gone, artifact file gone, GET → 404
- [ ] Test 2: DELETE on Running job → 409 with `job_active` error
- [ ] Test 3: bulk `DELETE /v1/jobs?status=all` removes all terminal jobs + artifacts, preserves Running
- [ ] Test 4: bulk `DELETE /v1/jobs?status=completed` removes only completed jobs
- [ ] Test 5: DELETE on non-existent job → 404 with `not_found` error
- [ ] All tests use `#[serial]` for deterministic ordering
- [ ] Environment variables are properly scoped with `temp_env::async_with_vars` and cleaned up unconditionally
- [ ] No production source files are modified
