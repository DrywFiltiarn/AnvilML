# Plan Report: P13-A5

| Field | Value |
|-------|-------|
| Task ID | P13-A5 |
| Phase | 013 — Dispatch & Execute |
| Description | anvilml-scheduler: handle worker Completed/Failed -> terminal status + idle |
| Depends on | P13-A4 |
| Project | anvilml |
| Planned at | 2026-06-09T13:30:00Z |
| Attempt | 1 |

## Objective

Extend the dispatch loop in `anvilml-scheduler/src/scheduler.rs` to handle `WorkerEvent::Completed` and `WorkerEvent::Failed` events from workers: update the job's DB status to the corresponding terminal state (`Completed` or `Failed`), set the worker idle, broadcast the appropriate WebSocket event, and notify the dispatch loop. Also extend the `update_status` function to support terminal status transitions and add a test that verifies a mock job reaches `Completed` in the database.

## Scope

### In Scope
- Extend `update_status()` in `job_store.rs` to accept optional `completed_at` and `error` parameters for terminal transitions (`Running → Completed`, `Running → Failed`)
- Add event subscription and handling in `start_dispatch_loop()` for `WorkerEvent::Completed` and `WorkerEvent::Failed`
- On `Completed`: re-read job from DB, update status to `Completed(completed_at)`, call `workers.set_idle`, broadcast `WsEvent::JobCompleted`, `notify.notify_one()`
- On `Failed`: re-read job from DB, update status to `Failed(error)`, call `workers.set_idle`, broadcast `WsEvent::JobFailed`, `notify.notify_one()`
- Re-read job status from DB before applying events (skip if already terminal)
- Add `publish_event()` test helper to `WorkerPool` (gated behind `#[cfg(test)]`)
- Add `test_complete()` integration test in `scheduler.rs`
- Bump `anvilml-scheduler` patch version `0.1.12 → 0.1.13`

### Out of Scope
- Cancelling jobs on `WorkerEvent::Cancelled` (deferred to a later task)
- Cancelling jobs on `WorkerEvent::Dying` or `WorkerStatusChanged(Dead)` (deferred)
- Any changes to the Python worker (P13-A4 handles that)
- Any changes to `main.rs` or `anvilml-server` (P13-A6 handles startup wiring)
- Any changes to other crates

## Approach

### Step 1: Extend `update_status()` in `job_store.rs`

Modify the `update_status` function signature from:
```rust
pub async fn update_status(pool: &SqlitePool, id: Uuid, new_status: JobStatus, started_at: Option<DateTime<Utc>>) -> Result<bool, sqlx::Error>
```
to:
```rust
pub async fn update_status(
    pool: &SqlitePool,
    id: Uuid,
    new_status: JobStatus,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    error: Option<String>,
) -> Result<bool, sqlx::Error>
```

Update the SQL query to include `completed_at` and `error` columns when present:
```sql
UPDATE jobs SET status = ?, started_at = COALESCE(started_at, started_at), completed_at = ?, worker_id = COALESCE(worker_id, worker_id), error = ? WHERE id = ? AND status != 'Running'
```

Wait — the WHERE clause `status != 'Running'` blocks transitions FROM Running. For Completed/Failed we need to transition FROM Running. Change the guard to only block transitions from terminal states:
```sql
UPDATE jobs SET status = ?, started_at = ?, completed_at = COALESCE(completed_at, completed_at), error = COALESCE(error, error) WHERE id = ? AND status = 'Running'
```

Actually, the dispatch loop already calls `update_status` for `Running` with `started_at` set, and that uses `status != 'Running'` to prevent double-transition. For the Completed/Failed path, we need `status = 'Running'` to only match running jobs. The simplest approach: change the WHERE clause to allow any transition from Running:
```sql
UPDATE jobs SET status = ?, started_at = COALESCE(started_at, started_at), completed_at = COALESCE(completed_at, completed_at), error = COALESCE(error, error) WHERE id = ? AND status = 'Running'
```

This means `update_status` now handles two cases:
1. `Queued → Running`: caller sets `started_at`, `completed_at = None`, `error = None`; guard `status = 'Running'` would NOT match Queued. So we need to handle both.

Better approach: use a conditional WHERE clause. The simplest correct approach is to use:
```sql
UPDATE jobs SET status = ?, started_at = COALESCE(started_at, started_at), completed_at = COALESCE(completed_at, completed_at), error = COALESCE(error, error) WHERE id = ? AND (status = 'Queued' OR status = 'Running')
```

This allows both `Queued → Running` (dispatch loop) and `Running → Completed/Failed` (event handler).

The existing call in `scheduler.rs` (dispatch loop, line ~193) passes `started_at` but not `completed_at` or `error`. Update that call site to pass `None` for the new parameters.

### Step 2: Add event subscription to dispatch loop in `scheduler.rs`

In `start_dispatch_loop()`, add an event subscription before the main loop:
```rust
let mut event_rx = workers.subscribe_events();
```

Inside the dispatch loop's inner `while let Some(job) = queue.pop_next()` block, after the dispatch attempts, add an event processing step. The dispatch loop currently waits on `notify.notified()` with a 100ms timeout. We need to also wake on worker events.

Restructure the wait to use `tokio::select!`:
```rust
tokio::select! {
    _ = notify.notified() => { tracing::debug!("dispatch loop: job submitted notification"); }
    result = event_rx.recv() => {
        match result {
            Ok((worker_id, event)) => {
                tracing::debug!(worker_id = %worker_id, event_type = ?event_discriminant(&event), "dispatch loop: received worker event");
                // Process Completed/Failed below
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!(lagged = n, "dispatch loop: dropped events");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
        tracing::debug!("dispatch loop: timeout, checking queue");
    }
}
```

After receiving a `Completed` or `Failed` event, process it:
```rust
match &event {
    WorkerEvent::Completed { job_id, elapsed_ms } => {
        handle_completed(&db, &workers, &broadcaster, &dispatch_notify, *job_id, now).await;
        dispatch_notify.notify_one();
    }
    WorkerEvent::Failed { job_id, error, traceback: _ } => {
        handle_failed(&db, &workers, &broadcaster, &dispatch_notify, *job_id, error.clone(), now).await;
        dispatch_notify.notify_one();
    }
    _ => {} // Progress, ImageReady, etc. — not handled here
}
```

Extract the event handling into two private async functions:

```rust
async fn handle_completed(
    db: &SqlitePool,
    workers: &Arc<WorkerPool>,
    broadcaster: &broadcast::Sender<WsEvent>,
    notify: &Arc<Notify>,
    job_id: Uuid,
    now: DateTime<Utc>,
) {
    // Re-read job status from DB to confirm it's still Running
    let job = get_job(db, job_id).await.ok().flatten();
    let Some(job) = job else { return };
    if !matches!(job.status, JobStatus::Running) {
        tracing::debug!(job_id = %job_id, status = %job.status, "completed: job already terminal, ignoring");
        return;
    }

    // Update status to Completed
    update_status(db, job_id, JobStatus::Completed, None, Some(now), None)
        .await
        .ok();
    tracing::info!(job_id = %job_id, "job completed");

    // Set worker idle
    if let Some(ref wid) = job.worker_id {
        workers.set_idle(wid).await;
    }

    // Broadcast
    let _ = broadcaster.send(WsEvent::JobCompleted(JobCompletedEvent {
        event: "job.completed".to_string(),
        timestamp: now,
        job_id,
    }));

    // Wake dispatch loop for next job
    notify.notify_one();
}

async fn handle_failed(
    db: &SqlitePool,
    workers: &Arc<WorkerPool>,
    broadcaster: &broadcast::Sender<WsEvent>,
    notify: &Arc<Notify>,
    job_id: Uuid,
    error: String,
    now: DateTime<Utc>,
) {
    let job = get_job(db, job_id).await.ok().flatten();
    let Some(job) = job else { return };
    if !matches!(job.status, JobStatus::Running) {
        tracing::debug!(job_id = %job_id, status = %job.status, "failed: job already terminal, ignoring");
        return;
    }

    update_status(db, job_id, JobStatus::Failed, None, None, Some(error.clone()))
        .await
        .ok();
    tracing::info!(job_id = %job_id, error = %error, "job failed");

    if let Some(ref wid) = job.worker_id {
        workers.set_idle(wid).await;
    }

    let _ = broadcaster.send(WsEvent::JobFailed(JobFailedEvent {
        event: "job.failed".to_string(),
        timestamp: now,
        job_id,
        error,
        traceback: None,
    }));

    notify.notify_one();
}
```

### Step 3: Add `publish_event()` test helper to `WorkerPool`

Add to `crates/anvilml-worker/src/pool.rs` under the `#[cfg(any(test, feature = "test-helpers"))]` impl block:
```rust
/// Publish an event to the pool's broadcast channel (test-only).
///
/// Forwards a `(worker_id, WorkerEvent)` pair to the pool's internal
/// broadcast channel, simulating what a per-worker listener task would send.
pub fn publish_event(&self, worker_id: String, event: anvilml_ipc::WorkerEvent) {
    let _ = self.event_tx.send((worker_id, event));
}
```

### Step 4: Add `test_complete()` integration test

Add to `scheduler.rs` under `#[cfg(test)] mod tests`:

```rust
/// Submitting a job causes the dispatch loop to transition it to Running,
/// then a Completed event from the worker transitions it to Completed.
#[serial]
#[tokio::test]
async fn test_complete() {
    let worker = Arc::new(ManagedWorker::new("worker-0".to_string(), 0));
    worker.set_status(WorkerStatus::Idle).await;

    let pool = Arc::new(WorkerPool::new_test_pool_with_workers(vec![worker.clone()]));
    let ledger = Arc::new(tokio::sync::Mutex::new(VramLedger::new()));
    let (broadcaster, _rx) = broadcast::channel(16);
    let scheduler = JobScheduler::new(
        JobQueue::new(),
        pool.clone(),
        setup_pool().await,
        broadcaster,
        ledger,
        "auto".to_string(),
    );

    let dispatch_handle = scheduler.start_dispatch_loop();

    // Submit a job — triggers dispatch loop.
    let req = SubmitJobRequest {
        graph: valid_zit_graph(),
        settings: JobSettings::default(),
    };
    let resp = scheduler.submit(req).await.expect("submit succeeded");

    // Wait for dispatch to move job to Running.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify job is Running.
    let db_job = get_job(&scheduler.db, resp.job_id)
        .await
        .expect("get from DB succeeded")
        .expect("job exists");
    assert_eq!(db_job.status, JobStatus::Running, "job should be Running after dispatch");

    // Inject Completed event via pool test helper.
    let now = Utc::now();
    pool.publish_event(
        "worker-0".to_string(),
        WorkerEvent::Completed {
            job_id: resp.job_id,
            elapsed_ms: 42,
        },
    );

    // Wait for event handler to process.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify job is Completed in DB.
    let db_job = get_job(&scheduler.db, resp.job_id)
        .await
        .expect("get from DB succeeded")
        .expect("job exists");
    assert_eq!(db_job.status, JobStatus::Completed, "job should be Completed after event");
    assert!(db_job.completed_at.is_some(), "completed_at should be set");

    // Verify worker is back to Idle.
    let infos = pool.list().await;
    assert_eq!(infos[0].status, WorkerStatus::Idle, "worker should be Idle after completion");

    dispatch_handle.abort();
}
```

### Step 5: Bump crate version

Update `crates/anvilml-scheduler/Cargo.toml`: `version = "0.1.12"` → `version = "0.1.13"`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add event subscription, handle Completed/Failed in dispatch loop, add `handle_completed`/`handle_failed` functions, add `test_complete` test |
| Modify | `crates/anvilml-scheduler/src/job_store.rs` | Extend `update_status()` signature with `completed_at` and `error` params, update SQL WHERE clause to allow `Running → Completed/Failed` transitions |
| Modify | `crates/anvilml-worker/src/pool.rs` | Add `publish_event()` test helper to `WorkerPool` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.12 → 0.1.13` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-scheduler/src/scheduler.rs` | `test_complete` | End-to-end: submit job → dispatch → Running → worker Completed event → Completed in DB, worker back to idle |

## CI Impact

No CI changes required. The existing CI gates (`cargo test --workspace --features mock-hardware`, clippy, format checks) will cover the new code. The `test_complete` test uses `#[serial]` to avoid parallelism conflicts with other scheduler tests that share the same in-memory SQLite pool pattern.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `update_status` SQL change breaks existing dispatch loop call | Medium | High | The new WHERE clause `(status = 'Queued' OR status = 'Running')` is a superset of the old `status != 'Running'` guard for the dispatch case. Existing `test_dispatch_sends_execute` test validates this path. |
| Event subscription races with dispatch loop timeout | Low | Medium | Using `tokio::select!` with a 100ms sleep as fallback ensures both notification and event paths are checked. The test sleeps 500ms before injecting events, providing ample margin. |
| `publish_event` not accessible from scheduler tests | Low | Medium | Adding it behind `#[cfg(any(test, feature = "test-helpers"))]` on `WorkerPool` makes it available to all crates that depend on `anvilml-worker` with the `test-helpers` feature (already enabled in scheduler's dev-dependencies). |
| Re-read from DB misses the update race window | Low | Low | The dispatch loop holds the tokio::sync::Mutex across the await, so no concurrent status change can occur between the DB re-read and the update. SQLite in-memory is single-writer. |

## Acceptance Criteria

- [ ] `update_status` accepts `completed_at: Option<DateTime<Utc>>` and `error: Option<String>` parameters
- [ ] Dispatch loop subscribes to `workers.subscribe_events()` and processes `Completed`/`Failed` events
- [ ] On `Completed`: job DB status → `Completed(completed_at)`, worker → idle, `JobCompleted` broadcast, `notify.notify_one()`
- [ ] On `Failed`: job DB status → `Failed(error)`, worker → idle, `JobFailed` broadcast, `notify.notify_one()`
- [ ] Events for already-terminal jobs are ignored (DB re-read guard)
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- complete` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes with zero warnings
- [ ] `anvilml-scheduler` patch version bumped to `0.1.13`
