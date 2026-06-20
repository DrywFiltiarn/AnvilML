# Plan Report: P13-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-B1                                            |
| Phase       | 013 — Job Queue & Persistence                     |
| Description | anvilml-server: POST /v1/jobs, GET /v1/jobs, GET /v1/jobs/:id wired to scheduler |
| Depends on  | P13-A3 (JobScheduler with submit, get_job, list_jobs) |
| Project     | anvilml                                           |
| Planned at  | 2026-06-20T05:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace the Phase 012 placeholder `submit_job` handler with a real implementation that delegates to `JobScheduler::submit()`, and add two new handlers (`list_jobs`, `get_job`) wired to `GET /v1/jobs` and `GET /v1/jobs/:id`. After this task, submitted jobs are persisted to SQLite, assigned a real queue position, broadcast as `WsEvent::JobQueued`, and queryable via the REST API. The acceptance criterion is `cargo test -p anvilml-server --features mock-hardware` exits 0.

## Scope

### In Scope
- Add `scheduler: Arc<JobScheduler>` field to `AppState` struct in `state.rs`.
- Update all three `AppState` constructors (`new`, `new_with_hardware`, `new_with_hardware_no_workers`) to accept and store a scheduler.
- Replace `submit_job` handler in `handlers/jobs.rs` to call `scheduler.submit(req).await` — removing the pre-check for empty registry and placeholder UUID generation.
- Add `list_jobs` handler in `handlers/jobs.rs` accepting query params (`status`, `limit`, `before`), delegating to `scheduler.list_jobs()`.
- Add `get_job` handler in `handlers/jobs.rs` accepting `Path<Uuid>`, returning 200 with `Job` JSON or 404 via `AnvilError::JobNotFound`.
- Export `list_jobs` and `get_job` from `handlers/mod.rs`.
- Mount `GET /v1/jobs` and `GET /v1/jobs/:id` routes in `build_router()` in `lib.rs`.
- Initialise `JobScheduler` in `backend/src/main.rs` before constructing `AppState`.
- Update all test files that call `AppState::new()` to pass a real `JobScheduler` built with in-memory database.
- Bump `anvilml-server` crate version from `0.1.21` to `0.1.22`.

### Out of Scope
- Implementing `POST /v1/jobs/:id/cancel`, `DELETE /v1/jobs/:id`, `DELETE /v1/jobs` (future tasks).
- Modifying `JobScheduler` internals (belongs to P13-A3).
- Adding integration tests for the dispatch loop (Phase 014).
- Modifying `anvilml-scheduler` source code.

## Existing Codebase Assessment

**What already exists:** The `JobScheduler` struct is fully implemented in `crates/anvilml-scheduler/src/scheduler.rs` with three public async methods: `submit()`, `get_job()`, and `list_jobs()`. The scheduler owns a `JobQueue`, `VramLedger`, `NodeTypeRegistry` reference, `SqlitePool`, and `EventBroadcaster`. The `submit()` method validates the graph via `validate_graph()`, persists the job to SQLite via `insert_job()`, pushes to the in-memory queue, broadcasts `WsEvent::JobQueued`, and returns `SubmitJobResponse { job_id, queue_position }`.

The `submit_job` handler in `handlers/jobs.rs` currently performs the same validation (via `validate_graph` through `state.node_registry`) but returns a placeholder UUID without persisting. It checks `state.node_registry.is_empty()` to gate on worker readiness.

`AppState` currently has no scheduler field. The three constructors (`new`, `new_with_hardware`, `new_with_hardware_no_workers`) accept `node_registry` but not `JobScheduler`.

`build_router()` in `lib.rs` mounts only `POST /v1/jobs` — no list or get routes exist.

**Established patterns:**
- Handlers return `Result<(StatusCode, Json<T>), AnvilError>` for success-with-status or error responses. `AnvilError` implements `IntoResponse` with correct HTTP status codes.
- Query params are extracted via `axum::extract::Query<T>` where `T` has `Option` fields.
- Path params use `axum::extract::Path<T>`.
- Tests use `AppState::new()` with in-memory database and `build_router()`, then call `router.oneshot(request)`.
- Logging uses `tracing::info!` with structured fields; mandatory INFO log points are defined in ENVIRONMENT.md §9.
- Every `pub` item has a `///` doc comment.

**Gap between design doc and source:** The design doc §12.4 specifies `POST /v1/jobs` returns 202, but the current handler returns 202 with a placeholder UUID. The scheduler's `submit()` also returns 202-equivalent (no status override needed — `StatusCode::ACCEPTED` is the natural fit). The `before` query parameter is defined in the design doc as a query filter but is not yet wired in any handler. The `Job` struct in `anvilml-core` derives `Serialize, Deserialize, ToSchema` — it will serialise correctly for JSON responses.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | anvilml-scheduler | 0.1.3 (workspace) | Cargo.lock | mock-hardware (forwarded) |

No new external crates are introduced. `JobScheduler` is already declared in the workspace. The `anvilml-server` crate already depends on `anvilml-scheduler` via path dependency.

## Approach

### Step 1: Add `scheduler` field to `AppState` and update constructors

**File:** `crates/anvilml-server/src/state.rs`

Add `pub scheduler: Arc<anvilml_scheduler::JobScheduler>` as a field on `AppState`.

Update `new()` to accept `scheduler: Arc<anvilml_scheduler::JobScheduler>` as an additional parameter (after `node_registry`). Store it in the struct. The `new()` constructor creates an in-memory `SqlitePool` via `anvilml_registry::open_in_memory()` — the scheduler will use this same pool.

Update `new_with_hardware()` to accept `scheduler: Arc<anvilml_scheduler::JobScheduler>` as an additional parameter (after `node_registry`). Store it.

Update `new_with_hardware_no_workers()` to accept `scheduler: Arc<anvilml_scheduler::JobScheduler>` as an additional parameter. Store it.

Each constructor update adds one parameter and one field assignment. No logic changes.

### Step 2: Update `submit_job` handler

**File:** `crates/anvilml-server/src/handlers/jobs.rs`

Replace the entire `submit_job` function body. The new implementation:
1. Extracts `State(state)` and `Json(req)`.
2. Calls `state.scheduler.submit(req).await`.
3. On `Ok(response)`, returns `(StatusCode::ACCEPTED, Json(response))`.
4. On `Err(e)`, returns `Err(e)` — the `AnvilError` `IntoResponse` impl maps:
   - `InvalidGraph` → 422
   - `Db` → 500
   - `Serde` → 500
   - Other errors → their mapped status codes

Remove the `is_empty()` check, the `validate_graph` call, and the placeholder UUID generation. The scheduler handles all of this internally.

Keep the `#[tracing::instrument]` attribute and the doc comment (updated to reflect the new behaviour).

### Step 3: Add `list_jobs` handler

**File:** `crates/anvilml-server/src/handlers/jobs.rs`

Add a new async function:

```rust
pub async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<ListJobsQuery>,
) -> Result<Json<Vec<Job>>, AnvilError>
```

Where `ListJobsQuery` is a helper struct:

```rust
#[derive(Debug, Deserialize)]
pub struct ListJobsQuery {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub before: Option<String>,
}
```

The handler:
1. Parses `status` from string to `JobStatus` (match on `"queued"`, `"running"`, `"completed"`, `"failed"`, `"cancelled"`, or `None`). If the string doesn't match any variant, return 400 with `AnvilError::Internal("invalid status filter")`.
2. Parses `before` from RFC3339 string to `DateTime<Utc>`. If parsing fails, return 400 with `AnvilError::Internal("invalid before timestamp")`.
3. Calls `state.scheduler.list_jobs(status, limit, before).await`.
4. Returns `Ok(Json(jobs))`.

### Step 4: Add `get_job` handler

**File:** `crates/anvilml-server/src/handlers/jobs.rs`

Add a new async function:

```rust
pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Job>, AnvilError>
```

The handler:
1. Calls `state.scheduler.get_job(id).await`.
2. On `Ok(Some(job))`, returns `Ok(Json(job))`.
3. On `Ok(None)`, returns `Err(AnvilError::JobNotFound(id.to_string()))` (404).
4. On `Err(e)`, returns `Err(e)`.

### Step 5: Export new handlers from `handlers/mod.rs`

**File:** `crates/anvilml-server/src/handlers/mod.rs`

Add:
```rust
pub use jobs::list_jobs;
pub use jobs::get_job;
```

### Step 6: Mount routes in `build_router()`

**File:** `crates/anvilml-server/src/lib.rs`

Add route registrations:
- `.route("/v1/jobs", get(list_jobs))` — for `GET /v1/jobs`
- `.route("/v1/jobs/{id}", get(get_job))` — for `GET /v1/jobs/:id` (axum uses `{id}` syntax, not `:id`)

Update the existing `POST /v1/jobs` route comment to note it now delegates to the scheduler.

The import section needs:
```rust
use handlers::jobs::{submit_job, list_jobs, get_job};
```

### Step 7: Initialise `JobScheduler` in `main.rs`

**File:** `backend/src/main.rs`

After the `node_registry` is created (line 156) and before constructing `AppState`, create the scheduler:

```rust
// Build the job scheduler with the node registry, database pool,
// and event broadcaster. The queue and ledger are freshly initialised
// — the queue starts empty and the ledger has no registered devices
// (VRAM checks are added in Phase 014).
let scheduler = Arc::new(JobScheduler::new(
    Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
    Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
    Arc::clone(&node_registry),
    pool.clone(),
    Arc::new(EventBroadcaster::new()),
));
```

This requires adding imports:
- `anvilml_scheduler::{JobScheduler, queue::JobQueue, ledger::VramLedger}`
- `anvilml_ipc::EventBroadcaster` (already available via `anvilml_server` re-export, but import directly from `anvilml_ipc` for clarity)

Then pass `scheduler.clone()` to all three `AppState` constructors. The `new_with_hardware_no_workers()` call at line 170 and the `new_with_hardware()` call at line 198 both need the scheduler.

For `AppState::new()` in tests, the scheduler uses the same in-memory pool that `open_in_memory()` creates.

### Step 8: Update all test files that call `AppState::new()`

**Files affected:** All test files under `crates/anvilml-server/tests/` that call `AppState::new()`.

Each test must create a `JobScheduler` with in-memory database before calling `AppState::new()`:

```rust
let pool = anvilml_registry::open_in_memory().await.expect("in-memory pool for test");
let registry = NodeTypeRegistry::new().await;
let scheduler = Arc::new(JobScheduler::new(
    Arc::new(tokio::sync::Mutex::new(JobQueue::default())),
    Arc::new(tokio::sync::Mutex::new(VramLedger::new())),
    Arc::new(registry.clone()),
    pool,
    Arc::new(EventBroadcaster::new()),
));
let state = AppState::new("test-version", Arc::new(registry), scheduler).await;
```

**Files to update:**
- `jobs_tests.rs` — 3 calls to `AppState::new()` (all 3 existing tests need the scheduler)
- `workers_tests.rs` — 1 call
- `system_tests.rs` — 1 call
- `nodes_tests.rs` — 2 calls
- `models_tests.rs` — 4 calls
- `health_tests.rs` — 1 call
- `handler_tests.rs` — 2 calls
- `state_tests.rs` — 3 calls

**Specific test changes for `jobs_tests.rs`:**
- `test_submit_job_returns_503_when_no_workers`: This test expects 503 when the registry is empty. With the new implementation, the scheduler's `submit()` will call `validate_graph()` which checks the registry. When the registry is empty, `validate_graph()` will return errors for any graph containing nodes (because no node types are registered). For an empty graph `{}`, it will return 422 (invalid graph) rather than 503. The test should be updated to expect 422 with `invalid_graph` error, since an empty registry means unknown node types for any non-trivial graph.
- `test_submit_job_returns_422_with_unknown_node_type`: Still expects 422 — will continue to pass. The scheduler's `submit()` calls `validate_graph()` which detects the unknown type.
- `test_submit_job_returns_202_with_valid_graph`: Still expects 202 — will continue to pass. The scheduler's `submit()` validates, persists, enqueues, and returns `SubmitJobResponse`. Update assertion: `queue_position` should be `1` (not `0`) since the scheduler uses 1-based indexing (`queue.len() as u32 + 1`).

### Step 9: Add new tests for `list_jobs` and `get_job`

**File:** `crates/anvilml-server/tests/jobs_tests.rs`

Add two new integration tests:

1. **`test_list_jobs_returns_queued_jobs`**: Submit a job via POST, then GET /v1/jobs and verify the returned list contains the job with `status: "queued"`. Uses a real scheduler with in-memory DB.

2. **`test_get_job_returns_404_for_unknown_id`**: Call GET /v1/jobs/{uuid} with a UUID that was never submitted, verify 404 response with `error: "job_not_found"`.

### Step 10: Bump `anvilml-server` crate version

**File:** `crates/anvilml-server/Cargo.toml`

Change `version = "0.1.21"` to `version = "0.1.22"`.

## Public API Surface

### New `pub` items in `anvilml-server`

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `list_jobs` handler | `anvilml_server::handlers::jobs` | `pub async fn list_jobs(State<AppState>, Query<ListJobsQuery>) -> Result<Json<Vec<Job>>, AnvilError>` |
| `get_job` handler | `anvilml_server::handlers::jobs` | `pub async fn get_job(State<AppState>, Path<Uuid>) -> Result<Json<Job>, AnvilError>` |
| `ListJobsQuery` | `anvilml_server::handlers::jobs` | `pub struct ListJobsQuery { status: Option<String>, limit: Option<u32>, before: Option<String> }` |
| `AppState.scheduler` field | `anvilml_server::state` | `pub scheduler: Arc<anvilml_scheduler::JobScheduler>` (new field on existing struct) |

### Modified `pub` items

| Item | Crate/Module | Before | After |
|------|-------------|--------|-------|
| `AppState::new()` | `anvilml_server::state` | `async fn new(version, node_registry)` | `async fn new(version, node_registry, scheduler)` |
| `AppState::new_with_hardware()` | `anvilml_server::state` | `(version, hardware, db, registry, model_dirs, workers, node_registry)` | `+ scheduler` |
| `AppState::new_with_hardware_no_workers()` | `anvilml_server::state` | `(version, hardware, db, registry, model_dirs, node_registry)` | `+ scheduler` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `scheduler` field; update 3 constructors |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Replace `submit_job`; add `list_jobs`, `get_job` |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Export `list_jobs`, `get_job` |
| Modify | `crates/anvilml-server/src/lib.rs` | Mount `GET /v1/jobs`, `GET /v1/jobs/{id}` routes; update imports |
| Modify | `backend/src/main.rs` | Create `JobScheduler`; pass to `AppState` constructors |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump version 0.1.21 → 0.1.22 |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Update 3 existing tests; add 2 new tests |
| Modify | `crates/anvilml-server/tests/workers_tests.rs` | Update `AppState::new()` call |
| Modify | `crates/anvilml-server/tests/system_tests.rs` | Update `AppState::new()` call |
| Modify | `crates/anvilml-server/tests/nodes_tests.rs` | Update 2 `AppState::new()` calls |
| Modify | `crates/anvilml-server/tests/models_tests.rs` | Update 4 `AppState::new()` calls |
| Modify | `crates/anvilml-server/tests/health_tests.rs` | Update `AppState::new()` call |
| Modify | `crates/anvilml-server/tests/handler_tests.rs` | Update 2 `AppState::new()` calls |
| Modify | `crates/anvilml-server/tests/state_tests.rs` | Update 3 `AppState::new()` calls |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `jobs_tests.rs` | `test_submit_job_returns_503_when_no_workers` | POST /v1/jobs with empty registry returns 422 (scheduler validates, empty registry → unknown node types) | Fresh registry with no workers | Empty graph `{}` | 422, `"error": "invalid_graph"` | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_submit_job_returns_503_when_no_workers` exits 0 |
| `jobs_tests.rs` | `test_submit_job_returns_422_with_unknown_node_type` | POST /v1/jobs with unknown node type returns 422 | Registry with LoadModel only | Graph with `GhostNode` | 422, `"error": "invalid_graph"` | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_submit_job_returns_422_with_unknown_node_type` exits 0 |
| `jobs_tests.rs` | `test_submit_job_returns_202_with_valid_graph` | POST /v1/jobs with valid graph returns 202 with real UUID and queue_position=1 | Registry with LoadModel | Valid graph with `LoadModel` node | 202, valid `job_id`, `queue_position: 1` | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_submit_job_returns_202_with_valid_graph` exits 0 |
| `jobs_tests.rs` | `test_list_jobs_returns_queued_jobs` | GET /v1/jobs returns submitted jobs with correct status | Job persisted via submit | None | 200, list contains job with `status: "queued"` | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_list_jobs_returns_queued_jobs` exits 0 |
| `jobs_tests.rs` | `test_get_job_returns_404_for_unknown_id` | GET /v1/jobs/{uuid} returns 404 for unknown UUID | None | Any UUID | 404, `"error": "job_not_found"` | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_get_job_returns_404_for_unknown_id` exits 0 |
| All test files | (all existing tests) | All `AppState::new()`-based tests still compile and pass | Scheduler injected | N/A | Full test suite exits 0 | `cargo test -p anvilml-server --features mock-hardware` exits 0 |

## CI Impact

No CI changes required. The existing CI jobs (`rust-linux`, `rust-windows`) run `cargo test --workspace --features mock-hardware` which includes `anvilml-server` tests. The new routes and handlers are covered by the Rust test suite. No new file types, gates, or test modules are added. The OpenAPI drift gate (if enabled) would need regeneration after implementation since new endpoints are added, but that is an ACT-phase concern.

## Platform Considerations

None identified. The task introduces no platform-specific code. All changes are in handler logic and state management, which are platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `AppState::new()` signature change breaks all 17 call sites across 8 test files — a missed update will cause a compile error. | High | High | Update all 8 test files systematically. After Step 8, run `cargo test -p anvilml-server --features mock-hardware` to confirm all compile and pass. Fix any remaining call site before proceeding. |
| The `before` query parameter parsing (RFC3339 → DateTime<Utc>) may fail for malformed input, returning a 500 error instead of 400. | Low | Medium | Use `DateTime::parse_from_rfc3339()` with proper error handling in the handler — map parse errors to `AnvilError::Internal` with a descriptive message. The `IntoResponse` impl maps `Internal` to 500; if needed, a custom error variant for bad query params can be added later. |
| `test_submit_job_returns_503_when_no_workers` behaviour change: scheduler's `submit()` calls `validate_graph()` which returns errors for unknown node types when registry is empty, rather than returning 503. The test must be updated to expect 422. | Medium | Medium | Document this explicitly in the test changes section. The new assertion (422, invalid_graph) is more correct — an empty registry means no node types are known, so any graph with nodes fails validation. |
| `queue_position` value changes from `0` to `1` in the 202 response. The existing test asserts `queue_position == 0`. | High | Low | Update the assertion to expect `1` (1-based indexing per scheduler implementation). This is a documented change in the test table. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo fmt --all -- --check` exits 0 (format gate)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (lint gate)
- [ ] `head -1 .forge/reports/P13-B1_plan.md` prints `# Plan Report: P13-B1`
- [ ] `grep "^## " .forge/reports/P13-B1_plan.md` shows 12 section headings
- [ ] `wc -l .forge/reports/P13-B1_plan.md` returns > 40 lines
