# Plan Report: P12-A4

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P12-A4                                          |
| Phase       | 012 — Job Submission & Queue                    |
| Description | anvilml-server: wire POST /v1/jobs to scheduler.submit + GET /v1/jobs/:id |
| Depends on  | P12-A3                                           |
| Project     | anvilml                                          |
| Planned at  | 2026-06-07T17:45:00Z                            |
| Attempt     | 1                                                |

## Objective

Wire the `POST /v1/jobs` endpoint to call `JobScheduler::submit()` (replacing the stub that returned a fake UUID) and add a new `GET /v1/jobs/:id` endpoint that retrieves a persisted job from SQLite via `job_store::get_job()`. The scheduler is constructed in `main.rs` and injected into `AppState`.

## Scope

### In Scope
- Add `scheduler: Arc<JobScheduler>` field to `AppState` (in `crates/anvilml-server/src/state.rs`)
- Update both `AppState::new()` and `AppState::new_with_hardware()` constructors to accept an optional `Arc<JobScheduler>` parameter
- Clone the `scheduler` in `AppState::clone()`
- Construct a `JobScheduler` instance in `backend/src/main.rs` (using existing `db`, `broadcaster`, workers, and a new `Notify`)
- Wire `JobScheduler` into `AppState` when building it in `main.rs`
- Update `crates/anvilml-server/src/handlers/jobs.rs`:
  - Rewrite `submit_job()` to call `state.scheduler.submit(req)` → returns 202 with `SubmitJobResponse`, or 422 on `AnvilError::InvalidGraph`
  - Add new `get_job(State, Path<Uuid>)` handler → returns 200 with `Job`, or 404 when not found
- Wire both routes in `crates/anvilml-server/src/lib.rs`:
  - `.route("/v1/jobs", post(handlers::jobs::submit_job))` (existing, already present)
  - `.route("/v1/jobs/{id}", get(handlers::jobs::get_job))` (new)
- Update tests in `handlers/jobs.rs`:
  - `build_test_app()` must construct a minimal `JobScheduler` with an in-memory pool and channel
  - `submit_job_valid_zit_graph_returns_202` now verifies real job_id + queue_position >= 1
  - `submit_job_bad_graph_returns_422` unchanged behavior (still returns 422)
  - Add `get_job_returns_200_with_queued_job()` — submit a valid job, then GET it
  - Add `get_job_returns_404_when_missing()` — GET a nonexistent UUID

### Out of Scope
- GET /v1/jobs list endpoint (task P12-A5)
- Job cancellation, deletion, or other job lifecycle endpoints
- Dispatch loop wiring (phase 13)
- Changes to `anvilml-scheduler` crate internals (already complete from P12-A3)
- OpenAPI doc generation (handled by P12-A5's openapi drift gate, not required here since no new response types are introduced beyond what already exists in utoipa annotations)

## Approach

### Step 1: Add `scheduler` to `AppState` (`state.rs`)

Add field:
```rust
pub scheduler: Arc<JobScheduler>,
```

Update both constructors (`new` and `new_with_hardware`) to accept `Option<Arc<JobScheduler>>`:
- If `Some(s)`, store it directly
- If `None`, panic — the server should never start without a scheduler (it's the core job management component)

Update `Clone::clone()` to include `scheduler: Arc::clone(&self.scheduler)`.

Add import for `JobScheduler` from `anvilml_scheduler`.

### Step 2: Construct `JobScheduler` in `main.rs`

After the existing setup (db, hardware detection, worker pool, broadcaster):
```rust
let notify = Arc::new(Notify::new());
let scheduler = Arc::new(JobScheduler::new(
    JobQueue::new(),
    Arc::clone(&workers_info),  // workers snapshot
    db.clone(),                  // SQLite pool
    Arc::clone(&broadcaster_sender),  // broadcast channel for WS events
    notify,
));
```

Pass `Some(scheduler)` to `AppState::new_with_hardware()`.

The `JobScheduler` needs:
- `JobQueue::new()` — fresh in-memory queue (P12-A2)
- Workers snapshot — `Arc::clone(&workers)` from the already-spawned WorkerPool (we need WorkerInfo list; if no direct accessor exists, pass empty Arc<Vec<WorkerInfo>>)
- `db` — the existing SqlitePool
- `broadcaster` — the existing broadcast channel sender for WsEvent
- `notify` — new `Arc::Notify` for dispatch loop wake-up (unused until phase 13)

### Step 3: Update route wiring (`lib.rs`)

Add one route line to `build_router`:
```rust
.route("/v1/jobs/{id}", get(handlers::jobs::get_job))
```

The existing `/v1/jobs` POST route is already present.

### Step 4: Rewrite `submit_job` handler (`handlers/jobs.rs`)

Replace the current stub implementation with:
```rust
pub async fn submit_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitJobRequest>,
) -> impl IntoResponse {
    match state.scheduler.submit(req).await {
        Ok(resp) => (StatusCode::ACCEPTED, Json(Json(resp))),
        Err(AnvilError::InvalidGraph(msg)) => {
            tracing::warn!(error = %msg, "submit_job: graph validation failed");
            (StatusCode::UNPROCESSABLE_ENTITY, Json(error_body("invalid_graph", &msg)))
        }
        Err(e) => {
            tracing::error!(error = %e, "submit_job: unexpected error");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error_body("internal_error", &e.to_string())))
        }
    }
}
```

Where `error_body` is a small helper returning the standard uniform error JSON structure:
```rust
fn error_body(code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({ "error": code, "message": message, "request_id": Uuid::new_v4().to_string() })
}
```

### Step 5: Add `get_job` handler (`handlers/jobs.rs`)

```rust
pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
    match anvilml_scheduler::get_job(&state.scheduler.db, job_id).await {
        Ok(Some(job)) => (StatusCode::OK, Json(Json(job))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(Json(serde_json::json!({
                "error": "not_found",
                "message": format!("job {} not found", job_id),
            }))),
        ),
        Err(e) => {
            tracing::error!(error = %e, get_job = %job_id, "get_job: database query failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error_body("internal_error", &e.to_string())))
        }
    }
}
```

Note: `JobScheduler` stores `db: sqlx::SqlitePool` as a private field. We need to either:
- Add a public getter `scheduler.db_pool()` returning `&SqlitePool`, or
- Store the pool separately in AppState and pass it to both scheduler and handler

The cleanest approach: add `pub fn db(&self) -> &SqlitePool` to `JobScheduler`. However, since this task is **server-only**, we should avoid modifying the scheduler crate. Instead, store a clone of the pool reference in `AppState`:

Actually, looking more carefully at the scheduler struct, `db` is private (`db: sqlx::SqlitePool`). The handler needs to call `job_store::get_job(&pool, id)`. We have two options:
1. Add a getter on `JobScheduler` — this modifies the scheduler crate (not strictly "server-only")
2. Store `pub db: SqlitePool` in `AppState` and pass it to the handler

Option 2 is cleaner since `state.db` already exists as `pub db: Option<SqlitePool>` in AppState. The handler can unwrap it (it's always Some when scheduler exists). No scheduler crate modification needed.

Revised `get_job`:
```rust
pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = state.db.as_ref().expect("db must be present when scheduler is configured");
    match anvilml_scheduler::get_job(pool, job_id).await {
        Ok(Some(job)) => (StatusCode::OK, Json(Json(job))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(Json(serde_json::json!({
                "error": "not_found",
                "message": format!("job {} not found", job_id),
            }))),
        ),
        Err(e) => {
            tracing::error!(error = %e, get_job = %job_id, "get_job: database query failed");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error_body("internal_error", &e.to_string())))
        }
    }
}
```

### Step 6: Update tests in `handlers/jobs.rs`

Update `build_test_app()` to create a real `JobScheduler`:
- Create an in-memory SQLite pool with the jobs table (reuse setup from scheduler crate tests)
- Create a `broadcast::channel(16)` for WsEvent
- Create a `Notify`
- Build `JobScheduler::new(queue, workers, pool, broadcaster, notify)`
- Pass it to `AppState::new()` as `Some(scheduler)`

Update existing tests:
- `submit_job_valid_zit_graph_returns_202`: verify real `job_id` is non-nil and `queue_position >= 1`
- `submit_job_bad_graph_returns_422`: unchanged behavior

Add new tests:
- `get_job_returns_200_with_queued_job()`: submit a valid job via the test app, extract the job_id from the response, then issue a GET /v1/jobs/{id} and verify 200 with matching job_id and status Queued
- `get_job_returns_404_when_missing()`: GET a random UUID that was never submitted, verify 404

### Step 7: Verify acceptance criteria

The plan includes unit tests in the handler module. No integration test file is needed — the existing test infrastructure (tower::ServiceExt + oneshot) covers both endpoints.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `scheduler: Arc<JobScheduler>` field, update constructors and Clone impl |
| Modify | `backend/src/main.rs` | Construct `JobScheduler`, pass to `AppState::new_with_hardware()` |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `.route("/v1/jobs/{id}", get(...))` route |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Rewrite `submit_job`, add `get_job` handler, update tests |
| Bump | `crates/anvilml-server/Cargo.toml` | Patch version `0.1.1 → 0.1.2` (per FORGE_AGENT_RULES §12) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/handlers/jobs.rs` | `submit_job_bad_graph_returns_422` | Invalid graph → 422 with `"error": "invalid_graph"` |
| `crates/anvilml-server/src/handlers/jobs.rs` | `submit_job_valid_zit_graph_returns_202` | Valid ZiT graph → 202 with real job_id and queue_position >= 1 |
| `crates/anvilml-server/src/handlers/jobs.rs` | `get_job_returns_200_with_queued_job` | Submit + GET: job persisted, status Queued, matches submitted data |
| `crates/anvilml-server/src/handlers/jobs.rs` | `get_job_returns_404_when_missing` | GET nonexistent UUID → 404 with `"error": "not_found"` |

## CI Impact

No CI workflow files are modified. The existing CI gates (format, clippy, test, platform cross-checks) apply automatically. No new dependencies are added — `anvilml-scheduler` is already a dependency of `anvilml-server`. The only version bump is the crate patch version per FORGE_AGENT_RULES §12.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobScheduler::new()` requires workers snapshot (`Arc<Vec<WorkerInfo>>`) but no easy accessor exists on WorkerPool | Medium | Low — pass empty vec; workers field is `_workers` (unused prefix) in scheduler anyway | Use `Arc::new(vec![])` for the workers parameter since it's only a read-only snapshot and the `_workers` prefix signals it's not actively used |
| `AppState::clone()` currently creates independent state; adding Arc<JobScheduler> preserves shared reference | Low | None — `Arc` is designed for cloning; all handlers share the same scheduler instance | No action needed; this is the intended behavior |
| Test `build_test_app()` needs a real DB pool with jobs table; current stub uses in-memory without tables | High | Medium — existing tests would fail if db isn't available | Create pool with migrations or create-table setup inline in test helper (same pattern used in scheduler crate tests) |
| Route ordering conflict: `/v1/jobs` and `/v1/jobs/{id}` in axum | Low | None — axum matches more specific routes first; `/v1/jobs/{id}` is unambiguous | No action needed; axum handles this correctly |
| `JobScheduler` requires `Arc<Notify>` but dispatch loop doesn't exist yet (phase 13) | Low | None — Notify::new() creates a no-op handle; dispatch loop will use it in phase 13 | Create `Notify::new()` here; it's harmless until phase 13 wires the consumer |

## Acceptance Criteria

- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware` — all tests pass (4 handler tests in jobs.rs + existing lib.rs tests)
- [ ] POST a valid ZiT graph body to `/v1/jobs` → returns HTTP 202 with `job_id` (non-nil UUID) and `queue_position >= 1`
- [ ] GET `/v1/jobs/<job_id>` from the submit response → returns HTTP 200 with `status: "Queued"`
- [ ] GET a nonexistent job UUID → returns HTTP 404
- [ ] Crate patch version bumped in `crates/anvilml-server/Cargo.toml` (`0.1.1 → 0.1.2`)
