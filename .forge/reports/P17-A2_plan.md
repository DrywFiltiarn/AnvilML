# Plan Report: P17-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-A2                                      |
| Phase       | 17 — Cancellation                           |
| Description | anvilml-scheduler: cancel() sends WorkerMessage::CancelJob for Running jobs |
| Depends on  | P17-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-11T00:58:37Z                        |
| Attempt     | 1                                           |

## Objective

Complete the `Running` branch of `JobScheduler::cancel()` by sending a cooperative `WorkerMessage::CancelJob{job_id}` via the ZeroMQ transport to the job's assigned worker. The scheduler does not change the job's status — it remains `Running` until the Python worker processes the cancellation and emits `WorkerEvent::Cancelled`, which Phase 16's event loop persists. Acceptance: ≥4 new tests exercising the IPC send to the correct worker_id, status preservation after cancel(), and error handling for a Running job with no assigned worker_id; `cargo test -p anvilml-scheduler --test scheduler_tests` exits 0 (≥9 total in the file).

## Scope

### In Scope
- Modify `JobScheduler::cancel()`'s `Running` branch in `crates/anvilml-scheduler/src/scheduler.rs` to send `WorkerMessage::CancelJob{job_id}` via the transport to the job's `worker_id`.
- Add `transport: Arc<RouterTransport>` field to `JobScheduler` and update `JobScheduler::new()` to accept it (required so `cancel()` can reach the transport without changing its public signature).
- Handle the edge case where a `Running` job has `worker_id: None` — return `AnvilError::Internal` with a descriptive message rather than panicking.
- Update logging in the `Running` branch: replace the "deferred to P17-A2" debug log with an info-level log for successful send and a warn-level log for send failure.
- Add ≥4 new tests in `crates/anvilml-scheduler/tests/scheduler_tests.rs`.

### Out of Scope
- None. `defers_to (from JSON): []` — this task implements its full scope. No deferrals.
- The HTTP handler (`P17-C1`) that exposes `POST /v1/jobs/:id/cancel` — handled by a separate task.
- The Python worker's `CancelJob` handling and `cancel_flag` setting (`P17-B5`) — handled by a separate task.
- The event loop's `Cancelled` event persistence (`P16-A2`) — already complete.

## Existing Codebase Assessment

The codebase already has:
- **`JobScheduler::cancel()`** with status-aware branching (from P17-A1): `Queued` (immediate, queue-level cancel + DB update), `Running` (returns `Ok(true)` with a TODO comment and "deferred to P17-A2" log), and terminal/unknown (returns `Ok(false)`).
- **`WorkerMessage::CancelJob { job_id }`** defined in `anvilml-ipc/src/messages.rs` — the exact variant this task will construct and send.
- **`RouterTransport::send(&self, worker_id: &str, msg: &WorkerMessage)`** in `anvilml-ipc/src/transport.rs` — the method that serialises via msgpack, builds a 3-frame ROUTER multipart message, and sends it. Returns `Result<(), IpcError>`.
- **`WorkerPool::transport()`** returning `&Arc<RouterTransport>` — the shared transport owned by the pool.
- **`Job.worker_id: Option<String>`** — the field that identifies which worker received the job during dispatch. In the `Running` branch, this is typically `Some("0")` etc., but the edge case of `None` must be handled.
- **18 test functions** in `scheduler_tests.rs` (as of P17-A1), including `test_cancel_running_job_returns_true_no_ipc` which creates a Running job and verifies status stays `Running` — this test will need updating since cancel() will now attempt a send.

Established patterns to follow:
- Error handling: `AnvilError::Internal(String)` for unexpected internal conditions (e.g., a Running job with no worker_id).
- Logging: `tracing::info!` for successful operations, `tracing::warn!` for recoverable failures, `tracing::error!` for unrecoverable failures. Structured field notation (`field = %value`).
- Test style: Each test constructs a `JobStore` via `create_job_store()`, a registry via `make_registry()`, and a scheduler via `JobScheduler::new()`. Tests use `persist_job_test()` to insert jobs with arbitrary statuses.
- The `#[cfg(feature = "test-util")]` gate on test-only public methods (`dispatch_one_test`, `persist_job_test`, etc.).

Gap between design doc and source: The design doc says `cancel()` sends an IPC signal for Running jobs, but the current source only returns `Ok(true)` with a TODO comment. This task closes that gap.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | zeromq  | 0.6.0           | rust-docs MCP  | tokio-runtime, all-transport |
| crate  | rmp-serde | 1.3.1         | rust-docs MCP  | (none — default features) |

No new external dependencies are introduced. The task uses existing crates already in the scheduler's dependency graph:
- `anvilml-ipc` (already a dev-dependency via `anvilml-worker` transitively; the `RouterTransport` is accessed through the field added to `JobScheduler`)
- `anvilml-worker` (already a dependency of `anvilml-scheduler` for `WorkerPool`)

## Approach

### Step 1: Add transport field to JobScheduler and update constructor

Add `transport: Arc<RouterTransport>` as a new field on `JobScheduler` (placed after `artifact_store`, following the existing pattern of subsystem fields). Update `JobScheduler::new()` to accept `transport: Arc<RouterTransport>` as a fourth parameter. This is required so `cancel()` can reach the transport without changing its public `cancel(&self, id)` signature — the transport is available as a field on `self`.

The `RouterTransport` type comes from `anvilml_ipc::RouterTransport` (already imported via `anvilml_ipc::WorkerMessage` at the top of scheduler.rs — add `RouterTransport` to the import).

### Step 2: Implement the Running branch IPC send

In `cancel()`'s `Running` branch, replace the current stub:
```rust
// defers_to: P17-A2 — IPC send of WorkerMessage::CancelJob
tracing::info!(job_id = %id, "cancel: Running job — IPC send deferred to P17-A2");
Ok(true)
```

With the actual implementation:
```rust
JobStatus::Running => {
    // The job is running on a worker. Send a cooperative CancelJob signal.
    // The worker checks its cancel_flag between node execution steps;
    // cancel() does NOT change the job's status here — the event loop
    // (Phase 16) transitions to Cancelled when WorkerEvent::Cancelled arrives.
    match &job.worker_id {
        Some(worker_id) => {
            // Build and send the CancelJob message via the transport.
            let msg = WorkerMessage::CancelJob { job_id: id };
            if let Err(e) = self.transport.send(worker_id, &msg).await {
                // Send failure is a warning, not a fatal error.
                // The cancellation was accepted — it's just that the signal
                // didn't reach the worker. The worker may be slow, the
                // network may be congested, or the worker may have died
                // (in which case the keepalive watchdog will detect it).
                tracing::warn!(
                    job_id = %id,
                    worker_id = %worker_id,
                    error = %e,
                    "cancel: Running job — CancelJob send failed (cancellation still accepted)"
                );
            } else {
                tracing::info!(
                    job_id = %id,
                    worker_id = %worker_id,
                    "cancel: Running job — CancelJob sent"
                );
            }
            Ok(true)
        }
        None => {
            // A Running job without a worker_id is an unexpected state —
            // this should never happen in normal operation because the
            // dispatch loop (dispatch_one) sets worker_id when transitioning
            // a job to Running. If it occurs, it indicates a bug elsewhere
            // in the system. Return an Internal error rather than panicking.
            tracing::error!(
                job_id = %id,
                "cancel: Running job has no worker_id — internal error"
            );
            Err(AnvilError::Internal(
                "Running job has no assigned worker_id".into(),
            ))
        }
    }
}
```

Rationale: The task says "Cooperative, not forceful" — we send a signal and return `Ok(true)` regardless of send success/failure. The signal might not reach the worker, but the cancellation was accepted. A send failure is a warning, not a reason to return an error from `cancel()`.

### Step 3: Update the defers_to comment

Remove the `// defers_to: P17-A2` comment at the stub site since the deferred work is now complete. Per FORGE_AGENT_RULES.md §9.7, the comment marker exists only when `defers_to` is non-empty and a stub remains — here, the stub is replaced with real implementation.

### Step 4: Add new tests

Add four new tests to `scheduler_tests.rs`:

**Test 1: `test_cancel_running_sends_cancel_job`**
- Create a Running job with `worker_id: Some("0".into())` via `persist_job_test()`.
- Call `cancel()`.
- Verify `Ok(true)` is returned.
- Verify the job's status is still `Running`.
- Use the test harness with mock workers to verify the send was attempted (the send will fail since no real worker is listening, but the warning path is exercised).

**Test 2: `test_cancel_running_status_stays_running`
- Same setup as test 1, but specifically verify that `job.status` remains `JobStatus::Running` after `cancel()` returns — this is the same assertion as the existing `test_cancel_running_job_returns_true_no_ipc` but now with the send path active. The existing test may need to be updated to handle the send failure path.

**Test 3: `test_cancel_running_no_worker_id_errors`**
- Create a Running job with `worker_id: None` (simulating the unexpected state).
- Call `cancel()`.
- Verify `Err(AnvilError::Internal(...))` is returned.
- Verify the error message mentions "worker_id".

**Test 4: `test_cancel_running_send_failure_handled`**
- Create a Running job with `worker_id: Some("0".into())`.
- Call `cancel()` — the send will fail (no real worker listening).
- Verify `Ok(true)` is still returned (send failure doesn't change the outcome).
- Verify the status remains `Running`.

### Step 5: Update the existing test `test_cancel_running_job_returns_true_no_ipc`

The existing test creates a Running job with `worker_id: Some("0".into())` and calls `cancel()`. After the change, `cancel()` will attempt a send to worker "0", which will fail (no real worker listening). The test's assertion that `Ok(true)` is returned still holds (send failure → `Ok(true)`), but the status check needs to account for the fact that the send path is now active. Update the test's doc comment to reflect that the IPC send is no longer deferred.

### Step 6: Update all callers of `JobScheduler::new()`

Every place that constructs a `JobScheduler` must now pass `transport: Arc<RouterTransport>` as the fourth argument. This includes:
- `backend/main.rs` (production construction — pass the pool's transport)
- All test files in `crates/anvilml-scheduler/tests/` (pass a transport from a `WorkerPool::new()`-constructed pool)

## Public API Surface

| Item | Type | Crate/Module Path | Change |
|------|------|-------------------|--------|
| `JobScheduler::new()` | fn | `anvilml_scheduler::JobScheduler::new` | Added 4th parameter: `transport: Arc<RouterTransport>` |
| `JobScheduler::cancel()` | fn | `anvilml_scheduler::JobScheduler::cancel` | No signature change. Implementation changed (IPC send added). |
| `JobScheduler::transport` | field | `anvilml_scheduler::scheduler::JobScheduler` | New field: `transport: Arc<RouterTransport>` (private) |

No new `pub` items are introduced. Only the constructor signature changes.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `transport` field, update `new()`, implement Running branch IPC send in `cancel()` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.26 → 0.1.27 |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add 4 new tests, update existing `test_cancel_running_job_returns_true_no_ipc` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `scheduler_tests.rs` | `test_cancel_running_sends_cancel_job` | Running job with worker_id sends CancelJob via transport; returns Ok(true); status stays Running | `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_sends_cancel_job` |
| `scheduler_tests.rs` | `test_cancel_running_status_stays_running` | Status remains `Running` immediately after `cancel()` returns (no status change in cancel()) | `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_status_stays_running` |
| `scheduler_tests.rs` | `test_cancel_running_no_worker_id_errors` | Running job with `worker_id: None` returns `Err(AnvilError::Internal)` rather than panicking | `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_no_worker_id_errors` |
| `scheduler_tests.rs` | `test_cancel_running_send_failure_handled` | Send failure (no real worker listening) still returns `Ok(true)` — cancellation accepted even if signal doesn't arrive | `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_send_failure_handled` |

## CI Impact

No CI job changes. The test suite is already exercised by the `rust-linux` and `rust-windows` CI jobs via `cargo test --workspace --features mock-hardware`. The new tests are in the `anvilml-scheduler` crate's `tests/` directory, which is picked up by the existing test command.

## Platform Considerations

None identified. The transport send is platform-neutral — it uses ZeroMQ's cross-platform TCP transport. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Changing `JobScheduler::new()` signature breaks production callers in `backend/main.rs` | Medium | High | Read `backend/main.rs` before writing to confirm the exact construction site. Add the transport parameter there. The change is straightforward — pass `Arc::clone(&pool.transport())` or equivalent. |
| Send failure on every cancel() in tests (no real worker listening) causes all existing tests to hit the warning path | High | Medium | The warning path still returns `Ok(true)`, so existing tests pass. Update the existing `test_cancel_running_job_returns_true_no_ipc` doc comment to reflect the active send path. |
| The transport field adds a dependency on `anvilml-ipc::RouterTransport` in the scheduler's struct definition | Low | Low | `anvilml-ipc` is already a dev-dependency of `anvilml-scheduler` (via `anvilml-worker`). Need to add it as a regular dependency if not already present. Check `Cargo.toml` — it's not currently in `[dependencies]`, only in `[dev-dependencies]`. Must add `anvilml-ipc = { path = "../anvilml-ipc" }` to `[dependencies]` and import `RouterTransport` from it. |
| A Running job with `worker_id: None` is genuinely impossible in production (dispatch_one always sets it) | Low | Low | The error handling is defensive — it returns `AnvilError::Internal` rather than panicking. In practice, this branch should never be hit. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests` exits 0 with ≥9 total tests in the file
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_sends_cancel_job` exits 0
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_no_worker_id_errors` exits 0
- [ ] `cargo test -p anvilml-scheduler --test scheduler_tests test_cancel_running_send_failure_handled` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `grep -n "deferred to P17-A2" crates/anvilml-scheduler/src/scheduler.rs` returns 0 lines (stub comment removed)
- [ ] `grep -n "defers_to: P17-A2" crates/anvilml-scheduler/src/scheduler.rs` returns 0 lines (§9.7 marker removed since work is complete)
