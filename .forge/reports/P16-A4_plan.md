# Plan Report: P16-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P16-A4                                            |
| Phase       | 016 — Job Cancellation                            |
| Description | Integration test for cancel of a running mock job |
| Depends on  | P16-A1, P16-A2, P16-A3                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-10T13:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `backend/tests/api_cancel.rs`, an integration test that exercises the full job cancellation flow using the live axum server with `mock-hardware` and `ANVILML_WORKER_MOCK=1`. The test must verify: (1) submitting a slow mock job, waiting until it transitions to Running, POSTing `/v1/jobs/:id/cancel` returns 202, the WebSocket stream emits `job.cancelled`, GET `/v1/jobs/:id` returns status `Cancelled`, and the worker returns to Idle within 3 seconds; (2) cancelling a terminal (Completed) job returns 409 with `job_not_cancellable`.

## Scope

### In Scope
- Create `backend/tests/api_cancel.rs` with two test functions:
  - `cancel_running_job_returns_202_and_ws_cancelled`: full end-to-end cancel flow with mock worker and `ANVILML_MOCK_NODE_DELAY_MS` set.
  - `cancel_terminal_job_returns_409`: submit a job, advance it to Completed via event injection, then cancel → 409.
- Use `temp-env` crate for env var isolation (set `ANVILML_MOCK_DEVICE_TYPE`, `ANVILML_MOCK_VRAM_MIB`, `ANVILML_WORKER_MOCK`, `ANVILML_MOCK_NODE_DELAY_MS`).
- Follow the existing integration test pattern from `api_ws_lifecycle.rs` (in-memory DB, EventBroadcaster, JobScheduler, mock WorkerPool, hyper HTTP client, tokio-tungstenite WS client).
- Mark test with `#[serial]` to prevent parallel env-var interference.

### Out of Scope
- No changes to source code, handlers, scheduler, worker, or IPC protocol.
- No new dependencies (all required crates are already in `backend/Cargo.toml` dev-dependencies).
- No CI workflow modifications (the test is discovered automatically by `cargo test --test api_cancel`).
- No version bumps (only test file created, no crate source modified).

## Approach

1. **File creation:** Create `backend/tests/api_cancel.rs` with two `#[serial] #[tokio::test]` async functions.

2. **Test 1 — `cancel_running_job_returns_202_and_ws_cancelled`:**
   - Set env vars: `ANVILML_MOCK_DEVICE_TYPE=cuda`, `ANVILML_MOCK_VRAM_MIB=8192`, `ANVILML_WORKER_MOCK=1`, `ANVILML_MOCK_NODE_DELAY_MS=400`.
   - Build server components: in-memory DB via `anvilml_registry::open_in_memory()`, artifact store, `EventBroadcaster`, `JobScheduler` with mock `WorkerPool`, `App`, and `build_router`.
   - Bind to a random port via `TcpListener::bind("127.0.0.1:0")`.
   - Start the server in a `tokio::spawn` task.
   - Connect a `tokio_tungstenite` WebSocket client to `/v1/events`.
   - POST a valid ZiT 2-node graph (same as `api_ws_lifecycle.rs`) to `/v1/jobs`. Assert 202. Parse `job_id` from response.
   - Poll `GET /v1/jobs/{job_id}` in a loop (up to 3 s, 100 ms intervals) until `status == "Running"`. This confirms the job is executing with the mock delay.
   - POST `DELETE /v1/jobs/{job_id}/cancel` (note: the route is `POST /v1/jobs/:id/cancel`). Assert 202 response status.
   - Read from the WebSocket stream within a 5-second deadline and assert the next WS event variant is `JobCancelled` (maps to `"job.cancelled"`).
   - Poll `GET /v1/jobs/{job_id}` again and assert status is `"Cancelled"`.
   - Poll `GET /v1/workers` and assert the worker status is `"Idle"` within 3 seconds.
   - Cleanup: abort server handle, remove all env vars via `std::env::remove_var`.

3. **Test 2 — `cancel_terminal_job_returns_409`:**
   - Set same mock env vars.
   - Build the same server stack (in-memory DB, scheduler, router).
   - Submit a valid ZiT job via POST `/v1/jobs`. Assert 202. Parse `job_id`.
   - Directly update the DB to set status to `Completed` (same pattern as `cancel_job_returns_409_for_completed_job` unit test in `handlers/jobs.rs`): `UPDATE jobs SET status = 'Completed', completed_at = ? WHERE id = ?`.
   - POST `/v1/jobs/{job_id}/cancel`. Assert 409.
   - Parse response body and assert `"error" == "job_not_cancellable"`.
   - Cleanup: remove env vars.

4. **Helper functions** (module-level, private):
   - `minimal_zit_graph()` — returns a 2-node ZiT graph (same as `api_ws_lifecycle.rs`).
   - `python_on_path()` — checks Python availability (same guard as `api_ws_lifecycle.rs`).
   - `build_test_app()` — shared setup: in-memory DB, artifact store, scheduler, broadcaster, router (reduces duplication between the two tests).

5. **Environment variable cleanup:** Use `temp-env::async_test` or manual save/restore pattern. Since `temp-env` supports `async_closure` feature, use `temp_env::with_vars` to scope env vars per test. On session end, remove all set vars unconditionally.

## Files Affected

| Action | Path                              | Description                                                     |
|--------|-----------------------------------|-----------------------------------------------------------------|
| Create | `backend/tests/api_cancel.rs`     | Integration test: cancel of a running mock job + terminal job   |

## Tests

| Test File                      | Test Name                                    | What It Verifies                                                        |
|--------------------------------|----------------------------------------------|-------------------------------------------------------------------------|
| `backend/tests/api_cancel.rs`  | `cancel_running_job_returns_202_and_ws_cancelled` | Submit → Running → cancel → 202 + WS `job.cancelled` + DB `Cancelled` + worker Idle within 3s |
| `backend/tests/api_cancel.rs`  | `cancel_terminal_job_returns_409`            | Submit → DB Completed → cancel → 409 + `job_not_cancellable` error body |

## CI Impact

No CI workflow files are modified. The new test file is automatically discovered by `cargo test --workspace --features mock-hardware` and `cargo test --features mock-hardware --test api_cancel`. The test requires Python on PATH (same guard as `api_ws_lifecycle.rs` — returns early if Python is not found). No OpenAPI drift gate required (no handler or schema changes).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Test flakes due to timing (race between cancel HTTP response and WS event delivery) | Medium | High | Use generous deadlines (5 s for WS read, 3 s for worker idle poll). Use `temp-env` for env isolation. The `#[serial]` attribute prevents parallel interference. |
| Python not available on the test runner | Low | Medium | Guard with `python_on_path()` check — skip the test with a clear message, same pattern as `api_ws_lifecycle.rs`. |
| Mock node delay not long enough for cancel to arrive mid-flight | Low | Medium | Use `ANVILML_MOCK_NODE_DELAY_MS=400` (400 ms per node) which gives a wide window between job start and completion. Poll for Running status before cancelling. |
| Test environment pollution from previous tests | Low | Medium | Use `temp-env` crate (already in dev-dependencies) with `async_closure` feature to scope env vars. Unconditional `remove_var` teardown. |

## Acceptance Criteria

- [ ] `backend/tests/api_cancel.rs` exists with two test functions as specified
- [ ] `cargo test --features mock-hardware --test api_cancel` exits 0 (all tests pass)
- [ ] Cancelling a running job returns HTTP 202
- [ ] WebSocket stream receives `job.cancelled` event after cancel
- [ ] GET `/v1/jobs/:id` returns status `Cancelled` after cancel
- [ ] Worker returns to Idle within 3 seconds of cancel
- [ ] Cancelling a terminal (Completed) job returns HTTP 409 with `job_not_cancellable` error
- [ ] Test uses `#[serial]` and `temp-env` for env var isolation
- [ ] Test skips gracefully if Python is not on PATH
- [ ] `cargo fmt --all -- --check` passes on the new file
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes

