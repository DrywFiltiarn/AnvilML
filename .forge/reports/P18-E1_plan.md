# Plan Report: P18-E1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-E1                                            |
| Phase       | 18 — HTTP/WebSocket Server Completion             |
| Description | anvilml-server: DELETE /v1/jobs/:id single-job delete handler |
| Depends on  | P18-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-12T14:45:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the `DELETE /v1/jobs/:id` handler that deletes a single terminal-status job (Completed/Failed/Cancelled) along with its associated artifacts, returning 204 on success, 409 on non-terminal jobs, and 404 on unknown IDs. This completes the per-job delete path from `ANVILML_DESIGN.md §13.4`; bulk clear is deferred to P18-E2. Also adds the required `JobStore::delete()` and `ArtifactStore::delete()` persistence methods, updates `build_router()` to register the DELETE route, and adds >=4 integration tests to `jobs_tests.rs`.

## Scope

### In Scope
- Add `pub async fn delete(&self, id: Uuid) -> Result<(), AnvilError>` to `JobStore` in `crates/anvilml-registry/src/job_store.rs` — issues `DELETE FROM jobs WHERE id = ?`, no error if row absent.
- Add `pub async fn delete(&self, hash: &str) -> Result<(), AnvilError>` to `ArtifactStore` in `crates/anvilml-artifacts/src/store.rs` — deletes the artifact file from disk and removes the DB row.
- Add `pub(crate) async fn delete_job(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AnvilError>` to `crates/anvilml-server/src/handlers/jobs.rs`:
  - Look up job via `JobStore::new(state.db.clone()).get(id)`. Return 404 via `AnvilError::JobNotFound` if absent.
  - Check `job.status` — if `Queued` or `Running`, return 409 `StatusCode::CONFLICT` (same pattern as Phase 17's cancel handler).
  - List artifacts by `job_id` via `state.artifact_store.list(Some(id))`, then delete each artifact file + row via `state.artifact_store.delete(hash)`.
  - Delete the job row via `JobStore::new(state.db.clone()).delete(id)`.
  - Log deletion at INFO level, return `StatusCode::NO_CONTENT` (204).
- Update `build_router()` in `crates/anvilml-server/src/lib.rs` to add `.delete(delete_job)` on the existing `/v1/jobs/{id}` route.
- Add >=4 new tests to `crates/anvilml-server/tests/jobs_tests.rs`.
- Bump patch versions: `anvilml-server` 0.1.24→0.1.25, `anvilml-registry` 0.1.8→0.1.9, `anvilml-artifacts` 0.1.3→0.1.4.

### Out of Scope
- Bulk clear (`DELETE /v1/jobs` with `?status=`) — deferred to P18-E2, which is named in this task's `defers_to` field and whose `description`/`context` genuinely states bulk delete as its own deliverable.
- Any changes to `anvilml-scheduler` — the handler accesses `JobStore` directly via `state.db`, not through the scheduler.
- Any changes to `anvilml-core` types or error variants.

## Existing Codebase Assessment

The `anvilml-server` crate already has a mature handler pattern in `jobs.rs`: `submit_job`, `list_jobs`, `get_job`, and `cancel_job` all follow the same structure — accept `State<AppState>` + optional extractors, delegate to a store/scheduler method, map results to `StatusCode` or `Json<T>`, and use `#[tracing::instrument]` for logging. The `cancel_job` handler (Phase 17) establishes the exact conflict-on-non-terminal pattern this task reuses: it returns 409 for terminal-status jobs.

The `JobStore` in `anvilml-registry` currently has `upsert`, `get`, `list`, and `reset_ghost_jobs` but no `delete` method — this is the gap this task fills. The `ModelStore` already has a `delete(&self, id: &str)` method (line 214 of `store.rs`) that serves as the exact template for the `JobStore::delete` implementation.

The `ArtifactStore` in `anvilml-artifacts` has `save`, `get`, and `list` (with `job_id` filter) but no `delete` method. The `list(job_id)` method returns `Vec<ArtifactMeta>` which includes each artifact's `hash` — this is the bridge between identifying artifacts to delete and calling the new `delete` method.

The test file `jobs_tests.rs` has 15 tests covering submit, list, get, and cancel. The task requires >=18 total, so >=4 new tests are needed. The test patterns (in-process HTTP via `router.oneshot()`, `make_test_state` helper, `JobStore` direct access for setup) are well-established and will be reused.

No dual-mode parity markers apply — this is a Rust HTTP handler, not a Python node `execute()` or arch module `load()`/`sample()`/`decode()`.

## Resolved Dependencies

| Type   | Name  | Version verified | MCP source | Feature flags confirmed |
|--------|-------|-----------------|------------|------------------------|
| crate  | sqlx  | (from Cargo.lock — no version change) | lockfile | sqlite |
| crate  | uuid  | (from Cargo.lock — no version change) | lockfile | — |

No new external dependencies are introduced. All types and methods referenced exist in the current codebase.

## Approach

1. **Add `JobStore::delete`** (`crates/anvilml-registry/src/job_store.rs`):
   - Implement `pub async fn delete(&self, id: Uuid) -> Result<(), AnvilError>` following the `ModelStore::delete` template (line 214 of `store.rs`).
   - The method issues `DELETE FROM jobs WHERE id = ?` bound to `id.to_string()`.
   - No error if the row does not exist — SQL DELETE is a no-op for missing rows (same pattern as `ModelStore::delete`).
   - Add `#[tracing::instrument(fields(id = %id), skip(self))]` for structured logging.
   - Add a `///` doc comment describing the method, arguments, and error behavior.

2. **Add `ArtifactStore::delete`** (`crates/anvilml-artifacts/src/store.rs`):
   - Implement `pub async fn delete(&self, hash: &str) -> Result<(), AnvilError>`.
   - Delete the file from disk: construct `{artifact_dir}/{hash}.png`, call `std::fs::remove_file`. If the file does not exist, treat as success (idempotent delete).
   - Delete the DB row: issue `DELETE FROM artifacts WHERE hash = ?` bound to `hash`.
   - Wrap both operations in a single transaction-like sequence (no explicit transaction needed for SQLite — each is its own statement).
   - Add `#[tracing::instrument(fields(hash = %hash), skip(self))]` and a `///` doc comment.

3. **Add `delete_job` handler** (`crates/anvilml-server/src/handlers/jobs.rs`):
   - Signature: `pub(crate) async fn delete_job(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AnvilError>`
   - Step 1: Create `JobStore` from `state.db.clone()`, call `.get(id)`. If `Ok(None)`, return `AnvilError::JobNotFound(id.to_string())` (→ 404).
   - Step 2: Check `job.status`. If `Queued` or `Running`, return `StatusCode::CONFLICT` (409) with a `tracing::debug!` log. This mirrors the cancel handler's pattern.
   - Step 3: List artifacts with `state.artifact_store.list(Some(id))`. For each artifact, call `state.artifact_store.delete(&artifact.hash)`. Log count at DEBUG.
   - Step 4: Delete the job row with `job_store.delete(id)`. Log at INFO: `tracing::info!(job_id = %id, "deleted job and artifacts");`
   - Step 5: Return `StatusCode::NO_CONTENT` (204).
   - Add `#[tracing::instrument(skip(state), fields(job_id = %id))]` for span instrumentation.
   - Add a full `///` doc comment covering the method, response codes, and state access.

4. **Update `build_router()`** (`crates/anvilml-server/src/lib.rs`):
   - On the existing `/v1/jobs/{id}` route (line 61-64), add `.delete(handlers::jobs::delete_job)` to the routing builder.
   - The route currently has `.get(...).post(...)` for get_job and cancel_job. Add `.delete(...)` for delete_job.

5. **Add integration tests** (`crates/anvilml-server/tests/jobs_tests.rs`):
   - `test_delete_terminal_job_returns_204`: Submit a job, manually set its status to Completed via `JobStore::upsert`, call DELETE, assert 204. Verify the job is no longer in the DB via `JobStore::get`.
   - `test_delete_terminal_job_removes_artifacts`: Submit a job, persist an artifact row for that job_id (via direct DB insert or by calling `artifact_store.save()` with a fake PNG), set job to Completed, call DELETE, assert 204. Verify the artifact is gone via `artifact_store.list(Some(id))`.
   - `test_delete_non_terminal_queued_returns_409`: Submit a job (it enters Queued state), call DELETE, assert 409.
   - `test_delete_non_terminal_running_returns_409`: Submit a job, manually set status to Running via `JobStore::upsert`, call DELETE, assert 409.
   - `test_delete_unknown_id_returns_404`: Call DELETE with a random UUID, assert 404.
   - (5 tests total — exceeds the >=4 requirement, bringing total to 20.)

6. **Bump crate versions** per `ENVIRONMENT.md §12`:
   - `crates/anvilml-server/Cargo.toml`: 0.1.24 → 0.1.25
   - `crates/anvilml-registry/Cargo.toml`: 0.1.8 → 0.1.9
   - `crates/anvilml-artifacts/Cargo.toml`: 0.1.3 → 0.1.4

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| New pub fn | `crates/anvilml-registry/src/job_store.rs` | `pub async fn delete(&self, id: Uuid) -> Result<(), AnvilError>` |
| New pub fn | `crates/anvilml-artifacts/src/store.rs` | `pub async fn delete(&self, hash: &str) -> Result<(), AnvilError>` |
| New pub(crate) fn | `crates/anvilml-server/src/handlers/jobs.rs` | `pub(crate) async fn delete_job(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AnvilError>` |

No new `pub struct`, `pub enum`, or `pub trait` items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-registry/src/job_store.rs` | Add `delete(&self, id: Uuid)` method |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |
| MODIFY | `crates/anvilml-artifacts/src/store.rs` | Add `delete(&self, hash: &str)` method |
| MODIFY | `crates/anvilml-artifacts/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| MODIFY | `crates/anvilml-server/src/handlers/jobs.rs` | Add `delete_job` handler function |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Add `.delete(delete_job)` to `/v1/jobs/{id}` route in `build_router()` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.24 → 0.1.25 |
| MODIFY | `crates/anvilml-server/tests/jobs_tests.rs` | Add >=4 new integration tests for delete_job |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_delete_terminal_job_returns_204` | DELETE on a Completed job returns 204 and removes the job row from DB | `cargo test -p anvilml-server --test jobs_tests -- test_delete_terminal_job_returns_204` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_delete_terminal_job_removes_artifacts` | DELETE on a Completed job with associated artifacts also deletes artifact file and DB row | `cargo test -p anvilml-server --test jobs_tests -- test_delete_terminal_job_removes_artifacts` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_delete_non_terminal_queued_returns_409` | DELETE on a Queued job returns 409 Conflict | `cargo test -p anvilml-server --test jobs_tests -- test_delete_non_terminal_queued_returns_409` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_delete_non_terminal_running_returns_409` | DELETE on a Running job returns 409 Conflict | `cargo test -p anvilml-server --test jobs_tests -- test_delete_non_terminal_running_returns_409` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_delete_unknown_id_returns_404` | DELETE on an unknown UUID returns 404 Not Found | `cargo test -p anvilml-server --test jobs_tests -- test_delete_unknown_id_returns_404` exits 0 |

Acceptance command for full suite: `cargo test -p anvilml-server --test jobs_tests` exits 0 (>=20 total tests).

## CI Impact

No CI changes required. The new handler is exercised by the existing `cargo test --workspace --features mock-hardware` CI job (rust-linux and rust-windows). No new file types, no new gates. The OpenAPI drift gate (P18-F1/F2) will pick up the new route annotation when those tasks are implemented.

## Platform Considerations

None identified. The `delete_job` handler is platform-neutral: it uses `sqlx` for SQLite operations (already cross-platform in this codebase), `std::fs::remove_file` for artifact deletion (standard Rust, identical behavior on Linux/Windows), and `StatusCode::NO_CONTENT` which is an HTTP-level constant. No `#[cfg(unix)]` or `#[cfg(windows)]` guards required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobStore::delete` must be added to a different file than `ModelStore::delete` — the templates are in different crates. A copy-paste error could use the wrong table name (`models` instead of `jobs`). | Low | Medium | Follow the exact `ModelStore::delete` SQL pattern but with the `jobs` table. The column is `id` (TEXT, UUID string) in both tables, so the query structure is identical. |
| `ArtifactStore::delete` must handle the case where the file exists but the DB row does not (or vice versa). Deleting a non-existent file would return `Err(NotFound)` from `std::fs::remove_file`. | Medium | Low | Check `file_path.exists()` before calling `remove_file` — skip the file delete if absent. The DB delete is a no-op for missing rows (same as `ModelStore::delete`). |
| The handler creates two separate `JobStore` instances (`new(state.db.clone())`) — one for `get` and one for `delete`. If the DB is under heavy load, there's a tiny window between get and delete where another request could modify the job. | Low | Low | This is acceptable for an admin/delete operation. The status check + delete is not atomic, but the 409 guard on the status is best-effort. For production, a single SQL transaction would be needed, but this is out of scope. |
| `cargo test -p anvilml-server --test jobs_tests` currently has 15 tests. Adding 5 new tests brings the total to 20, exceeding the >=18 requirement. However, if any test has a setup defect (e.g., the artifact row is not properly associated with the job), the test will fail and block staging. | Medium | Medium | Use the same test patterns as existing tests (direct `JobStore` access for setup, `artifact_store.list()` for verification). Run the full suite after implementation to confirm. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test jobs_tests` exits 0 (>=20 total tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `DELETE /v1/jobs/:id` on a Completed job returns 204
- [ ] `DELETE /v1/jobs/:id` on a Queued job returns 409
- [ ] `DELETE /v1/jobs/:id` on an unknown UUID returns 404
