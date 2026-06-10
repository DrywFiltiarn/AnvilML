# Plan Report: P16-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-A3                                      |
| Phase       | 016 — Job Cancellation                        |
| Description | anvilml-server: POST /v1/jobs/:id/cancel      |
| Depends on  | P16-A1, P16-A2                                |
| Project     | anvilml                                       |
| Planned at  | 2026-06-10T12:32:00Z                          |
| Attempt     | 1                                             |

## Objective

Add the `cancel_job` handler to `anvilml-server` and wire `POST /v1/jobs/:id/cancel` into the
axum router. The handler calls `JobScheduler::cancel`, maps `JobNotFound` to HTTP 404,
`JobNotCancellable` to HTTP 409, and success to HTTP 202.

## Scope

### In Scope
- Add `cancel_job(State<Arc<App>>, Path<Uuid>) -> (StatusCode, Json<Value>)` in
  `crates/anvilml-server/src/handlers/jobs.rs`.
- Wire the route in `crates/anvilml-server/src/lib.rs` (`build_router`).
- Add unit tests in `jobs.rs` (same pattern as existing `submit_job`/`get_job` tests):
  - Cancel a non-existent job → 404.
  - Cancel a queued job → 202 (scheduler cancels + broadcasts).
  - Cancel a completed job → 409.
- Update `Cargo.toml` patch version for `anvilml-server` (0.1.8 → 0.1.9).
- Run `cargo test -p anvilml-server` — must pass.
- Run `cargo fmt --all` and `cargo fmt --all -- --check`.

### Out of Scope
- Integration test (P16-A4: `backend/tests/api_cancel.rs`) — handled by a separate task.
- Changes to `anvilml-scheduler` — already implemented in P16-A2.
- Changes to worker cooperative cancel — already implemented in P16-A1.
- OpenAPI regeneration — only needed if handler signatures change; this task adds a new
  handler with `utoipa::path` annotations, so the OpenAPI diff gate will need running
  after implementation.

## Approach

1. **Add `cancel_job` handler** in `handlers/jobs.rs`.
   - Signature: `pub async fn cancel_job(State(state): State<Arc<App>>, Path(job_id): Path<Uuid>) -> (StatusCode, Json<serde_json::Value>)`
   - Extract `scheduler` from `state.scheduler` (match `None` → 503, same pattern as `submit_job`).
   - Call `scheduler.cancel(job_id).await`.
   - Map errors:
     - `AnvilError::JobNotFound(_)` → 404 `not_found`.
     - `AnvilError::JobNotCancellable(_)` → 409 `job_not_cancellable`.
     - `AnvilError::DbError(_)` → 500 `internal_error`.
     - Catch-all `Err(e)` → 500 `internal_error`.
   - On `Ok(())` → 202 with body `{"status": "cancelled", "job_id": ...}`.
   - Add `#[utoipa::path(...)]` annotation for OpenAPI generation:
     - `post, path = "/v1/jobs/{id}", summary = "Cancel a queued or running job"`
     - `params: ("id" = Uuid, Path, description = "Job UUID")`
     - `responses: (202, 404, 409, 500)`

2. **Wire the route** in `lib.rs` `build_router`.
   - Add `.route("/v1/jobs/{id}/cancel", post(handlers::jobs::cancel_job))` to the existing
     router chain, alongside the existing `/v1/jobs/{id}` route.

3. **Add unit tests** in `handlers/jobs.rs` `mod tests`.
   - **`cancel_job_returns_404_when_missing`**: Build test app with scheduler, POST cancel
     for a random UUID → 404, body `{"error": "not_found"}`.
   - **`cancel_job_returns_202_for_queued_job`**: Submit a valid job (starts Queued),
     then POST cancel → 202, body `{"status": "cancelled"}`.
   - **`cancel_job_returns_409_for_completed_job`**: Submit a job, simulate it reaching
     Completed status via DB update (or use the existing test pattern of submitting then
     directly updating the DB to Completed), then POST cancel → 409,
     body `{"error": "job_not_cancellable"}`.

4. **Version bump** `anvilml-server` Cargo.toml: `0.1.8` → `0.1.9`.

5. **Verify**:
   - `cargo test -p anvilml-server --features mock-hardware` — all pass.
   - `cargo fmt --all -- --check` — zero drift.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `cancel_job` handler + unit tests |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `POST /v1/jobs/{id}/cancel` route |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.8 → 0.1.9` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/src/handlers/jobs.rs` | `cancel_job_returns_404_when_missing` | Cancelling a nonexistent job returns 404 with `not_found` |
| `crates/anvilml-server/src/handlers/jobs.rs` | `cancel_job_returns_202_for_queued_job` | Cancelling a Queued job returns 202; scheduler transitions to Cancelled |
| `crates/anvilml-server/src/handlers/jobs.rs` | `cancel_job_returns_409_for_completed_job` | Cancelling a terminal (Completed) job returns 409 with `job_not_cancellable` |

## CI Impact

The new route and handler are added to `anvilml-server`. After implementation, the
OpenAPI drift gate (`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json`)
must pass — the new `cancel_job` endpoint with its `utoipa::path` annotation will appear
in the generated `openapi.json`. If the diff is non-empty, regenerate and stage the new
`openapi.json`. No CI workflow files are modified.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobScheduler::cancel` already returns `AnvilError` variants that map cleanly to HTTP codes | Low | Low | The error enum already has `JobNotFound` and `JobNotCancellable` — no changes needed. |
| Route conflict: `/v1/jobs/{id}/cancel` vs `/v1/jobs/{id}` | Low | Medium | axum handles nested routes correctly when `/cancel` is a separate route segment; the `{id}` parameter is captured differently. Verify with a compile check. |
| OpenAPI drift after adding handler | Medium | Low | Regenerate `openapi.json` with `anvilml-openapi` and stage if diff is non-empty. |
| Test isolation: scheduler state shared between tests | Medium | Medium | Use `#[serial]` on cancel tests that modify scheduler state, or build a fresh test app per test (as existing tests already do with `build_test_app()`). |

## Acceptance Criteria

- [ ] `cancel_job` handler exists in `crates/anvilml-server/src/handlers/jobs.rs` with `utoipa::path` annotation
- [ ] `POST /v1/jobs/{id}/cancel` route is wired in `build_router`
- [ ] Cancel nonexistent job → 404 with `not_found`
- [ ] Cancel queued job → 202 with `cancelled` status
- [ ] Cancel completed job → 409 with `job_not_cancellable`
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `anvilml-server` Cargo.toml version bumped to `0.1.9`
- [ ] No public API signatures changed in other crates
