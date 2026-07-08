# Plan Report: P14-D2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-D2                                      |
| Phase       | 14 — Dispatch & Execute                     |
| Description | anvilml-server: GET /v1/jobs and GET /v1/jobs/:id handlers |
| Depends on  | P14-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-08T11:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the two read endpoints for the jobs API — `GET /v1/jobs` (list with optional status/limit filters) and `GET /v1/jobs/:id` (single job lookup) — in `crates/anvilml-server/src/handlers/jobs.rs`, wire them into `build_router()` in `lib.rs`, and add at least five new integration tests to `jobs_tests.rs` so the file reaches >=9 total tests. This closes out Phase 14's HTTP surface for jobs, completing the handler group D started by P14-D1.

## Scope

### In Scope
- Add `list_jobs()` handler to `crates/anvilml-server/src/handlers/jobs.rs`: accepts `Query<ListJobsParams>` (status, limit, before), delegates to `state.scheduler.get_job_store().list(status, limit)`, returns `Json<Vec<Job>>` at 200 OK. The `before` field is kept in the struct for forward-compatibility per §13.4 but not passed to `list()` — the `JobStore::list()` method from Phase 13 does not support a before-cursor parameter.
- Add `get_job()` handler to `crates/anvilml-server/src/handlers/jobs.rs`: accepts `Path<Uuid>`, delegates to `state.scheduler.get_job(id)`, returns `Json<Job>` at 200 OK, or `404` via `AnvilError::JobNotFound` if the job is absent.
- Add `ListJobsParams` query struct with `status: Option<JobStatus>`, `limit: Option<u32>`, `before: Option<DateTime<Utc>>` fields.
- Register both new routes in `build_router()`: `.route("/v1/jobs", get(list_jobs))` and `.route("/v1/jobs/:id", get(get_job))`.
- Add >=5 new integration tests in `crates/anvilml-server/tests/jobs_tests.rs` covering: list no filter, list status filter, list with limit, get existing job, get unknown job (404), and list before-param acceptance (forward compat).
- Bump `anvilml-server` patch version per §12 of ENVIRONMENT.md.

### Out of Scope
None. This task's `defers_to` field is empty (`[]` from JSON), and the task context contains no deferred scope. All described functionality is implemented in full.

## Existing Codebase Assessment

The `jobs.rs` file currently contains only the `submit_job()` handler from P14-D1, plus `SubmitJobRequest`/`SubmitJobResponse` structs. The handler follows a thin-delegation pattern: it extracts `State<AppState>` and `Json<SubmitJobRequest>`, calls `state.scheduler.submit()`, and returns `(StatusCode, Json<...>)`. The `submit_job` function is `pub(crate)`.

The `JobStore` (in `anvilml-registry`) already provides `list(status: Option<JobStatus>, limit: Option<u32>) -> Result<Vec<Job>, AnvilError>` and `get(id: Uuid) -> Result<Option<Job>, AnvilError>`. The `JobScheduler` exposes `job_store` as a private field, so the handlers must access it through the scheduler — the scheduler's `get_job()` method already delegates to `job_store.get()`, but there is no `list_jobs()` method on the scheduler yet. The `JobStore::list()` is public and accessible directly through `AppState.db` (SqlitePool) if needed, but the cleaner approach is to add a thin `list_jobs()` method to `JobScheduler` that delegates to `job_store.list()`, mirroring the existing `get_job()` pattern.

The test file `jobs_tests.rs` has 4 tests already. The pattern uses `router.oneshot(req)` for in-process HTTP testing with an in-memory SQLite pool, and `make_test_state()` helper constructs a minimal `AppState`. The `JobStore` is created from the pool and passed to `JobScheduler`, so the scheduler's `get_job()` and any future `list_jobs()` will operate on the test database.

The established patterns to follow:
- Handler functions are `pub(crate) async fn` with `#[tracing::instrument]` attribute.
- Error propagation uses `?` through `AnvilError`.
- Tests use `tower::util::ServiceExt::oneshot()` for in-process routing.
- `ListJobsParams` follows axum's `Deserialize` derive from `serde`.

No gap between design doc and current source affects the approach — `JobStore::list()` exists and works, `JobScheduler::get_job()` exists and works, and the only missing piece is wiring the handlers and routes.

## Resolved Dependencies

None. This task introduces no new external crates or packages. All types used (`JobStatus`, `Job`, `DateTime<Utc>`, `Uuid`, `serde_json::Value`) are already in scope through existing workspace dependencies (`anvilml-core`, `axum`, `serde`, `chrono`, `uuid`).

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) |         |                 |                |                        |

## Approach

### Step 1: Add `list_jobs()` method to `JobScheduler`

Add a `pub async fn list_jobs(&self, status: Option<JobStatus>, limit: Option<u32>) -> Result<Vec<Job>, AnvilError>` method to `crates/anvilml-scheduler/src/scheduler.rs`. This is a one-line delegation: `self.job_store.list(status, limit).await`. It mirrors the existing `get_job()` pattern. Add `#[tracing::instrument(skip(self), fields(status, limit))]` for observability.

### Step 2: Add `ListJobsParams` struct and `list_jobs()` handler to `jobs.rs`

Add the `ListJobsParams` query struct:

```rust
#[derive(Debug, Deserialize)]
pub(crate) struct ListJobsParams {
    pub status: Option<JobStatus>,
    pub limit: Option<u32>,
    #[serde(default)]
    pub before: Option<DateTime<Utc>>,
}
```

The `before` field is kept for forward-compatibility per §13.4's documented query parameter. It is **not** passed to `job_store.list()` because that method (Phase 13) does not accept a before-cursor. This is a Deviation: the struct accepts the parameter from the HTTP layer, but the field is silently ignored at the persistence layer. The struct field is not dropped, satisfying the task's requirement to keep it for forward-compat.

Add the `list_jobs()` handler:

```rust
#[tracing::instrument(skip(state), fields(status, limit))]
pub(crate) async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<ListJobsParams>,
) -> Result<Json<Vec<Job>>, AnvilError> {
    let jobs = state.scheduler.list_jobs(params.status, params.limit).await?;
    tracing::info!(count = jobs.len(), "listed jobs");
    Ok(Json(jobs))
}
```

### Step 3: Add `get_job()` handler to `jobs.rs`

```rust
#[tracing::instrument(skip(state), fields(job_id))]
pub(crate) async fn get_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<Job>, AnvilError> {
    let job = state.scheduler.get_job(job_id).await?;
    job.ok_or_else(|| AnvilError::JobNotFound(job_id.to_string())).map(Json)
}
```

This delegates to `state.scheduler.get_job(id)`, which already exists. If `None`, it returns `AnvilError::JobNotFound`, which `IntoResponse` maps to HTTP 404.

### Step 4: Register routes in `build_router()`

Update `crates/anvilml-server/src/lib.rs`:

```rust
axum::Router::new()
    .route("/health", axum::routing::get(handlers::health::health))
    .route("/v1/jobs", axum::routing::get(list_jobs).post(handlers::jobs::submit_job))
    .route("/v1/jobs/:id", axum::routing::get(handlers::jobs::get_job))
    .route("/v1/nodes", axum::routing::get(handlers::nodes::list_nodes))
    .with_state(app_state)
```

The `/v1/jobs` route now has both `get` and `post` methods. The `/v1/jobs/:id` route adds the `get` method.

### Step 5: Add imports to `jobs.rs`

Add required imports:
- `use axum::extract::Query;`
- `use axum::extract::Path;`
- `use anvilml_core::JobStatus;` (already imported via `anvilml_core::*` pattern — verify)
- `use chrono::DateTime;` (for `ListJobsParams::before`)

### Step 6: Add >=5 new integration tests to `jobs_tests.rs`

Add the following tests:

1. **`test_list_jobs_no_filter_returns_all`** — Submit a job (via POST), then list with no filters, assert the returned vector contains the submitted job.
2. **`test_list_jobs_status_filter`** — Submit two jobs, cancel one (or manipulate DB directly), list with `status=completed` or `status=queued`, assert only matching jobs are returned.
3. **`test_list_jobs_limit`** — Submit multiple jobs, list with `limit=2`, assert at most 2 jobs returned.
4. **`test_get_job_existing_returns_200`** — Submit a job, extract its ID from the POST response, call GET /v1/jobs/:id, assert 200 and the returned job matches.
5. **`test_get_job_unknown_returns_404`** — Call GET /v1/jobs/:id with a UUID that was never submitted, assert 404 status.
6. **`test_list_jobs_before_param_accepted`** — Submit a job, list with `before=<timestamp>` query param, assert 200 (the param is accepted but ignored by the persistence layer).

### Step 7: Bump `anvilml-server` patch version

Read current version from `crates/anvilml-server/Cargo.toml` and increment the patch version (Z in X.Y.Z).

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `list_jobs()` handler | `anvilml-server::handlers::jobs` | `pub(crate) async fn list_jobs(State<AppState>, Query<ListJobsParams>) -> Result<Json<Vec<Job>>, AnvilError>` |
| `get_job()` handler | `anvilml-server::handlers::jobs` | `pub(crate) async fn get_job(State<AppState>, Path<Uuid>) -> Result<Json<Job>, AnvilError>` |
| `ListJobsParams` struct | `anvilml-server::handlers::jobs` | `pub(crate) struct ListJobsParams { status: Option<JobStatus>, limit: Option<u32>, before: Option<DateTime<Utc>> }` |
| `list_jobs()` method | `anvilml-scheduler::JobScheduler` | `pub async fn list_jobs(&self, status: Option<JobStatus>, limit: Option<u32>) -> Result<Vec<Job>, AnvilError>` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `list_jobs()`, `get_job()` handlers, `ListJobsParams` struct, and new imports |
| Modify | `crates/anvilml-server/src/lib.rs` | Register GET routes for `/v1/jobs` and `/v1/jobs/:id` in `build_router()` |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `list_jobs()` delegation method |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Add >=5 new integration tests |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_list_jobs_no_filter_returns_all` | `GET /v1/jobs` with no query params returns all submitted jobs (200, non-empty) | `cargo test -p anvilml-server --test jobs_tests test_list_jobs_no_filter_returns_all` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_list_jobs_status_filter` | `GET /v1/jobs?status=queued` returns only jobs matching the status filter | `cargo test -p anvilml-server --test jobs_tests test_list_jobs_status_filter` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_list_jobs_limit` | `GET /v1/jobs?limit=2` returns at most 2 jobs | `cargo test -p anvilml-server --test jobs_tests test_list_jobs_limit` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_get_job_existing_returns_200` | `GET /v1/jobs/:id` on an existing job returns 200 with correct job data | `cargo test -p anvilml-server --test jobs_tests test_get_job_existing_returns_200` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_get_job_unknown_returns_404` | `GET /v1/jobs/:id` on a non-existent UUID returns 404 | `cargo test -p anvilml-server --test jobs_tests test_get_job_unknown_returns_404` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_list_jobs_before_param_accepted` | `GET /v1/jobs?before=2026-07-08T00:00:00Z` returns 200 (param accepted but ignored by persistence layer) | `cargo test -p anvilml-server --test jobs_tests test_list_jobs_before_param_accepted` exits 0 |

## CI Impact

No CI changes required. The existing `rust-linux` and `rust-windows` CI jobs already run `cargo test --workspace --features mock-hardware`, which picks up the new tests in `jobs_tests.rs`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `DateTime<Utc>` type is cross-platform, and the handlers operate on in-memory SQLite with no path or platform-specific I/O. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobScheduler::list_jobs()` requires a new public method on the scheduler, but `job_store` is a private `Arc<JobStore>` field — the scheduler must expose a delegation method. | Low | Medium | Add a thin `pub async fn list_jobs()` method on `JobScheduler` that delegates to `self.job_store.list(status, limit).await`, mirroring the existing `get_job()` pattern. |
| `ListJobsParams::before` is accepted in the query struct but `JobStore::list()` does not support a before-cursor — the field must not be silently dropped from the struct. | Low | Low | Keep `before: Option<DateTime<Utc>>` in the struct with `#[serde(default)]`. Document the deviation in the plan's Scope and Approach. The handler ignores the field when calling `list()`. |
| Route collision between `GET /v1/jobs` (list) and `GET /v1/jobs/:id` (single) — axum must distinguish the literal path `/v1/jobs` from the parameterised `/v1/jobs/:id`. | Low | High | Axum 0.8+ handles this by matching literal routes before parameterised ones. The route registration order in `build_router()` must list `/v1/jobs` before `/v1/jobs/:id`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test jobs_tests` exits 0 (>=9 tests total)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `head -1 .forge/reports/P14-D2_plan.md` prints `# Plan Report: P14-D2`
- [ ] `grep "^## " .forge/reports/P14-D2_plan.md` shows 12 section headings
- [ ] `wc -l .forge/reports/P14-D2_plan.md` returns > 40 lines
