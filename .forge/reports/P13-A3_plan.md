# Plan Report: P13-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P13-A3                                              |
| Phase       | 013 — Dispatch & Execute                            |
| Description | anvilml-scheduler: dispatch loop (Queued -> Execute on idle worker) |
| Depends on  | P13-A1, P13-A2                                      |
| Project     | anvilml                                             |
| Planned at  | 2026-06-09T08:15:00Z                                |
| Attempt     | 1                                                   |

## Objective

Add the background dispatch loop to `JobScheduler` in `scheduler.rs`. The loop runs as a `tokio::JoinHandle`, waking on a `Notify` (new job submission) or worker events (`subscribe_events`). On each wake it pops the next Queued job from `JobQueue`, calls `select_worker` to find an idle worker, and if found: updates the job status to `Running` in the database, marks the worker busy via `WorkerPool::set_busy`, broadcasts a `JobStarted` WebSocket event via the `EventBroadcaster`, and sends an `Execute` IPC message to the worker. The loop repeats until no further dispatch is possible. The `tokio::sync::Mutex` is held across all `await` points.

## Scope

### In Scope
- Add `start_dispatch_loop()` method to `JobScheduler` returning `JoinHandle<()>`.
- The dispatch loop subscribes to `WorkerPool::subscribe_events()` for worker events.
- Per-wake: `pop_next` → `select_worker` → `update_status(Running)` → `workers.set_busy` → `broadcaster.send(JobStarted)` → `workers.send(Execute{...})`.
- Repeat until no match in the current cycle; then `wait` on both `Notify` and worker events.
- Add a unit test that verifies a submitted job causes an `Execute` message to be sent to a mock worker.
- Version bump `anvilml-scheduler` patch version from `0.1.11` to `0.1.12`.

### Out of Scope
- Handling `Completed`/`Failed` events (covered by P13-A5).
- Hardware-aware dispatch (P13-A1 `VramLedger` is already in scope; dispatch uses `select_worker` which uses the ledger).
- Integration with `main.rs` startup (covered by P13-A6).
- Mock worker implementation (covered by P13-A4).

## Approach

### 1. Extend `JobScheduler` struct

Add fields to `JobScheduler`:
- `workers: Arc<WorkerPool>` — the worker pool for `set_busy`, `set_idle`, `subscribe_events`, and `send`.
- `ledger: Arc<Mutex<VramLedger>>` — the VRAM ledger for `select_worker`.
- `default_device: String` — the `gpu_selection.default_device` config value.

The `WorkerPool` and `VramLedger` will be wrapped in `Arc<Mutex<>>` so they can be accessed inside the dispatch loop which holds its own internal `tokio::sync::Mutex`.

### 2. Update `JobScheduler::new()` signature

Add `workers: Arc<WorkerPool>` and `ledger: Arc<Mutex<VramLedger>>` and `default_device: String` parameters. Update the call site in existing tests to pass these new parameters.

### 3. Implement `start_dispatch_loop()`

```rust
pub fn start_dispatch_loop(&self) -> JoinHandle<()> {
    let notify = self.notify.clone();
    let mut event_rx = self.workers.subscribe_events();
    // ... loop body
}
```

The loop body:

1. **Wait for a trigger** — use `tokio::select!` to wait on `notify.notified()` or `event_rx.recv()`. On worker event, loop back to step 1 (the event may be Ready/Progress/Completed which the dispatch loop doesn't handle yet — P13-A5 covers that).

2. **Pop next job** — `self.queue.pop_next()`. If `None`, go to step 1.

3. **Select a worker** — call `select_worker(job, &worker_infos, &ledger, &default_device)`. Worker infos come from `self.workers.list().await` (or cached snapshot).

4. **Dispatch** (if worker found):
   a. `update_status(&self.db, job.id, JobStatus::Running, Some(Utc::now()))` — persist Running + started_at.
   b. `self.workers.set_busy(&worker.worker_id, &job.id.to_string()).await` — mark worker busy.
   c. `self.broadcaster.send(WsEvent::JobStarted(JobStartedEvent { ... }))` — broadcast event.
   d. `self.workers.send(&worker.worker_id, WorkerMessage::Execute { job_id, graph, settings, device_index }).await` — send Execute IPC.
   e. Log dispatch at DEBUG level: `job_id=`, `worker_id=`.

5. **Repeat** — loop back to step 2 to try dispatching more queued jobs (multiple workers may be idle).

6. **Back to step 1** when no more jobs can be dispatched.

### 4. Add test: dispatch sends Execute to mock worker

Create a test `test_dispatch_sends_execute` in `scheduler.rs` that:

- Sets up a `JobScheduler` with a `WorkerPool` mock (using the existing `WorkerPool` but with a test-controlled worker).
- Subscribes to the worker event channel.
- Submits a job via `scheduler.submit()`.
- Waits for the dispatch loop to process it.
- Verifies that `Execute` was sent to the worker by checking the worker's message channel or by asserting the worker received the expected message.

Since the `WorkerPool::send` method routes through `ManagedWorker`, and in test mode workers are spawned as real Python subprocesses, we need a different approach: use a mock that captures the send call. However, since `WorkerPool` doesn't have a mock trait, the test will:

1. Create a `WorkerPool` with mock-hardware feature.
2. Use the pool's `subscribe_events()` to verify events flow.
3. Submit a job and then use a **separate channel** to verify dispatch happened — specifically, assert that the job's DB status transitions to `Running` and that the queue length decreases.
4. For the Execute message verification: add a **test-only hook** on `WorkerPool` (gated behind `#[cfg(test)]` or a `test-helpers` feature) that provides a `mpsc::Receiver<WorkerMessage>` for each worker, allowing the test to assert `Execute` was sent.

Actually, a simpler approach: since the dispatch loop is an async task, we can:
1. Create the scheduler with a real `WorkerPool` (mock-hardware, no real Python workers).
2. Manually inject a `Ready` event into the pool's broadcast channel to make the worker appear Idle.
3. Submit a job.
4. Wait briefly for the dispatch loop to process.
5. Check DB: job status is `Running`.
6. Check queue: length decreased.
7. Use `WorkerPool::list().await` to verify the worker is now `Busy`.

For verifying the `Execute` message was sent, we add a test-only method to `WorkerPool` or `ManagedWorker` that captures sent messages. Looking at the existing code, `ManagedWorker::send(msg)` writes to the IPC stdin channel. We can check the test by asserting the worker's status changed to Busy and the job is Running — the Execute send is an implementation detail that the test verifies indirectly.

Actually, re-reading the task spec: "submitted job causes Execute sent to mock worker". The simplest way is to use the `WorkerPool`'s event channel: after submitting, wait for the `JobStarted` broadcast event, then verify via a **test helper** that captures IPC sends. Since `ManagedWorker` uses an `mpsc::Sender` for IPC, we can add a `test_send_capture` method behind `#[cfg(test)]` that returns a `mpsc::Receiver<WorkerMessage>`.

Let me simplify: the test will verify the observable outcomes:
- Job status in DB transitions to `Running`
- Worker status transitions to `Busy`
- `JobStarted` event is broadcast
- Queue length decreases

The `Execute` message send is verified by checking that the worker's internal message channel received it (test-only hook).

### 5. Version bump

Increment `anvilml-scheduler` version from `0.1.11` to `0.1.12` in `Cargo.toml`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `start_dispatch_loop()`, extend `JobScheduler` struct, add dispatch test |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version `0.1.11 → 0.1.12` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `scheduler.rs` (mod tests) | `test_dispatch_sends_execute` | Submitted job causes dispatch loop to: transition DB status to Running, set worker Busy, broadcast JobStarted event, send Execute message to worker, decrease queue length |

## CI Impact

No CI changes required. The task adds code to an existing crate; no new CI gates, no new workflow jobs, and no changes to existing CI commands. The existing `cargo test --workspace --features mock-hardware` and `cargo clippy --workspace --features mock-hardware -- -D warnings` gates cover the new code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::list()` requires async but dispatch loop holds a sync `Mutex` | Medium | High | Use `tokio::sync::RwLock` for worker list access instead of `list()`, or pre-fetch worker list snapshot at loop start under a single lock. Alternatively, hold the dispatch `Mutex` and use `.await` only for DB writes and IPC sends — the `select_worker` call uses a snapshot of worker info obtained before acquiring the dispatch lock. |
| Worker pool events may lag behind dispatch decisions | Low | Medium | The dispatch loop re-checks worker status via `WorkerPool::list()` or `acquire_idle()` before dispatching. If a worker became busy between selection and dispatch, the `set_busy` call is idempotent and the subsequent `send` will fail (worker Busy), which is logged at DEBUG. |
| `broadcast::Receiver` drops events if dispatch loop is slow | Low | Low | Use `broadcast::channel` with sufficient capacity; lagged events are acceptable since the next `Notify` will trigger a re-check. |
| Test may be flaky due to async timing | Medium | Medium | Use `tokio::time::timeout` with generous timeout (e.g., 5s) and `assert!` on observable state (DB status, queue length, worker status). No timing-dependent assertions. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (all existing tests still pass)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] Dispatch loop is implemented in `scheduler.rs` as a `pub fn start_dispatch_loop(&self) -> JoinHandle<()>`
- [ ] Dispatch loop wakes on `Notify` (new job) OR worker events (`subscribe_events`)
- [ ] On dispatch: DB status transitions Queued→Running, worker→Busy, `JobStarted` broadcast sent, `Execute` IPC message sent
- [ ] Dispatch loop repeats until no further match in current cycle
- [ ] `anvilml-scheduler` version bumped to `0.1.12`
