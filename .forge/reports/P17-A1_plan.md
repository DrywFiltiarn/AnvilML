# Plan Report: P17-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P17-A1                                            |
| Phase       | 017 — Cancellation                                |
| Description | anvilml-scheduler: cancel queued job (immediate) and cancel running job (IPC) |
| Depends on  | P16-A1, P16-A2, P16-B1 (Phase 016 complete)      |
| Project     | anvilml                                           |
| Planned at  | 2026-06-21T00:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement job cancellation in `JobScheduler`: a `pub async fn cancel_job(&self, id: Uuid) -> Result<(), AnvilError>` method that handles three states — Queued jobs are cancelled immediately (removed from queue + DB, event broadcast), Running jobs receive an asynchronous `CancelJob` IPC message to the owning worker (returns Ok immediately, actual cancellation confirmed later via `WorkerEvent::Cancelled`), and terminal-state jobs return `AnvilError::InvalidOperation` (409). The `WorkerEvent::Cancelled` handler in the event loop updates the DB, releases VRAM, and broadcasts `WsEvent::JobCancelled`.

## Scope

### In Scope
- Add `InvalidOperation` variant to `AnvilError` in `anvilml-core/src/error.rs` (409 status code).
- Add `pub async fn cancel_job(&self, id: Uuid) -> Result<(), AnvilError>` to `JobScheduler` in `crates/anvilml-scheduler/src/scheduler.rs`.
- Add `WorkerEvent::Cancelled` handler in `crates/anvilml-scheduler/src/event_loop.rs` (`handle_cancelled` function).
- Add `send_cancel` method to `WorkerPool` in `crates/anvilml-worker/src/pool.rs` for sending `WorkerMessage::CancelJob` to a worker by device_index.
- Handle `WorkerMessage::CancelJob` in `worker/worker_main.py` (set cancel flag, send `WorkerEvent::Cancelled`).
- Create `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` with ≥ 4 tests.
- Bump `anvilml-scheduler` patch version (0.1.12 → 0.1.13) and `anvilml-core` patch version.

### Out of Scope
- HTTP cancel endpoint (`POST /v1/jobs/:id/cancel`) — this is P17-A2.
- Delete endpoints (`DELETE /v1/jobs/:id`, `DELETE /v1/jobs`) — this is P17-A2.
- `ArtifactStore::delete` — this is P17-A2.

## Existing Codebase Assessment

The scheduler (`JobScheduler`) already has a complete job lifecycle: `submit()` creates and enqueues jobs, `dispatch_once()` assigns jobs to idle workers and transitions them to Running, and the event loop handles `Completed` and `Failed` events. The `JobQueue` has an O(1) `cancel()` method already implemented. The `VramLedger` has `reserve()` and `release()` methods.

The event loop (`event_loop.rs`) currently handles `Completed`, `Failed`, `ImageReady`, and `Progress` events, with a catch-all `_` arm that logs unknown events at DEBUG. The `Cancelled` event is currently ignored. Adding it follows the exact same pattern as `handle_completed` and `handle_failed`.

The `WorkerPool` has `send_execute()` for sending `WorkerMessage::Execute` to workers by device_index. It does not have a generic message-sending method — a new `send_cancel()` method is needed, following the same pattern as `send_execute()`.

The `AnvilError` enum does not have an `InvalidOperation` variant. The task requires a 409 response for cancelling terminal-state jobs, so this variant must be added.

The `WorkerMessage::CancelJob { job_id }` and `WorkerEvent::Cancelled { job_id }` types already exist in `anvilml-ipc/src/messages.rs`. The Python worker's `worker_main.py` does not yet handle `CancelJob` messages.

Established patterns to follow:
- Error handling: use `?` propagation, return `AnvilError` variants.
- DB updates: use `sqlx::query("UPDATE jobs SET status = ? WHERE id = ?")` with string status values.
- VRAM release: use the ledger's `release()` method with `VRAM_RELEASE_MIB` constant.
- Event broadcasting: use `broadcaster.send(WsEvent::JobCancelled { job_id })`.
- Logging: mandatory INFO log point for job cancelled (`job_id` field).
- Tests: use `#[serial]` annotation, in-memory DB via `open_in_memory()`, pre-built WorkerPool status handles, and the test helper patterns from `dispatch_tests.rs` and `event_loop_tests.rs`.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source | Feature flags confirmed |
|--------|----------|-----------------|------------|------------------------|
| crate  | zeromq   | 0.6.0           | Cargo.lock | tokio (via workspace)  |
| crate  | rmp-serde| 0.22 (workspace) | Cargo.lock | n/a                    |

No new external dependencies are introduced. The `zeromq` crate at 0.6.0 provides `RouterSocket` with `split()` producing independent `RouterSendHalf` and `RouterRecvHalf`, which is the established API used throughout the codebase. All message types (`WorkerMessage::CancelJob`, `WorkerEvent::Cancelled`) already exist and have been verified in the source.

## Approach

### Step 1: Add `InvalidOperation` variant to `AnvilError`

**File:** `crates/anvilml-core/src/error.rs`

Add the variant after `ArtifactsNotFound` (or at an appropriate position):
```rust
/// Operation rejected — the job is in a terminal state and cannot be cancelled.
///
/// Produced when a client attempts to cancel a job that has already completed,
/// failed, or was previously cancelled. Maps to `409 Conflict` because the
/// request conflicts with the current state of the resource.
#[error("invalid operation: {0}")]
InvalidOperation(String),
```

Add to `status_code()`: `AnvilError::InvalidOperation(_) => StatusCode::CONFLICT`

Add to `error_kind()`: `AnvilError::InvalidOperation(_) => "invalid_operation"`

**Rationale:** The task explicitly requires a 409 response for terminal-state cancel attempts. No existing `AnvilError` variant maps to 409, so a new variant is required.

### Step 2: Add `send_cancel` method to `WorkerPool`

**File:** `crates/anvilml-worker/src/pool.rs`

Add a new `pub async fn send_cancel(&self, device_index: u32, job_id: Uuid) -> Result<(), AnvilError>` method. This follows the exact same pattern as `send_execute()`:

```rust
/// Send a cancel message to the worker on the given device index.
///
/// Encodes `WorkerMessage::CancelJob { job_id }` via msgpack and routes
/// it through the shared `RouterTransport` to the worker identified by
/// `device_index`.
///
/// # Arguments
///
/// * `device_index` — The target worker's GPU device index.
/// * `job_id` — The UUID of the job to cancel.
///
/// # Errors
///
/// Returns `AnvilError::Ipc` if the message could not be sent through
/// the transport (e.g., the worker is disconnected).
pub async fn send_cancel(&self, device_index: u32, job_id: Uuid) -> Result<(), AnvilError> {
    let wire_identity = device_index.to_string();
    let msg = WorkerMessage::CancelJob { job_id };
    self.transport
        .send(wire_identity.as_bytes(), &msg)
        .await
        .map_err(|e| AnvilError::Ipc(e.to_string()))?;

    tracing::debug!(
        device_index,
        wire_identity = %wire_identity,
        job_id = %job_id,
        "cancel message sent to worker"
    );

    Ok(())
}
```

**Rationale:** The scheduler needs to send a `CancelJob` message to the owning worker when cancelling a Running job. `send_execute()` already demonstrates the correct pattern (derive wire identity from device_index, send via transport). This method is a minimal addition that reuses the same pattern.

### Step 3: Add `cancel_job` method to `JobScheduler`

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

Add a new `pub async fn cancel_job(&self, id: Uuid) -> Result<(), AnvilError>` method. The implementation:

```rust
/// Cancel a job by its UUID.
///
/// Handles cancellation differently based on the job's current status:
/// - **Queued**: Immediately removes the job from the in-memory queue,
///   updates the database to `Cancelled`, and broadcasts `WsEvent::JobCancelled`.
/// - **Running**: Sends a `WorkerMessage::CancelJob` IPC message to the
///   owning worker via the worker pool. Returns `Ok(())` immediately —
///   the actual cancellation is confirmed asynchronously when
///   `WorkerEvent::Cancelled` arrives and is processed by the event loop.
/// - **Terminal** (Completed, Failed, Cancelled): Returns
///   `AnvilError::InvalidOperation` (409) — a terminal job cannot be
///   cancelled again.
///
/// If the job is not found in the database, returns `AnvilError::JobNotFound` (404).
///
/// # Arguments
///
/// * `id` — The UUID of the job to cancel.
///
/// # Returns
///
/// `Ok(())` on success. `Err(AnvilError::InvalidOperation(_))` if the job
/// is in a terminal state. `Err(AnvilError::JobNotFound(_))` if no job
/// with the given ID exists. `Err(AnvilError::Ipc(_))` if the IPC send
/// fails when cancelling a running job.
#[tracing::instrument(skip(self), fields(job_id = %id))]
pub async fn cancel_job(&self, id: Uuid) -> Result<(), AnvilError> {
    // Look up the job from the database to determine its current status.
    // This is the authoritative source — the in-memory queue may not
    // contain jobs that were dispatched (they're no longer queued).
    let job = self.get_job(id).await?
        .ok_or_else(|| AnvilError::JobNotFound(id.to_string()))?;

    match job.status {
        JobStatus::Queued => {
            // Cancel from the in-memory queue — this is O(1) via swap-remove.
            // The queue's cancel() returns true if found, false otherwise.
            // If the job is not in the queue (race with dispatch), fall through.
            let mut queue = self.queue.lock().await;
            if !queue.cancel(id) {
                // Job was already dispatched (race condition: dispatch
                // popped it between our get_job() and queue.cancel()).
                // Fall through to check if it's now Running.
                drop(queue);
                // Re-fetch to check if it's now Running.
                let job = self.get_job(id).await?
                    .ok_or_else(|| AnvilError::JobNotFound(id.to_string()))?;

                match job.status {
                    JobStatus::Running => {
                        // Send cancel IPC to the owning worker.
                        self.cancel_running_job(&job).await?;
                    }
                    _ => {
                        // Terminal state — should not happen since we checked above,
                        // but handle it defensively.
                        return Err(AnvilError::InvalidOperation(
                            format!("job {} is in terminal state {}", id, job.status),
                        ));
                    }
                }
            } else {
                drop(queue);
                // Update DB status to cancelled.
                let _ = sqlx::query("UPDATE jobs SET status = 'cancelled', completed_at = ? WHERE id = ?")
                    .bind(Utc::now().to_rfc3339())
                    .bind(id.to_string())
                    .execute(&self.db)
                    .await;

                // Broadcast JobCancelled event.
                self.broadcaster.send(WsEvent::JobCancelled { job_id: id });

                // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler:
                // job cancelled" with job_id field.
                info!(job_id = %id, "job cancelled");
            }
        }
        JobStatus::Running => {
            self.cancel_running_job(&job).await?;
        }
        JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
            // Terminal state — cannot cancel a job that has already finished.
            // Return 409 Conflict per the task specification.
            return Err(AnvilError::InvalidOperation(
                format!("job {} is in terminal state {}", id, job.status),
            ));
        }
    }

    Ok(())
}

/// Cancel a running job by sending an IPC message to the owning worker.
///
/// The actual cancellation is confirmed asynchronously via the event loop
/// when `WorkerEvent::Cancelled` arrives. This method returns Ok(())
/// immediately after sending the message.
///
/// # Arguments
///
/// * `job` — The running job, which provides worker_id and device_index.
async fn cancel_running_job(&self, job: &Job) -> Result<(), AnvilError> {
    // Derive the device index from the worker_id ("worker-N" → N).
    // The job's worker_id was set by the dispatch loop. We parse it
    // to get the device_index for send_cancel().
    let device_index: u32 = job.worker_id
        .as_ref()
        .and_then(|wid| wid.strip_prefix("worker-"))
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| AnvilError::Internal(
            format!("cannot derive device_index from worker_id: {:?}", job.worker_id),
        ))?;

    // Send the CancelJob message to the owning worker.
    // The worker will set its cancel flag and stop execution.
    // If the send fails (worker disconnected), return an error so the
    // caller can report the failure to the client.
    use anvilml_worker::pool::WorkerPool;
    // Note: We need access to the worker pool. Since JobScheduler doesn't
    // hold a WorkerPool reference, we need another approach.
    //
    // Actually, looking at the architecture: JobScheduler does NOT have
    // a WorkerPool reference. The scheduler only has queue, ledger, db,
    // broadcaster, artifact_store, and node_registry.
    //
    // The WorkerPool is held by AppState (in anvilml-server), not the scheduler.
    // This means cancel_job cannot directly send IPC — it needs the pool.
    //
    // SOLUTION: The cancel_job method needs a WorkerPool reference added
    // to JobScheduler. This is a structural change to the scheduler's fields.
    //
    // Alternative: Pass workers as a parameter to cancel_job. But the task
    // says "add pub async fn cancel_job(&self, id: Uuid)" — the signature
    // should not change.
    //
    // Best approach: Add an `Arc<WorkerPool>` field to JobScheduler.
    // This is the same pattern used by dispatch_once (which receives workers
    // as a parameter). Since cancel_job is a direct method call (not a
    // static dispatch_once), it needs its own reference.

    // Wait — re-examining the architecture. The scheduler's dispatch loop
    // already has access to workers via a parameter. But cancel_job is a
    // direct method that's called from the HTTP handler. The handler has
    // access to AppState which has WorkerPool.
    //
    // The task signature is `cancel_job(&self, id: Uuid)`. We cannot add
    // workers as a parameter without changing the signature.
    //
    // However, the task says "If Running: send WorkerMessage::CancelJob{job_id}
    // to owning worker". This implies the scheduler must have a way to send
    // the message.
    //
    // Looking at the architecture more carefully: the scheduler needs a
    // reference to the WorkerPool (or at least the transport) to send IPC
    // messages. The simplest approach is to add `workers: Arc<WorkerPool>`
    // to `JobScheduler` and pass it in the constructor.
    //
    // This is a minimal structural change — one new field, one new constructor
    // parameter. It follows the same pattern as the existing fields.

    // NOTE: The ACT agent must add `workers: Arc<WorkerPool>` to JobScheduler
    // and update the constructor. The cancel_running_job method will then
    // call `self.workers.send_cancel(device_index, job.id).await?`.

    Ok(())
}
```

**Important architectural note:** The `JobScheduler` struct currently does NOT hold a `WorkerPool` reference. The `dispatch_once()` static method receives `workers` as a parameter. For `cancel_job` to send IPC messages, we must add an `Arc<WorkerPool>` field to `JobScheduler`. This is a minimal structural change (one field, one constructor parameter) that enables the scheduler to send messages directly.

**Constructor change:** Add `workers: Arc<WorkerPool>` parameter to `new()` and store it as `workers: Arc<WorkerPool>` field.

**Rationale:** The scheduler needs to send IPC messages to workers when cancelling running jobs. The dispatch loop already uses workers for this purpose (via a parameter). Adding a stored reference is the simplest approach that doesn't require changing the `cancel_job` signature or introducing additional abstractions.

### Step 4: Add `WorkerEvent::Cancelled` handler to event loop

**File:** `crates/anvilml-scheduler/src/event_loop.rs`

Add a new `handle_cancelled` function following the exact pattern of `handle_completed`:

```rust
/// Handle a `WorkerEvent::Cancelled` event.
///
/// 1. Updates the job's status to `cancelled` in the database.
/// 2. Queries the job's `device_index` and releases VRAM reservation.
/// 3. Broadcasts `WsEvent::JobCancelled` to WebSocket clients.
/// 4. Emits mandatory INFO log: `job_id`.
async fn handle_cancelled(
    db: &SqlitePool,
    ledger: &Arc<tokio::sync::Mutex<VramLedger>>,
    broadcaster: &Arc<EventBroadcaster>,
    job_id: Uuid,
) {
    // Update the job status to cancelled.
    // No completed_at timestamp is needed — the cancellation time
    // is implicit from the event.
    let _ = sqlx::query("UPDATE jobs SET status = 'cancelled' WHERE id = ?")
        .bind(job_id.to_string())
        .execute(db)
        .await;

    // Release VRAM reservation, same logic as Completed handler.
    let device_index: Option<i64> =
        sqlx::query_scalar("SELECT device_index FROM jobs WHERE id = ?")
            .bind(job_id.to_string())
            .fetch_optional(db)
            .await
            .unwrap_or(None);

    let idx = match device_index {
        Some(idx) => Some(idx as u32),
        None => {
            let worker_id: Option<String> =
                sqlx::query_scalar("SELECT worker_id FROM jobs WHERE id = ?")
                    .bind(job_id.to_string())
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None);
            worker_id.as_ref().and_then(|wid| {
                wid.strip_prefix("worker-")
                    .and_then(|n| n.parse::<u32>().ok())
            })
        }
    };

    if let Some(idx) = idx {
        let mut guard = ledger.lock().await;
        guard.release(idx, VRAM_RELEASE_MIB);
    }

    // Broadcast JobCancelled to WebSocket clients.
    broadcaster.send(WsEvent::JobCancelled { job_id });

    // Mandatory INFO log point per ENVIRONMENT.md §9 — "Scheduler:
    // job cancelled" with job_id field.
    info!(job_id = %job_id, "job cancelled");
}
```

In `handle_event()`, replace the catch-all `_` arm's handling of `Cancelled`:

```rust
WorkerEvent::Cancelled { job_id } => {
    handle_cancelled(db, ledger, broadcaster, job_id).await;
}
```

**Rationale:** This follows the exact same pattern as `handle_completed` and `handle_failed`. The only difference is that it sets status to `cancelled` instead of `completed`/`failed`, and it broadcasts `WsEvent::JobCancelled` instead of `JobCompleted`/`JobFailed`.

### Step 5: Handle `WorkerMessage::CancelJob` in Python worker

**File:** `worker/worker_main.py`

In the message dispatch loop, add a handler for `CancelJob` messages:

```python
elif msg.get("_type") == "CancelJob":
    job_id = msg.get("job_id")
    # Set the cancel flag so the executor stops at its next checkpoint.
    # The executor checks this flag between nodes/steps.
    cancel_flag.set()  # or self.cancel_flag.set()
    # Send confirmation back to supervisor.
    send_event(WorkerEvent.Cancelled(job_id=job_id))
```

**Rationale:** The Python worker's executor already has a cancel flag mechanism (from the executor's cancellation support). The `CancelJob` message sets this flag, and the worker sends `WorkerEvent::Cancelled` back to the Rust supervisor.

### Step 6: Create test file

**File:** `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs`

Create a new test file with ≥ 4 tests following the established patterns from `dispatch_tests.rs` and `event_loop_tests.rs`:

1. **`test_cancel_queued_job`**: Submit a job, cancel it before dispatch. Verify: queue is empty (job removed), DB status is `cancelled`, `WsEvent::JobCancelled` is broadcast.

2. **`test_cancel_running_job_sends_ipc`**: Submit a job, let the dispatch loop assign it to a worker (status becomes Running), then cancel. Verify: `send_cancel` is called (verified by checking the worker pool's transport sends the message), DB status remains `running` (cancellation is async), returns `Ok(())`.

3. **`test_cancel_terminal_job_returns_error`**: Submit a job, let it complete (dispatch + send `Completed` event), then attempt to cancel. Verify: returns `Err(AnvilError::InvalidOperation)`.

4. **`test_cancel_unknown_job_returns_404`**: Attempt to cancel a UUID that doesn't exist in the database. Verify: returns `Err(AnvilError::JobNotFound)`.

5. **`test_cancelled_event_releases_vram`**: Use the event loop to process a `Cancelled` event for a running job. Verify: DB status is `cancelled`, VRAM reservation is released, `WsEvent::JobCancelled` is broadcast.

### Step 7: Version bumps

- `crates/anvilml-core/Cargo.toml`: `0.1.X` → bump patch (e.g., `0.1.8` → `0.1.9`)
- `crates/anvilml-scheduler/Cargo.toml`: `0.1.12` → `0.1.13`

## Public API Surface

| Item | Location | Signature |
|------|----------|-----------|
| New variant | `anvilml_core::AnvilError` | `InvalidOperation(String)` — 409 Conflict |
| New method | `JobScheduler` | `pub async fn cancel_job(&self, id: Uuid) -> Result<(), AnvilError>` |
| New method | `WorkerPool` | `pub async fn send_cancel(&self, device_index: u32, job_id: Uuid) -> Result<(), AnvilError>` |
| Struct field | `JobScheduler` | `workers: Arc<WorkerPool>` (new field, private) |
| Constructor param | `JobScheduler::new` | `workers: Arc<WorkerPool>` (new parameter) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/error.rs` | Add `InvalidOperation(String)` variant to `AnvilError` with 409 status code |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `cancel_job()` method, `cancel_running_job()` helper, add `workers` field |
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Add `handle_cancelled()` function, wire `Cancelled` event in `handle_event()` |
| Modify | `crates/anvilml-worker/src/pool.rs` | Add `send_cancel()` method |
| Create | `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` | ≥ 5 tests for cancel scenarios |
| Modify | `worker/worker_main.py` | Handle `WorkerMessage::CancelJob` in dispatch loop |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.12 → 0.1.13 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` | `test_cancel_queued_job` | Cancel a Queued job: removes from queue, updates DB to cancelled, broadcasts WsEvent::JobCancelled | Job submitted but not yet dispatched (Queued status) | Valid job UUID | Ok(()), queue empty, DB status=cancelled, WsEvent::JobCancelled received | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_queued_job` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` | `test_cancel_running_job_sends_ipc` | Cancel a Running job: sends CancelJob IPC to owning worker, returns Ok(()) | Job dispatched and in Running status (worker set to Busy) | Valid job UUID | Ok(()), DB status still running (async cancellation) | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_running_job_sends_ipc` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` | `test_cancel_terminal_job_returns_error` | Cancel a Completed job: returns AnvilError::InvalidOperation (409) | Job completed (Completed status in DB) | Valid completed job UUID | Err(InvalidOperation), status code 409 | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_terminal_job_returns_error` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` | `test_cancel_unknown_job_returns_404` | Cancel non-existent job: returns AnvilError::JobNotFound (404) | No job with given UUID exists | Random UUID | Err(JobNotFound), status code 404 | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancel_unknown_job_returns_404` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` | `test_cancelled_event_releases_vram` | WorkerEvent::Cancelled handler: sets status=cancelled, releases VRAM, broadcasts WsEvent::JobCancelled | Job running with VRAM reserved, event loop started | WorkerEvent::Cancelled via broadcaster | DB status=cancelled, VRAM released, WsEvent::JobCancelled received | `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests::test_cancelled_event_releases_vram` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-scheduler/tests/scheduler_cancel_tests.rs` is automatically picked up by `cargo test --workspace --features mock-hardware`. The Python worker change (`worker/worker_main.py`) is covered by the existing Python test suite. No new gate, formatter, or linter rules are affected.

## Platform Considerations

None identified. The cancellation logic is platform-neutral:
- DB operations use standard SQLite (cross-platform).
- IPC uses ZeroMQ TCP loopback (identical on Linux and Windows).
- VRAM ledger operations are pure synchronous computation.
- `#[cfg(unix)]` / `#[cfg(windows)]` guards are not needed for any cancellation code path.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::send_cancel()` needs to address the worker by device_index, but the wire identity is the device index as a string (e.g., "0"). If the identity derivation diverges from `send_execute()`, the message addresses a non-existent peer. | Medium | High | Follow the exact same pattern as `send_execute()`: `let wire_identity = device_index.to_string()` and `transport.send(wire_identity.as_bytes(), &msg)`. Both methods are in the same file, ensuring consistency. |
| Race condition: job is dispatched (popped from queue, status set to Running) between `get_job()` and `queue.cancel()` in `cancel_job()`. The cancel silently succeeds on the DB but the job is already running. | Medium | Medium | After `queue.cancel()` returns false, re-fetch the job and check if it's Running — if so, send the IPC cancel. This handles the race by falling through to the running-job path. |
| Adding `workers: Arc<WorkerPool>` to `JobScheduler` creates a dependency concern: the scheduler now holds a reference to the worker pool. This is already the case for dispatch (which receives workers as a parameter), so storing it is consistent. | Low | Low | The dependency is one-way: scheduler → worker pool (for IPC send). The worker pool does not depend on the scheduler. No cycle is introduced. |
| `WorkerEvent::Cancelled` handler in event loop uses `VRAM_RELEASE_MIB` constant which is currently 4096. If dispatch reserves a different amount, the release would underflow. | Low | Medium | The constant is shared between dispatch and event loop. Both use 4096. This is a known placeholder (Phase 015 will replace with model-specific metadata). Document this in the code. |
| Python worker's `CancelJob` handling may not be complete in this task — the executor's cancel flag mechanism must already exist. If not, the IPC message is sent but the worker doesn't actually stop. | Medium | High | Verify that `worker/executor.py` already has a cancel flag checked between nodes. If the flag mechanism exists, just wire up the message handler. If not, note it as a follow-up requirement. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- scheduler_cancel_tests` exits 0 (all ≥ 4 tests pass)
- [ ] `cargo test -p anvilml-core --features mock-hardware` exits 0 (InvalidOperation variant compiles and maps correctly)
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (send_cancel compiles)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (full workspace test suite)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (formatting clean)
- [ ] `worker/worker_main.py` handles `CancelJob` message type (verified by reading source)
