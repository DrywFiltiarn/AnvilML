# Plan Report: P17-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-A2                                      |
| Phase       | 017 — Cancellation                          |
| Description | anvilml-server: POST /v1/jobs/:id/cancel + DELETE endpoints |
| Depends on  | P17-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-21T08:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Expose the cancellation and deletion HTTP endpoints on the AnvilML server. This task adds three new HTTP handlers — `cancel_job`, `delete_job`, and `bulk_clear` — and wires them into the `axum` router. The `cancel_job` handler delegates to `JobScheduler::cancel_job()` (already implemented in P17-A1) and maps results to appropriate HTTP status codes (202, 409, 404). The `delete_job` and `bulk_clear` handlers remove terminal jobs from SQLite and delete their associated artifact files from disk. An integration test verifies the cancel → 202 → Cancelled status flow.

## Scope

### In Scope
- Add `pub async fn cancel_job(State<AppState>, Path<Uuid>)` handler in `crates/anvilml-server/src/handlers/jobs.rs`
- Add `pub async fn delete_job(State<AppState>, Path<Uuid>)` handler in `crates/anvilml-server/src/handlers/jobs.rs`
- Add `pub async fn bulk_clear(State<AppState>, Query<BulkClearQuery>)` handler in `crates/anvilml-server/src/handlers/jobs.rs`
- Add `BulkClearQuery` query struct for the `status` filter parameter
- Mount `POST /v1/jobs/:id/cancel`, `DELETE /v1/jobs/{id}`, and `DELETE /v1/jobs` routes in `build_router()` in `crates/anvilml-server/src/lib.rs`
- Add `pub async fn delete(&self, hash: &str) -> Result<(), AnvilError>` method to `ArtifactStore` in `crates/anvilml-artifacts/src/store.rs`
- Add `pub async fn delete_jobs_by_status(&self, status: &str) -> Result<u32, AnvilError>` method to `JobScheduler` for bulk-clear DB deletion (the scheduler owns the jobs table)
- Add integration tests in `crates/anvilml-server/tests/jobs_tests.rs`
- Bump `anvilml-server` patch version (0.1.26 → 0.1.27)

### Out of Scope
- Python worker `CancelJob` message handling (P17-A1)
- `WorkerEvent::Cancelled` event loop handler (P17-A1)
- WebSocket event broadcasting for cancellation (P17-A1)
- OpenAPI spec regeneration (handled by CI `openapi-drift` gate)
- Config surface changes (no new config fields)

## Existing Codebase Assessment

The codebase already has `JobScheduler::cancel_job()` fully implemented in `crates/anvilml-scheduler/src/scheduler.rs` (P17-A1). This method looks up the job, checks its status, and either cancels it from the queue, sends an IPC cancel message, or returns `AnvilError::InvalidOperation` for terminal states.

The server crate (`anvilml-server`) follows a consistent handler pattern: each handler takes `State<AppState>` plus path/query extractors, delegates to the appropriate subsystem (scheduler, registry, etc.), and maps results to `Result<(StatusCode, Json<T>), AnvilError>`. The `build_router()` function in `lib.rs` mounts all routes using `axum::routing::{get, post, delete}`.

The `ArtifactStore` currently has `save()`, `get()`, and `list()` methods but no `delete()` method. This is the gap that needs filling — the delete method must atomically remove both the file from disk and the metadata row from SQLite.

The `anvilml-core::AnvilError` enum already includes `InvalidOperation` (maps to 409) and `JobNotFound` (maps to 404), so no error type changes are needed. The `JobStatus` enum has `Completed`, `Failed`, `Cancelled` as terminal states.

Test patterns in `crates/anvilml-server/tests/jobs_tests.rs` use `AppState::new()` with an in-memory database, build a router via `build_router()`, and use `router.clone().oneshot(request)` to send requests. This pattern will be reused for the new tests.

## Resolved Dependencies

No new external dependencies are introduced. All types and methods referenced exist in already-declared workspace crates:
- `anvilml_scheduler::JobScheduler::cancel_job()` — exists, verified in `scheduler.rs` line 464
- `anvilml_artifacts::ArtifactStore` — exists; `delete()` method is new (created by this task)
- `axum::routing::delete` — existing import pattern already used implicitly via `axum::Router::route`

## Approach

1. **Add `delete` method to `ArtifactStore`** (`crates/anvilml-artifacts/src/store.rs`):
   - Implement `pub async fn delete(&self, hash: &str) -> Result<(), AnvilError>` that:
     1. Queries the `artifacts` table for the row matching the hash to get the file path.
     2. If no row found, returns `Err(AnvilError::ArtifactNotFound(hash.to_string()))`.
     3. Attempts to delete the file from disk using `tokio::fs::remove_file` (ignoring NotFound on the file — the DB row is the source of truth).
     4. Deletes the row from the `artifacts` table via `DELETE FROM artifacts WHERE hash = ?`.
     5. Returns `Ok(())` on success.
   - Add `///` doc comment per FORGE_AGENT_RULES §12.1.
   - This method is called by the server handlers, not by the scheduler, maintaining the dependency graph (server → artifacts).

2. **Add `delete_jobs_by_status` method to `JobScheduler`** (`crates/anvilml-scheduler/src/scheduler.rs`):
   - Implement `pub async fn delete_jobs_by_status(&self, status: &str) -> Result<u32, AnvilError>` that:
     1. Validates the status string is one of `"completed"`, `"failed"`, `"cancelled"`, or `"all"`.
     2. If `"all"`, deletes all terminal jobs (`WHERE status IN ('completed','failed','cancelled')`). Otherwise, deletes matching single status.
     3. Returns the count of affected rows.
   - Add `///` doc comment per FORGE_AGENT_RULES §12.1.
   - This method is called by the `bulk_clear` handler.

3. **Add `cancel_job` handler** (`crates/anvilml-server/src/handlers/jobs.rs`):
   - Signature: `pub async fn cancel_job(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AnvilError>`
   - Implementation: `state.scheduler.cancel_job(id).await?; Ok(StatusCode::ACCEPTED)`
   - The `?` operator propagates `AnvilError` variants which `IntoResponse` maps to the correct status codes (404, 409).
   - Add `#[tracing::instrument]` with `fields(job_id = %id)`.
   - Add `///` doc comment.

4. **Add `delete_job` handler** (`crates/anvilml-server/src/handlers/jobs.rs`):
   - Signature: `pub async fn delete_job(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode, AnvilError>`
   - Implementation:
     1. `let job = state.scheduler.get_job(id).await?;` — look up the job.
     2. Check `match job.status` — only `Completed`, `Failed`, `Cancelled` are allowed.
     3. If non-terminal: `return Err(AnvilError::InvalidOperation(...))`.
     4. Delete artifacts: `state.artifact_store.delete(hash).await?` for each artifact from `state.artifact_store.list(Some(id)).await?`.
     5. Delete the job row: `DELETE FROM jobs WHERE id = ?`.
     6. Return `StatusCode::NO_CONTENT`.
   - Add `#[tracing::instrument]` with `fields(job_id = %id)`.
   - Add `///` doc comment.

5. **Add `BulkClearQuery` struct and `bulk_clear` handler** (`crates/anvilml-server/src/handlers/jobs.rs`):
   - Query struct:
     ```rust
     #[derive(Debug, Deserialize)]
     pub struct BulkClearQuery {
         pub status: Option<String>,
     }
     ```
   - Handler signature: `pub async fn bulk_clear(State(state): State<AppState>, Query(params): Query<BulkClearQuery>) -> Result<(StatusCode, Json<serde_json::Value>), AnvilError>`
   - Implementation:
     1. Parse `params.status` — if `None`, default to `"all"`.
     2. Validate the status is one of `completed`, `failed`, `cancelled`, `all`.
     3. If invalid: return 400 with `AnvilError::Internal(...)`.
     4. Call `state.scheduler.delete_jobs_by_status(status).await?` to get count.
     5. For each affected job (need to fetch them first to delete artifacts), delete artifacts from disk.
     6. Return `200 OK` with `{ "removed": count }`.
   - Add `///` doc comment.

6. **Mount routes in `build_router()`** (`crates/anvilml-server/src/lib.rs`):
   - Import `delete` from `axum::routing`.
   - Import new handlers from `handlers::jobs`.
   - Add three route lines:
     ```rust
     .route("/v1/jobs/{id}/cancel", post(cancel_job))
     .route("/v1/jobs/{id}", delete(delete_job))
     .route("/v1/jobs", delete(bulk_clear))
     ```
   - Note: `DELETE /v1/jobs/{id}` must come BEFORE `DELETE /v1/jobs` in the route chain so the more specific pattern matches first. Axum routes are matched in order of insertion.

7. **Update `handlers/mod.rs`** to re-export new handlers:
   - Add `pub use jobs::cancel_job;`, `pub use jobs::delete_job;`, `pub use jobs::bulk_clear;`

8. **Add integration tests** (`crates/anvilml-server/tests/jobs_tests.rs`):
   - `test_cancel_queued_job_returns_202`: Submit a job, POST /v1/jobs/{id}/cancel → 202, then GET /v1/jobs/{id} → status "Cancelled".
   - `test_cancel_terminal_job_returns_409`: Submit a job, manually update its status to Completed via DB, then POST /v1/jobs/{id}/cancel → 409.
   - `test_delete_terminal_job_returns_204`: Submit a job, manually update status to Completed, DELETE /v1/jobs/{id} → 204, then GET /v1/jobs/{id} → 404.
   - `test_delete_non_terminal_job_returns_409`: Submit a job (status Queued), DELETE /v1/jobs/{id} → 409.
   - `test_bulk_clear_returns_removed_count`: Submit multiple jobs, update them to terminal states, DELETE /v1/jobs?status=completed → 200 with `{"removed": N}`.

9. **Bump `anvilml-server` version** in `Cargo.toml`: 0.1.26 → 0.1.27.

## Public API Surface

### New `pub` items in `anvilml-server`

**File: `crates/anvilml-server/src/handlers/jobs.rs`**

```rust
/// Cancel a job by its UUID.
///
/// Delegates to the `JobScheduler::cancel_job()` method which handles
/// cancellation differently based on the job's current status:
/// - Queued: immediately removes from queue and marks Cancelled.
/// - Running: sends a CancelJob IPC message to the owning worker.
/// - Terminal: returns 409 Conflict.
///
/// # Arguments
///
/// * `state` — Shared application state containing the job scheduler.
/// * `id` — The UUID of the job to cancel.
///
/// # Returns
///
/// * `202 Accepted` — cancellation accepted (queued or running job).
/// * `404 Not Found` — no job with the given ID exists.
/// * `409 Conflict` — job is in a terminal state.
pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AnvilError>;

/// Delete a terminal job and its artifacts.
///
/// Only allows deletion of jobs in terminal states (Completed, Failed,
/// Cancelled). Deletes all associated artifact files from disk and the
/// job record from the database.
///
/// # Arguments
///
/// * `state` — Shared application state containing the job scheduler
///   and artifact store.
/// * `id` — The UUID of the job to delete.
///
/// # Returns
///
/// * `204 No Content` — job and artifacts deleted successfully.
/// * `404 Not Found` — no job with the given ID exists.
/// * `409 Conflict` — job is not in a terminal state.
pub async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AnvilError>;

/// Query parameters for the bulk_clear endpoint.
#[derive(Debug, Deserialize)]
pub struct BulkClearQuery {
    /// Filter by job status. Valid values: `"completed"`, `"failed"`,
    /// `"cancelled"`, or `"all"`. Defaults to `"all"` if omitted.
    pub status: Option<String>,
}

/// Bulk clear terminal jobs and their artifacts.
///
/// Deletes all jobs matching the given status filter (must be a terminal
/// status: completed, failed, cancelled, or all) along with their
/// artifact files from disk.
///
/// # Arguments
///
/// * `state` — Shared application state.
/// * `params` — Optional status filter.
///
/// # Returns
///
/// * `200 OK` with `{ "removed": u32 }` body.
/// * `400 Bad Request` — invalid status value.
pub async fn bulk_clear(
    State(state): State<AppState>,
    Query(params): Query<BulkClearQuery>,
) -> Result<(StatusCode, Json<serde_json::Value>), AnvilError>;
```

### New `pub` items in `anvilml-scheduler`

**File: `crates/anvilml-scheduler/src/scheduler.rs`**

```rust
/// Delete jobs matching a status filter and return the count deleted.
///
/// Only deletes jobs in terminal states (Completed, Failed, Cancelled)
/// or all jobs when `status` is `"all"`. Non-terminal status values
/// are rejected with an error.
///
/// # Arguments
///
/// * `status` — The status filter: `"completed"`, `"failed"`,
///   `"cancelled"`, or `"all"`.
///
/// # Returns
///
/// `Ok(count)` with the number of affected rows.
/// `Err(AnvilError::Internal(_))` if the status value is invalid.
pub async fn delete_jobs_by_status(&self, status: &str) -> Result<u32, AnvilError>;
```

### New `pub` items in `anvilml-artifacts`

**File: `crates/anvilml-artifacts/src/store.rs`**

```rust
/// Delete an artifact by its content hash.
///
/// Removes the artifact file from disk and deletes its metadata row
/// from the database. If the file does not exist on disk but the DB
/// row does, the DB row is still deleted (orphan cleanup).
///
/// # Arguments
///
/// * `hash` — The SHA-256 hex digest of the artifact to delete.
///
/// # Returns
///
/// `Ok(())` on success. `Err(AnvilError::ArtifactNotFound(_))` if no
/// artifact with the given hash exists in the database.
pub async fn delete(&self, hash: &str) -> Result<(), AnvilError>;
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.26 → 0.1.27 |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `cancel_job`, `delete_job`, `bulk_clear` handlers + `BulkClearQuery` struct |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Re-export new handlers |
| Modify | `crates/anvilml-server/src/lib.rs` | Mount 3 new routes in `build_router()` |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `delete_jobs_by_status` method |
| Modify | `crates/anvilml-artifacts/src/store.rs` | Add `delete` method |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Add 5 integration tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_cancel_queued_job_returns_202` | Cancel a queued job → 202, then GET returns Cancelled status | Registry with LoadModel, job submitted via POST | POST `/v1/jobs/{id}/cancel` on queued job | 202 on cancel; GET shows `"Cancelled"` | `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_cancel_queued_job_returns_202` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_cancel_terminal_job_returns_409` | Cancel a completed job → 409 | Registry with LoadModel, job submitted and manually set to Completed | POST `/v1/jobs/{id}/cancel` on terminal job | 409 with `"error": "invalid_operation"` | `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_cancel_terminal_job_returns_409` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_delete_terminal_job_returns_204` | Delete a completed job → 204, then GET → 404 | Registry with LoadModel, job submitted and manually set to Completed | DELETE `/v1/jobs/{id}` on terminal job | 204; subsequent GET returns 404 | `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_delete_terminal_job_returns_204` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_delete_non_terminal_job_returns_409` | Delete a queued job → 409 | Registry with LoadModel, job submitted (Queued) | DELETE `/v1/jobs/{id}` on queued job | 409 with `"error": "invalid_operation"` | `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_delete_non_terminal_job_returns_409` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_bulk_clear_returns_removed_count` | Bulk clear completed jobs → 200 with count | Registry with LoadModel, 2+ jobs submitted and set to Completed | DELETE `/v1/jobs?status=completed` | 200 with `{"removed": N}` where N ≥ 1 | `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_bulk_clear_returns_removed_count` exits 0 |

## CI Impact

No CI job changes required. The new handlers are added to existing modules that are already compiled and tested by the standard workspace test suite (`cargo test --workspace --features mock-hardware`). The OpenAPI drift gate (`openapi-drift`) will automatically detect the new routes when `cargo run -p anvilml-openapi` is run and compare against `api/openapi.json`. The config-drift gate is unaffected (no config changes).

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. All operations use cross-platform Rust std/async-fs APIs (`tokio::fs::remove_file` works on both Unix and Windows). The `axum` routing and `serde` serialization are platform-neutral.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum::Router::route` order sensitivity: if `DELETE /v1/jobs` is mounted before `DELETE /v1/jobs/{id}`, the more specific pattern may never match. | Low | High | Mount `{id}` routes before the collection route. Axum matches routes in insertion order — this is a well-documented pattern. |
| `ArtifactStore::delete` race condition: file deleted by another handler between list and delete calls. | Low | Medium | The delete method is the sole deletion path; no concurrent deletions exist in the current codebase. If the file is already gone, `tokio::fs::remove_file` returns an error which we map to a warning and continue — the DB row is the source of truth. |
| `bulk_clear` needs to delete artifacts for each job but `delete_jobs_by_status` only returns a count. We need to fetch jobs first to get their IDs for artifact deletion. | Medium | Medium | Fetch matching jobs via `list_jobs(status)` before calling `delete_jobs_by_status`. Delete artifacts per-job, then delete DB rows. This two-step approach ensures artifacts are cleaned up. |
| The `JobScheduler` doesn't expose a method to list jobs by status for the `bulk_clear` handler to find artifact IDs. | Low | Medium | The existing `list_jobs(status, None, None)` method already supports filtering by `JobStatus`. The handler can call this, extract artifact IDs, then call `delete_jobs_by_status`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_cancel_queued_job_returns_202` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_cancel_terminal_job_returns_409` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_delete_terminal_job_returns_204` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_delete_non_terminal_job_returns_409` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware --test jobs_tests test_bulk_clear_returns_removed_count` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0 (full crate test suite)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (full workspace test suite)
