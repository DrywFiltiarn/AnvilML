# Plan Report: P17-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P17-C1                                            |
| Phase       | 17 — Cancellation                                 |
| Description | anvilml-server: POST /v1/jobs/:id/cancel handler  |
| Depends on  | P17-A2, P17-B5                                    |
| Project     | anvilml                                           |
| Planned at  | 2026-07-11T17:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add the `cancel_job` HTTP handler for `POST /v1/jobs/{id}/cancel` that delegates entirely to `JobScheduler::cancel()` and returns the correct HTTP status code based on the scheduler's result: 202 Accepted for cancellable jobs (queued/running), 409 Conflict for already-terminal jobs, and 404 Not Found for unknown job IDs. Wire the new route into `build_router()` and add ≥5 cancellation-specific tests to `jobs_tests.rs` (≥14 total in the file).

## Scope

### In Scope
- Add `pub(crate) async fn cancel_job(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse` in `crates/anvilml-server/src/handlers/jobs.rs`.
- Register the `POST /v1/jobs/{id}/cancel` route in `build_router()` in `crates/anvilml-server/src/lib.rs`.
- Modify `JobScheduler::cancel()` return type from `Result<bool, AnvilError>` to `Result<CancelOutcome, AnvilError>` to distinguish "not found" from "already terminal" (required for the 404-vs-409 split).
- Add ≥5 cancellation tests to `crates/anvilml-server/tests/jobs_tests.rs`: cancel Queued → 202, cancel Completed → 409, cancel unknown ID → 404, cancel Running → 202, cancel already-cancelled → 409.
- Bump `anvilml-server` patch version (0.1.15 → 0.1.16).
- Bump `anvilml-scheduler` patch version (source of the `CancelOutcome` type change).

### Out of Scope
- The `DELETE /v1/jobs/{id}` handler (not mentioned in this task's scope; may exist from prior work but is not modified here).
- Worker-side cancel logic (P17-B5).
- Scheduler cancel status-branching implementation (P17-A1/A2).
- Runnable Proof (P17-D1).

defers_to (from JSON): absent

## Existing Codebase Assessment

**What already exists:** The `JobScheduler::cancel()` method (lines 335–461 of `scheduler.rs`) already implements status-aware branching: it checks the in-memory queue first (Queued → lazy-remove + DB update), then falls back to the database to branch on status (Running → send `CancelJob` via transport; terminal → no-op; not found → no-op). The method returns `Result<bool, AnvilError>` where `Ok(true)` means accepted and `Ok(false)` means no-op.

**Established patterns:** Handlers in `jobs.rs` follow a consistent pattern: they receive `State<AppState>` and optional `Path`/`Query`/`Json` extractors, delegate to a single scheduler method, and return `Result<T, AnvilError>` where `AnvilError`'s `IntoResponse` impl maps errors to HTTP status codes. The `submit_job` handler (line 62–78) is the closest existing example — it takes `State` + `Json`, calls `state.scheduler.submit()`, and returns `(StatusCode, Json<...>)`. The `get_job` handler (line 152–161) is the pattern for path-parameter lookups — it takes `Path<Uuid>`, calls the scheduler, and returns `Result<Json<Job>, AnvilError>`.

**Gap between design doc and current source:** `JobScheduler::cancel()` currently returns `Result<bool, AnvilError>` where `Ok(false)` is used for **both** "job not found" and "already terminal". This conflates the two cases that the HTTP handler needs to distinguish as 404 vs 409. The handler cannot tell whether a `false` result means "job doesn't exist" or "job is already completed" — so the 404-vs-409 split is impossible with the current return type. This is a load-bearing discrepancy that must be fixed in this task by changing `cancel()`'s return type from `Result<bool, AnvilError>` to `Result<CancelOutcome, AnvilError>`.

## Resolved Dependencies

None. All types and APIs used are from existing workspace dependencies (axum, uuid, tokio) that are already declared in `anvilml-server/Cargo.toml` and `anvilml-scheduler/Cargo.toml`. No new crates are introduced.

| Type   | Name     | Version verified | MCP source     | Feature flags confirmed |
|--------|----------|-----------------|----------------|------------------------|
| (none) | (none)   | (n/a)           | (n/a)          | (n/a)                   |

## Approach

### Step 1: Add `CancelOutcome` enum to `anvilml-scheduler`

In `crates/anvilml-scheduler/src/scheduler.rs`, add a new public enum before the `impl JobScheduler` block:

```rust
/// The result of a `JobScheduler::cancel()` call.
///
/// Distinguishes three outcomes needed by the HTTP handler:
/// - `Accepted` — job was in a cancellable state (Queued or Running) and cancellation
///   was accepted. For Queued jobs, the status is immediately updated to Cancelled.
///   For Running jobs, a cooperative CancelJob IPC signal has been sent.
/// - `AlreadyTerminal` — job exists but is in a terminal state (Completed/Failed/Cancelled);
///   cancelling is a no-op. The HTTP handler maps this to 409 Conflict.
/// - `NotFound` — no job with the given ID exists in the database. The HTTP handler
///   maps this to 404 Not Found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    /// Cancellation accepted (queued or running).
    Accepted,
    /// Job exists but is already in a terminal state — no-op.
    AlreadyTerminal,
    /// No job found with the given ID.
    NotFound,
}
```

This enum replaces the `bool` return value, making the "not found" vs "already terminal" distinction explicit at the type level.

### Step 2: Update `JobScheduler::cancel()` to return `CancelOutcome`

Modify the `cancel()` method signature from:
```rust
pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>
```
to:
```rust
pub async fn cancel(&self, id: Uuid) -> Result<CancelOutcome, AnvilError>
```

Update the return statements:
- Queued branch (line 361): `return Ok(CancelOutcome::Accepted);`
- Running branch (line 424): `Ok(CancelOutcome::Accepted)`
- Running branch with no worker_id (line 436): unchanged — still returns `Err(AnvilError::Internal(...))`
- Terminal branch (line 450): `Ok(CancelOutcome::AlreadyTerminal)`
- Not found branch (line 458): `Ok(CancelOutcome::NotFound)`

Update the doc comment to reflect the new return type and its three variants.

### Step 3: Add `cancel_job` handler in `jobs.rs`

Add a new handler function in `crates/anvilml-server/src/handlers/jobs.rs` after `get_job()`:

```rust
/// Cancel a job by its ID.
///
/// Accepts a job UUID as a path parameter (`/v1/jobs/{id}/cancel`). Delegates
/// entirely to `JobScheduler::cancel()`, which returns a `CancelOutcome`
/// indicating whether the cancellation was accepted, the job was already
/// terminal, or the job does not exist.
///
/// # Response
///
/// - `202 Accepted` — the job was in a cancellable state (Queued or Running)
///   and cancellation was accepted.
/// - `409 Conflict` — the job exists but is already in a terminal state
///   (Completed/Failed/Cancelled). Cancelling a finished job is a no-op, not
///   an error, per the idempotent-cancel principle.
/// - `404 Not Found` — no job with the given ID exists in the database.
///
/// State is injected via `axum::extract::State<AppState>` which provides
/// access to the `JobScheduler` through `state.scheduler`.
#[tracing::instrument(skip(state), fields(job_id = %id))]
pub(crate) async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // Delegate to the scheduler's cancel method. The CancelOutcome enum
    // distinguishes three cases so we can return the correct HTTP status:
    // - Accepted → 202 (queued or running job accepted for cancellation)
    // - AlreadyTerminal → 409 (job is already finished — no-op, not an error)
    // - NotFound → 404 (job ID does not exist in the database)
    // AnvilError is returned via ? for DB-level failures (mapped to 500).
    match state.scheduler.cancel(id).await {
        Ok(CancelOutcome::Accepted) => {
            tracing::info!(job_id = %id, "cancel accepted");
            StatusCode::ACCEPTED
        }
        Ok(CancelOutcome::AlreadyTerminal) => {
            tracing::debug!(job_id = %id, "cancel rejected: already terminal");
            StatusCode::CONFLICT
        }
        Ok(CancelOutcome::NotFound) => {
            tracing::debug!(job_id = %id, "cancel: job not found");
            StatusCode::NOT_FOUND
        }
        Err(e) => Err(e), // Propagate DB errors (→ 500) via IntoResponse
    }
}
```

This handler has **zero business logic** — it purely maps the scheduler's `CancelOutcome` to HTTP status codes, following `ANVILML_DESIGN.md §3.3`.

### Step 4: Wire the route into `build_router()`

In `crates/anvilml-server/src/lib.rs`, update the `/v1/jobs/{id}` route registration from:
```rust
.route("/v1/jobs/{id}", axum::routing::get(handlers::jobs::get_job))
```
to:
```rust
.route(
    "/v1/jobs/{id}",
    axum::routing::get(handlers::jobs::get_job)
        .post(handlers::jobs::cancel_job),
)
```

This adds the `POST` method to the existing `{id}` capture route. Axum matches the literal path `/v1/jobs` first (the GET+POST route above it), then falls through to the parameterised `/v1/jobs/{id}` route where both GET and POST are now registered.

### Step 5: Add cancellation tests to `jobs_tests.rs`

Add five new `#[tokio::test]` functions to `crates/anvilml-server/tests/jobs_tests.rs`:

1. **`test_cancel_queued_job_returns_202`**: Submit a job (it enters Queued), call `POST /v1/jobs/{id}/cancel`, assert 202.
2. **`test_cancel_completed_job_returns_409`**: Submit a job, manually update its DB status to `Completed` (via `job_store`), call cancel, assert 409.
3. **`test_cancel_unknown_id_returns_404`**: Call cancel with a random UUID never submitted, assert 404.
4. **`test_cancel_running_job_returns_202`**: Submit a job, manually set its DB status to `Running` with a `worker_id`, call cancel, assert 202 (the transport send is a no-op in tests since no real worker is connected).
5. **`test_cancel_already_cancelled_job_returns_409`**: Submit a job, cancel it once (returns 202), cancel again, assert 409 (idempotent-cancel rejection).

Each test follows the existing pattern: create `AppState` via `make_test_state()`, build router, make HTTP request via `router.oneshot()`, assert status code.

### Step 6: Bump crate versions

- `crates/anvilml-server/Cargo.toml`: `0.1.15` → `0.1.16`
- `crates/anvilml-scheduler/Cargo.toml`: bump patch version (check current version, increment Z)

## Public API Surface

### New type in `anvilml-scheduler`

```rust
// crates/anvilml-scheduler/src/scheduler.rs
pub enum CancelOutcome {
    Accepted,
    AlreadyTerminal,
    NotFound,
}
```

### Modified signature in `anvilml-scheduler`

```rust
// Before:
pub async fn cancel(&self, id: Uuid) -> Result<bool, AnvilError>

// After:
pub async fn cancel(&self, id: Uuid) -> Result<CancelOutcome, AnvilError>
```

### New handler in `anvilml-server`

```rust
// crates/anvilml-server/src/handlers/jobs.rs
pub(crate) async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse
```

### New route in `anvilml-server`

```
POST /v1/jobs/{id}/cancel → handlers::jobs::cancel_job
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `CancelOutcome` enum; update `cancel()` return type and all return sites |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Add `cancel_job()` handler function |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire `POST /v1/jobs/{id}/cancel` route into `build_router()` |
| Modify | `crates/anvilml-server/tests/jobs_tests.rs` | Add ≥5 cancellation tests |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.15 → 0.1.16 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `jobs_tests.rs` | `test_cancel_queued_job_returns_202` | Submitting a job and cancelling it while Queued returns 202 | `cargo test -p anvilml-server --test jobs_tests -- test_cancel_queued_job_returns_202` |
| `jobs_tests.rs` | `test_cancel_completed_job_returns_409` | Cancelling a job with DB status = Completed returns 409 | `cargo test -p anvilml-server --test jobs_tests -- test_cancel_completed_job_returns_409` |
| `jobs_tests.rs` | `test_cancel_unknown_id_returns_404` | Cancelling a UUID that was never submitted returns 404 | `cargo test -p anvilml-server --test jobs_tests -- test_cancel_unknown_id_returns_404` |
| `jobs_tests.rs` | `test_cancel_running_job_returns_202` | Cancelling a job with DB status = Running returns 202 (IPC send is best-effort) | `cargo test -p anvilml-server --test jobs_tests -- test_cancel_running_job_returns_202` |
| `jobs_tests.rs` | `test_cancel_already_cancelled_job_returns_409` | Cancelling a job that was already cancelled (idempotent) returns 409 | `cargo test -p anvilml-server --test jobs_tests -- test_cancel_already_cancelled_job_returns_409` |

Acceptance command for full suite:
```bash
cargo test -p anvilml-server --test jobs_tests
# → ≥14 tests total, exits 0
```

## CI Impact

No CI job changes. The new handler and tests are within the existing `anvilml-server` crate, which is already tested by the `rust-linux` and `rust-windows` CI jobs via `cargo test --workspace --features mock-hardware`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `cancel_job` handler is platform-neutral — it makes no syscalls that differ between Unix and Windows, uses no platform-specific types, and does not touch file paths or process management. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobScheduler::cancel()` currently returns `Result<bool, AnvilError>` where both "not found" and "already terminal" map to `Ok(false)`. Changing the return type to `Result<CancelOutcome, AnvilError>` is a breaking change for any code that calls `cancel()`. | Medium | High | Verify at compile time: grep for all call sites of `cancel()` in the codebase. Currently only the HTTP handler (not yet written) would call it, so there are zero existing callers. The change is safe. If any other caller exists, add the import and update the match. |
| The `Running` cancel branch in `cancel()` sends a `CancelJob` message via `self.transport.send()`. In tests, this send may fail (no real worker connected), but the method still returns `Ok(CancelOutcome::Accepted)` because the cancellation was accepted even if the signal didn't reach the worker. | Low | Medium | The test `test_cancel_running_job_returns_202` does not assert on the transport send result — it only asserts the HTTP status is 202, which is correct per the design (cancellation accepted ≠ signal delivered). The existing `cancel()` code already handles this: send failure logs a warning but still returns `Ok(true)`. |
| Axum route ordering: if `/v1/jobs/{id}/cancel` is registered as a separate route from `/v1/jobs/{id}`, the path `{id}/cancel` may not match because axum's `{id}` capture greedily consumes everything after `/v1/jobs/`. | Low | High | Register `cancel_job` as a POST on the **same** `/v1/jobs/{id}` route (not a separate `/v1/jobs/{id}/cancel` route). The handler itself can distinguish GET from POST via the request method, but since we're registering `.post(cancel_job)` on the same path, axum dispatches POST requests to `cancel_job` and GET requests to `get_job`. This is the correct approach. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test jobs_tests` exits 0 with ≥14 total tests
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:8488/v1/jobs/{unknown_uuid}/cancel` returns 404 (verifiable against a running server)
- [ ] `curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:8488/v1/jobs/{queued_uuid}/cancel` returns 202 for a Queued job (verifiable against a running server)
