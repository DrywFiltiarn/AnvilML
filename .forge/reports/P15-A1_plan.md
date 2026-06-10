# Plan Report: P15-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P15-A1                                            |
| Phase       | 015 — Live Job Events                             |
| Description | anvilml-scheduler: emit JobProgress events from worker Progress |
| Depends on  | P14-A5                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-10T07:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Wire the missing `WorkerEvent::Progress` handler into the scheduler's dispatch loop so that
progress updates from the Python worker are translated into `WsEvent::JobProgress` events
and broadcast to all WebSocket subscribers. Additionally, confirm that all five job-lifecycle
event types (`JobQueued`, `JobStarted`, `JobProgress`, `JobImageReady`, `JobCompleted`) are
already wired through the broadcaster — `JobQueued` and `JobStarted` are emitted in
`submit()` and the dispatch loop respectively, while `JobImageReady` and `JobCompleted` are
handled in dedicated handler functions.

## Scope

### In Scope
- Add a `WorkerEvent::Progress` arm in the `match &event` block inside `start_dispatch_loop()`
  that constructs and broadcasts `WsEvent::JobProgress` with `step` and `step_total` set to
  `None` (MVP).
- Add a `WorkerEvent::Cancelled` arm that calls `handle_cancelled()` — transition the job to
  `Cancelled` status, set the worker idle, and broadcast `WsEvent::JobCancelled`.
- Add a `handle_cancelled()` async helper function following the same pattern as
  `handle_completed()` and `handle_failed()`.
- Add a unit test `test_progress_broadcasts_event` that verifies `WsEvent::JobProgress` is
  broadcast when a `WorkerEvent::Progress` event is injected via the test pool.
- Add a unit test `test_cancel_broadcasts_event` that verifies `WsEvent::JobCancelled` is
  broadcast when a `WorkerEvent::Cancelled` event is injected.
- Bump `anvilml-scheduler` patch version from `0.1.15` to `0.1.16`.

### Out of Scope
- Integration WebSocket test (`api_ws_lifecycle.rs`) — that is task P15-A2.
- Documentation proof (`PROOF_phase015.md`) — that is task P15-A3.
- Any changes to the Python worker code.
- Any changes to the WebSocket handler or broadcaster.
- Any changes to crates outside `anvilml-scheduler`.

## Approach

### Step 1 — Add Progress handler in dispatch loop

In `crates/anvilml-scheduler/src/scheduler.rs`, inside the `match &event` block of the
`event_rx.recv()` branch (currently around line 178), add a new arm **before** the `_ => {}`
catch-all:

```rust
WorkerEvent::Progress {
    job_id,
    node_index,
    node_total,
    node_type,
    step: _,
    step_total: _,
} => {
    let now = Utc::now();
    // Broadcast JobProgress event (step/step_total None in MVP).
    let _ = broadcaster.send(WsEvent::JobProgress(JobProgressEvent {
        event: "job.progress".to_string(),
        timestamp: now,
        job_id: *job_id,
        node_index: *node_index,
        node_total: *node_total,
        node_type: node_type.clone(),
        step: None,
        step_total: None,
    }));
    tracing::debug!(
        job_id = %job_id,
        node_index = *node_index,
        node_type = %node_type,
        "dispatch loop: progress event broadcast"
    );
}
```

Also add `WorkerEvent::Cancelled` handling:

```rust
WorkerEvent::Cancelled { job_id, reason: _ } => {
    let now = Utc::now();
    handle_cancelled(&db, &workers, &broadcaster, &notify, *job_id, now).await;
    notify.notify_one();
}
```

And update the catch-all to only match remaining unhandled events:

```rust
_ => {} // MemoryReport, etc. — not handled here
```

### Step 2 — Add handle_cancelled() helper

Add a new async function `handle_cancelled()` at the bottom of the file, following the exact
same structure as `handle_completed()` and `handle_failed()`:

```rust
async fn handle_cancelled(
    db: &sqlx::SqlitePool,
    workers: &Arc<WorkerPool>,
    broadcaster: &broadcast::Sender<WsEvent>,
    notify: &Arc<Notify>,
    job_id: Uuid,
    now: DateTime<Utc>,
) {
    let job = get_job(db, job_id).await.ok().flatten();
    let Some(job) = job else { return };
    if !matches!(job.status, JobStatus::Running) {
        tracing::debug!(job_id = %job_id, status = ?job.status, "cancelled: job already terminal, ignoring");
        return;
    }

    let _ = update_status(
        db,
        job_id,
        JobStatus::Cancelled,
        None,
        None,
        None,
        None,
    )
    .await;
    tracing::info!(job_id = %job_id, "job cancelled");

    if let Some(ref wid) = job.worker_id {
        workers.set_idle(wid).await;
    }

    let _ = broadcaster.send(WsEvent::JobCancelled(JobCancelledEvent {
        event: "job.cancelled".to_string(),
        timestamp: now,
        job_id,
    }));

    notify.notify_one();
}
```

### Step 3 — Add import for JobCancelledEvent

Add `JobCancelledEvent` to the existing import on line 12:

```rust
use anvilml_core::types::events::{
    JobCancelledEvent, JobCompletedEvent, JobFailedEvent, JobImageReadyEvent,
    JobProgressEvent, JobQueuedEvent, JobStartedEvent, WsEvent,
};
```

### Step 4 — Add unit tests

Add two new tests in the `#[cfg(test)] mod tests` block:

**Test 1 — `test_progress_broadcasts_event`**
- Create a scheduler with a broadcaster + receiver pair.
- Start the dispatch loop.
- Submit a valid job (triggers dispatch → Running).
- Inject `WorkerEvent::Progress` via `pool.publish_event()`.
- Drain preceding events (`JobQueued`, `JobStarted`) from the receiver.
- Assert the next event is `WsEvent::JobProgress` with matching `job_id`, `node_index=3`,
  `node_total=5`, `node_type="Encode"`, and `step=None`, `step_total=None`.
- Abort the dispatch loop.

**Test 2 — `test_cancel_broadcasts_event`**
- Create a scheduler with a broadcaster + receiver pair.
- Start the dispatch loop.
- Submit a valid job (triggers dispatch → Running).
- Inject `WorkerEvent::Cancelled` via `pool.publish_event()`.
- Drain preceding events.
- Assert the next event is `WsEvent::JobCancelled` with matching `job_id`.
- Verify DB status is `Cancelled` and worker is back to `Idle`.
- Abort the dispatch loop.

### Step 5 — Version bump

Bump `crates/anvilml-scheduler/Cargo.toml` patch version from `0.1.15` to `0.1.16`.

### Step 6 — Logging audit (§11.5)

The dispatch loop already logs received worker events at DEBUG level (line 177). The new
Progress handler adds a DEBUG log for the broadcast. The new Cancelled handler adds an INFO
log for the cancellation event. Both comply with mandatory DEBUG log points for IPC events
(§11.5: "each event received from a worker").

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add Progress & Cancelled arms in dispatch loop; add `handle_cancelled()` helper; add two unit tests; update catch-all comment |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.15 → 0.1.16` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-scheduler/src/scheduler.rs` (mod tests) | `test_progress_broadcasts_event` | `WorkerEvent::Progress` → `WsEvent::JobProgress` broadcast with correct fields; `step`/`step_total` are `None` in MVP |
| `crates/anvilml-scheduler/src/scheduler.rs` (mod tests) | `test_cancel_broadcasts_event` | `WorkerEvent::Cancelled` → `WsEvent::JobCancelled` broadcast; DB status → Cancelled; worker → Idle |

No new test files — both tests are added as functions inside the existing `#[cfg(test)] mod tests`
block in `scheduler.rs`, following the established pattern.

## CI Impact

No CI workflow changes required. The task only modifies source code and tests within the
existing `anvilml-scheduler` crate. The existing CI gates (format, clippy, test, cross-check)
will cover the new code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Existing tests fail due to extra broadcast events consumed by receiver | Low | Medium | Tests that use `make_scheduler(pool)` discard the receiver (`_rx`), so no test is listening. The existing `test_submit_broadcasts_event` test creates its own broadcaster+receiver pair and checks only `JobQueued` — unaffected. New tests drain preceding events before asserting. |
| `WorkerEvent::Cancelled` field names don't match IPC definition | Low | Medium | Verified: `WorkerEvent::Cancelled { job_id, reason: _ }` matches the IPC enum definition in `anvilml-ipc/src/messages.rs`. |
| `handle_cancelled()` conflicts with existing `handle_failed()` pattern | Low | Low | Follows exact same structure: re-read job status, check Running, update_status to terminal, set_idle, broadcast, notify. |
| Version bump causes workspace build issue | Very Low | Low | Only patch version incremented; no dependency changes. |

## Acceptance Criteria

- [ ] `WorkerEvent::Progress` arm added in dispatch loop match block, broadcasting
      `WsEvent::JobProgress` with correct fields and `step`/`step_total` = `None`
- [ ] `WorkerEvent::Cancelled` arm added, calling `handle_cancelled()` helper
- [ ] `handle_cancelled()` function implemented following existing handler patterns
- [ ] `JobCancelledEvent` added to imports from `anvilml_core::types::events`
- [ ] Two new unit tests (`test_progress_broadcasts_event`, `test_cancel_broadcasts_event`)
      added and passing
- [ ] All five job-lifecycle events confirmed wired: `JobQueued` (submit), `JobStarted`
      (dispatch loop), `JobProgress` (new), `JobImageReady` (handler), `JobCompleted`
      (handler)
- [ ] `anvilml-scheduler` version bumped to `0.1.16`
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
