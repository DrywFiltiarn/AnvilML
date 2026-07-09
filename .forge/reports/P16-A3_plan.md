# Plan Report: P16-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-A3                                       |
| Phase       | 16 — Live Events                              |
| Description | anvilml-scheduler: event_loop restores Idle + wakes dispatch loop |
| Depends on  | P16-A2                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-07-09T11:15:00Z                         |
| Attempt     | 1                                            |

## Objective

Close a real starvation gap in `anvilml-scheduler`: after this task, every terminal `WorkerEvent` (`Completed`/`Failed`/`Cancelled`) causes the responsible worker's status to transition from `Busy` back to `Idle` via `WorkerHandle::set_status(WorkerStatus::Idle)`, and the dispatch loop is woken via `dispatch_notify.notify_one()`. Without this, a worker that finishes a job stays `Busy` forever (P14-A5 marks it `Busy` on dispatch but nothing reverses that), and queued jobs waiting for a free worker would never be re-evaluated since `dispatch_notify` is otherwise woken only by `submit()` — a starvation bug under any load beyond one job at a time.

## Scope

### In Scope
- Modify `crates/anvilml-scheduler/src/event_loop.rs`:
  - Add `Arc<anvilml_worker::WorkerPool>` parameter to `spawn_event_loop()`.
  - In each of the three terminal event arms (`Completed`, `Failed`, `Cancelled`), after VRAM release and status persistence but before publishing the `WsEvent`: find the worker handle by matching `job.worker_id` against `workers.handles()`, call `handle.set_status(WorkerStatus::Idle)`, then call `scheduler.wake_dispatch()`.
- Add `pub(crate) fn wake_dispatch(&self)` to `JobScheduler` in `scheduler.rs` — calls `self.dispatch_notify.notify_one()` and increments an internal counter.
- Add a `dispatch_wake_count: Arc<std::sync::atomic::AtomicUsize>` field to `JobScheduler` for test observability.
- Add >= 5 new tests to `crates/anvilml-scheduler/tests/event_loop_tests.rs`:
  - One per terminal event verifying worker Idle restoration + dispatch wake count increment.
  - One integration test verifying a queued second job is dispatched after the first completes with no new submission.
  - One test verifying no spurious wake (dispatch wake count unchanged for non-terminal events).
- Bump `anvilml-scheduler` patch version from `0.1.22` to `0.1.23`.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. The task's `context` mentions "confirm at ACT time" the `set_status` API — that means verify the API exists at ACT time and implement using the confirmed API, not skip.

## Existing Codebase Assessment

**What already exists:** `event_loop.rs` (538 lines) already handles all three terminal events (`Completed`, `Failed`, `Cancelled`) with: job lookup, VRAM ledger release, terminal status persistence to DB, and `WsEvent` publication. `map_worker_event()` and `spawn_event_loop()` are already public. `JobScheduler::dispatch_one()` (scheduler.rs, 958 lines) already calls `handle.set_status(WorkerStatus::Idle)` on dispatch-failure rollback paths — proving the exact same call pattern is correct for terminal-event completion. The `WorkerHandle::set_status(&self, new: WorkerStatus)` async method exists and is `pub`. The `WorkerPool::handles(&self) -> &[WorkerHandle]` method exists and returns the handle list. `dispatch_notify: Arc<Notify>` exists on `JobScheduler` and is used by `start_dispatch_loop()` to wake on `submit()`.

**Established patterns:** Tests in `event_loop_tests.rs` follow a consistent pattern: create transport, create artifact store, create JobStore with migrations, create scheduler, spawn event loop, connect DEALER socket, send msgpack event, wait for broadcaster with 5s timeout, verify outcome. Tests use `#[tokio::test]`, `Arc`-wrapped shared state, and `WorkerPool::set_up_test_workers()` (test-utils-gated) for mock worker handles. Error handling uses `?` propagation and `tracing` for logging.

**Gap between design doc and current source:** The design doc (ANVILML_DESIGN.md §12.5) specifies that terminal events should restore the worker to `Idle` and wake the dispatch loop — this is exactly what P16-A3 implements. However, `spawn_event_loop()` currently has no access to a `WorkerPool` or worker handles. This gap is what the task closes: adding the pool parameter. The `dispatch_notify` is private on `JobScheduler` — a new `wake_dispatch()` method must be added to expose the notify call.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source     | Feature flags confirmed |
|--------|-------------------|-----------------|----------------|------------------------|
| crate  | anvilml-worker    | 0.1.32          | rust-docs MCP  | test-utils (dev-dep)   |
| crate  | tokio             | 1.52.3          | rust-docs MCP  | rt, sync, macros       |

No new external crates are introduced. The task uses only existing types: `WorkerPool`, `WorkerHandle`, `WorkerStatus`, `Notify`. The `anvilml-worker` crate's `test-utils` feature is already declared in `anvilml-scheduler`'s `[dev-dependencies]` (line 32 of Cargo.toml), enabling `WorkerPool::set_up_test_workers()` for tests.

## Approach

### Step 1: Add `wake_dispatch()` method and `dispatch_wake_count` to `JobScheduler` (scheduler.rs)

Add a new field to `JobScheduler`:
```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// In JobScheduler struct:
dispatch_wake_count: Arc<AtomicUsize>,
```

Update `JobScheduler::new()` to initialize it:
```rust
dispatch_wake_count: Arc::new(AtomicUsize::new(0)),
```

Add the method:
```rust
/// Wake the dispatch loop and record the wake for test observability.
///
/// Calls `dispatch_notify.notify_one()` to wake a single waiter (the
/// dispatch loop task spawned by `start_dispatch_loop()`). The wake
/// count is incremented atomically so tests can verify the dispatch
/// loop was woken without needing to intercept the `Notify` itself.
///
/// Called from the event loop on every terminal event to ensure the
/// dispatch loop re-evaluates the queue after a worker frees up.
pub(crate) fn wake_dispatch(&self) {
    self.dispatch_wake_count.fetch_add(1, Ordering::Relaxed);
    self.dispatch_notify.notify_one();
}
```

Add a test accessor:
```rust
#[cfg(feature = "test-util")]
pub async fn dispatch_wake_count_test(&self) -> usize {
    self.dispatch_wake_count.load(Ordering::Relaxed)
}
```

**Rationale:** The `dispatch_notify` is `Arc<Notify>` — you cannot call `notify_one()` on an `Arc<Notify>` directly (only on the inner `Notify`). Wrapping the call in a `JobScheduler` method is the cleanest way to expose it. The atomic counter provides test observability without requiring tests to intercept the `Notify` or spawn additional tasks.

### Step 2: Add `Arc<WorkerPool>` parameter to `spawn_event_loop()` (event_loop.rs)

Change the function signature from:
```rust
pub fn spawn_event_loop(
    scheduler: Arc<JobScheduler>,
    transport: Arc<RouterTransport>,
    broadcaster: Arc<EventBroadcaster>,
) -> JoinHandle<()>
```

To:
```rust
pub fn spawn_event_loop(
    scheduler: Arc<JobScheduler>,
    transport: Arc<RouterTransport>,
    broadcaster: Arc<EventBroadcaster>,
    workers: Arc<anvilml_worker::WorkerPool>,
) -> JoinHandle<()>
```

### Step 3: Add worker Idle restoration + dispatch wake to each terminal event arm (event_loop.rs)

In the `Completed` arm (around line 327), after VRAM release and status persistence, add before the `broadcaster.publish()` call:

```rust
// Restore the worker to Idle and wake the dispatch loop.
// This is the scope P16-A2 deferred — P14-A5 marks the worker Busy
// on dispatch but nothing reversed that until this task. Without this,
// a worker that finishes a job stays Busy forever, and queued jobs
// would never be re-evaluated since dispatch_notify is only woken by
// submit() (a starvation bug under multi-job load).
//
// Find the worker handle by matching job.worker_id against the pool's
// handles — the worker_id is the bare device index as a string (e.g.
// "0"), matching the convention established in ANVILML_DESIGN.md §12.5.
let worker_id = job.worker_id.clone().unwrap_or_default();
if let Some(handle) = workers.handles().iter().find(|h| h.worker_id == worker_id) {
    handle.set_status(WorkerStatus::Idle).await;
    tracing::debug!(worker_id = %worker_id, "event_loop_worker_restored_idle");
} else {
    tracing::warn!(
        worker_id = %worker_id,
        "event_loop: no worker handle found for Completed — status not restored"
    );
}
scheduler.wake_dispatch();
```

Apply the same pattern to the `Failed` arm (around line 385) and `Cancelled` arm (around line 440), replacing the job lookup variable name as needed. The `worker_id` is already extracted in each arm as `let worker_id = job.worker_id.clone().unwrap_or_default();` — reuse that binding.

**Rationale:** The worker ID lookup uses `find()` on the handles slice — this is O(n) where n is the number of workers (typically 1-8), so performance is negligible. Using `find()` rather than a HashMap is consistent with the existing pattern in `dispatch_one()` (line 527-531 of scheduler.rs), which iterates all handles to collect idle workers.

### Step 4: Update `lib.rs` re-export if needed

No changes needed — `spawn_event_loop` is already re-exported. The signature change is internal to the module; callers pass an additional argument but the re-export name stays the same.

### Step 5: Version bump

Increment `anvilml-scheduler` version in `Cargo.toml` from `0.1.22` to `0.1.23`.

### Step 6: Add tests to event_loop_tests.rs

Add 5 new tests (see Tests section below). Each follows the existing pattern: create transport, artifact store, JobStore, scheduler with mock workers, spawn event loop, send event, verify outcome.

## Public API Surface

| Item | Location | Change |
|------|----------|--------|
| `spawn_event_loop()` signature | `event_loop.rs` | Adds 4th parameter: `workers: Arc<anvilml_worker::WorkerPool>` |
| `JobScheduler::wake_dispatch()` | `scheduler.rs` | New `pub(crate) fn` — calls `dispatch_notify.notify_one()` + increments counter |
| `JobScheduler::dispatch_wake_count_test()` | `scheduler.rs` | New `#[cfg(feature = "test-util")] pub async fn` — returns wake count |
| `JobScheduler::dispatch_wake_count` field | `scheduler.rs` | New field: `Arc<AtomicUsize>` |

No changes to any `pub` item's existing signature or behavior. The only signature change is `spawn_event_loop()` gaining a parameter, which is additive (no breaking change for callers that already construct a `WorkerPool`).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Add `workers` param to `spawn_event_loop()`, add Idle restoration + dispatch wake to 3 terminal event arms |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `dispatch_wake_count` field, `wake_dispatch()` method, `dispatch_wake_count_test()` accessor |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump version `0.1.22` → `0.1.23` |
| Modify | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | Add >= 5 new tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| event_loop_tests.rs | test_completed_restores_worker_idle_wakes_dispatch (mock) | `Completed` event restores worker to Idle and increments dispatch wake count | WorkerPool with 1 mock handle at Busy status; job persisted with worker_id="0" | Completed event for that job | Handle status == Idle; wake count >= 1 | `cargo test -p anvilml-scheduler --test event_loop_tests test_completed_restores_worker_idle_wakes_dispatch` |
| event_loop_tests.rs | test_failed_restores_worker_idle_wakes_dispatch (mock) | `Failed` event restores worker to Idle and increments dispatch wake count | WorkerPool with 1 mock handle at Busy status; job persisted with worker_id="0" | Failed event for that job | Handle status == Idle; wake count >= 1 | Same command as above |
| event_loop_tests.rs | test_cancelled_restores_worker_idle_wakes_dispatch (mock) | `Cancelled` event restores worker to Idle and increments dispatch wake count | WorkerPool with 1 mock handle at Busy status; job persisted with worker_id="0" | Cancelled event for that job | Handle status == Idle; wake count >= 1 | Same command as above |
| event_loop_tests.rs | test_queued_job_dispatched_after_first_completes (mock) | A second queued job is dispatched after the first job's terminal event frees the worker, with no new submission | WorkerPool with 1 mock handle; two jobs in queue; first job's terminal event sent | Completed event for first job, then check if second job was dispatched | Second job's status transitions to Running (verified via DB); dispatch wake count incremented | Same command as above |
| event_loop_tests.rs | test_progress_does_not_wake_dispatch (mock) | Non-terminal `Progress` event does NOT increment dispatch wake count | Scheduler created; event loop spawned | Progress event | Wake count unchanged (0) | Same command as above |

**Dual-mode parity markers:** N/A. This task modifies Rust scheduler code (`event_loop.rs`, `scheduler.rs`), not Python worker node code. The `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` convention (ANVILML_DESIGN.md §10.6) applies only to Python node `execute()`, arch module `load()`/`sample()`/`decode()`/`compute_latent_shape()` methods. No markers are needed.

## CI Impact

No CI changes required. The task only adds tests to the existing `event_loop_tests.rs` test file, which is already picked up by `cargo test --workspace --features mock-hardware`. The `test-util` feature is already declared in `[dev-dependencies]` of `anvilml-scheduler/Cargo.toml`, so no manifest changes affect CI.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `WorkerPool`, `WorkerHandle`, and `Notify` types are cross-platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The atomic counter uses `Ordering::Relaxed` which is correct for this use case (test observability only, no synchronization with other threads).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::handles()` returns an empty slice in tests because `set_up_test_workers()` requires `GpuDevice` pairs | Medium | Medium | Create minimal `GpuDevice` structs with `index` matching the worker_id string. The devices list is not used by the event loop — only `handles()` is needed. Test pattern: `(handle, GpuDevice { index: 0, name: "test".into(), ... })`. |
| `dispatch_notify.notify_one()` fires before `start_dispatch_loop()` has called `notified()`, losing the wake | Low | Low | This is an existing pattern — `submit()` does the same thing and it works. `Notify`'s design guarantees that `notify_one()` before `notified()` is not lost: the next `notified()` call will resolve immediately. The dispatch loop is started before any events arrive in production. |
| Worker handle lookup fails (worker_id not found in handles) because the event loop's transport and the pool's transport are different in tests | Medium | Medium | In tests, the event loop uses a direct ROUTER socket (not the pool's transport), so the event loop doesn't receive events from the pool's workers. Instead, the test manually sends events via a DEALER connected to the event loop's transport. The worker handle lookup is by `worker_id` string match — it doesn't depend on the transport. The mock handle's `worker_id` is set to "0" to match the job's `worker_id`. |
| The `wake_dispatch()` counter increments even when the dispatch loop isn't running (e.g., in tests where `start_dispatch_loop()` isn't spawned) | Low | Low | The counter is test observability only. Tests verify the count incremented, which confirms the event loop called `wake_dispatch()` — whether the dispatch loop actually runs is a separate concern tested by the integration test. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests` exits 0 with >= 20 total tests (17 existing + >= 5 new)
- [ ] Each terminal event (Completed, Failed, Cancelled) restores the worker to Idle — verified by checking `handle.status().await == WorkerStatus::Idle` after the event is processed
- [ ] Each terminal event increments the dispatch wake count — verified by checking `scheduler.dispatch_wake_count_test().await >= 1` after the event is processed
- [ ] A queued second job is dispatched after the first job's terminal event frees the worker, with no new `submit()` call — verified by checking the second job's DB status is `Running` after the first completes
- [ ] Non-terminal Progress event does NOT increment the dispatch wake count — verified by checking `scheduler.dispatch_wake_count_test().await == 0` after a Progress event
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
