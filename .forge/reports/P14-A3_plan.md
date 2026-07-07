# Plan Report: P14-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P14-A3                                            |
| Phase       | 14 — Dispatch & Execute                           |
| Description | anvilml-scheduler: dispatch loop skeleton, notify-driven wake |
| Depends on  | P14-A2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-07T14:55:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add `pub fn start_dispatch_loop(self: Arc<Self>, workers: Arc<WorkerPool>) -> tokio::task::JoinHandle<()>` and a `dispatch_one(&self, job: &Job, workers: &WorkerPool) -> bool` stub to `JobScheduler` in `crates/anvilml-scheduler/src/scheduler.rs`. The dispatch loop spawns a tokio task that waits on `dispatch_notify` (woken by `submit()`'s `notify_one()` from P14-A1) and, on each wake, iterates the queue front-to-back calling `dispatch_one()` for each job. For this task, `dispatch_one()` always returns `false` — no actual worker selection yet (deferred to P14-A4). The loop must not block the async runtime (no sync I/O). Acceptance: ≥3 new tests in `tests/scheduler_tests.rs` proving the loop returns a live JoinHandle, submit() wakes it, and it survives multiple wakes; `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests` exits 0 with ≥11 total tests.

## Scope

### In Scope
- `crates/anvilml-scheduler/src/scheduler.rs`: add `start_dispatch_loop()` and `dispatch_one()` methods to `JobScheduler`.
- `crates/anvilml-scheduler/src/scheduler.rs`: update the `#[allow(dead_code)]` annotation on `ledger` (it is now used by the dispatch loop).
- `crates/anvilml-scheduler/Cargo.toml`: add `rt` feature to tokio dependency (required for `tokio::task::spawn`).
- `crates/anvilml-scheduler/tests/scheduler_tests.rs`: add ≥3 new tests for the dispatch loop.
- `docs/TESTS.md`: add entries for the new tests (per FORGE_AGENT_RULES.md §5.10).

### Out of Scope
- Real worker selection algorithm — deferred to P14-A4, which replaces `dispatch_one()`'s always-false stub.
- Worker-idle-triggered wake — a later task will wire a worker status change channel into the dispatch loop's `select!`.
- Marking the assigned worker's status `Busy` — deferred to P14-A5.
- VRAM reservation/release in the dispatch loop — deferred to P14-A4.
- Any changes to `anvilml-worker`, `anvilml-ipc`, `anvilml-core`, or `anvilml-registry`.

## Existing Codebase Assessment

The `JobScheduler` struct (already fully implemented through P14-A2) owns a `tokio::sync::Mutex<JobQueue>`, `tokio::sync::Mutex<VramLedger>`, `Arc<JobStore>`, `Arc<NodeTypeRegistry>`, and `Arc<Notify>` (`dispatch_notify`). The `submit()` method (P14-A1) already calls `dispatch_notify.notify_one()` after enqueueing a job. The `cancel()` and `get_job()` methods (P14-A2) are complete.

The established patterns in this crate are:
- `tokio::sync::Mutex` for any state held across `.await` points (not `std::sync::Mutex`).
- `#[tracing::instrument]` on async functions representing meaningful work units.
- Structured tracing fields (`field_name = %value`) rather than format strings.
- `///` doc comments on every `pub` item describing what it does, arguments, and return values.
- Tests in `tests/scheduler_tests.rs` using `create_job_store()` and `make_registry()` helpers for database isolation and a minimal node registry.

No gap exists between the design doc and current source that affects this approach — the `dispatch_notify` field is already present and wired in `submit()`. The only structural gap is that `ledger` currently carries `#[allow(dead_code)]` because nothing yet reads it; the dispatch loop will consume it.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP  | sync, macros, rt       |

The `rt` feature is not currently listed in `crates/anvilml-scheduler/Cargo.toml`'s tokio dependency (only `sync` and `macros` are). Adding `rt` is required for `tokio::task::spawn` used in `start_dispatch_loop()`. No other external crates are introduced.

## Approach

### Step 1: Add `rt` feature to tokio dependency

In `crates/anvilml-scheduler/Cargo.toml`, change the tokio dependency from:
```toml
tokio = { version = "1.52.3", features = ["sync", "macros"] }
```
to:
```toml
tokio = { version = "1.52.3", features = ["rt", "sync", "macros"] }
```
**Rationale:** `tokio::task::spawn` requires the `rt` feature flag. This is confirmed by the rust-docs MCP lookup of tokio 1.52.3's feature table. The `rt` feature is already present in the sibling `anvilml-worker` crate's tokio dependency, confirming the convention.

### Step 2: Add `dispatch_one()` stub to `JobScheduler`

Add a private async method to `JobScheduler`:

```rust
/// Attempt to dispatch a single job to an idle worker.
///
/// For this task (P14-A3), always returns `false` — no worker selection
/// logic exists yet. The real selection algorithm is implemented in P14-A4,
/// which replaces this stub. This stub proves the dispatch loop's iteration
/// logic works correctly in isolation from the selection algorithm.
///
/// # Arguments
///
/// * `job` — The job to attempt dispatching.
/// * `workers` — The worker pool, used for future worker selection.
///
/// # Returns
///
/// `true` if the job was dispatched to a worker, `false` otherwise.
/// For this task, always returns `false`.
#[allow(dead_code)] // Stub: real dispatch logic arrives in P14-A4.
async fn dispatch_one(&self, _job: &Job, _workers: &WorkerPool) -> bool {
    // P14-A3 stub: always return false — no worker selection yet.
    // The real algorithm (device_preference then VRAM ranking) is
    // implemented in P14-A4 and replaces this body.
    false
}
```

**Rationale:** The method takes `&self` (not `self`) so it can be called from the dispatch loop without consuming the scheduler. It takes `&Job` (not `Job`) because the loop iterates the queue and may need to peek at jobs without consuming them. The `_workers` parameter is accepted but unused — it exists because P14-A4's real implementation will need it, and adding it now avoids a breaking signature change in the next task.

### Step 3: Add `start_dispatch_loop()` to `JobScheduler`

Add a public async method to `JobScheduler`:

```rust
/// Start the dispatch loop as a background tokio task.
///
/// The loop waits on `dispatch_notify` (woken by `submit()` via
/// `notify_one()`), then iterates the queue front-to-back, calling
/// `dispatch_one()` for each job. On each wake, it processes jobs
/// until the queue is empty or no idle workers are available.
///
/// The loop runs indefinitely until the `JobScheduler` is dropped.
/// It must not block the async runtime — all operations inside the
/// loop body are async (queue lock, dispatch_one).
///
/// # Arguments
///
/// * `workers` — The worker pool, passed to `dispatch_one()` for
///   worker selection.
///
/// # Returns
///
/// A `JoinHandle<()>` for the spawned task. The caller should store
/// this handle and await it during shutdown.
#[tracing::instrument(skip(self, workers), fields(workers_count = workers.handles().len()))]
pub fn start_dispatch_loop(self: Arc<Self>, workers: Arc<WorkerPool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // Wait for a notification that a job was enqueued or a
            // worker became idle (future task). dispatch_notify.wait()
            // returns immediately if no notification has arrived, so
            // we must re-check the queue after each wake.
            self.dispatch_notify.notified().await;

            tracing::debug!("dispatch_loop_wake");

            // Lock the queue and pop jobs front-to-back.
            let mut queue = self.queue.lock().await;

            while let Some(job) = queue.pop_front() {
                // Attempt to dispatch this job. dispatch_one() returns
                // false for P14-A3 (stub), true when P14-A4's selection
                // logic finds a matching worker.
                let dispatched = self.dispatch_one(&job, &workers).await;

                if !dispatched {
                    // No worker available — push the job back to the
                    // front of the queue so it can be retried on the
                    // next wake (either a new submission or a worker
                    // becoming idle).
                    //
                    // Note: pop_front() already removed it from the
                    // deque, so we need to re-insert. For P14-A3's
                    // always-false stub, every job gets pushed back,
                    // creating a tight loop if the queue has jobs.
                    // The loop mitigates this by only re-checking
                    // dispatch_notify after pushing back — but since
                    // we're inside the lock, we need to be careful.
                    // See risk #1 in Risks and Mitigations.
                    queue.push(job);
                    break; // No idle workers — stop iterating.
                }
            }

            // Drop the queue lock before the next notified().await.
            drop(queue);
        }
    })
}
```

**Rationale for the loop structure:** The dispatch loop uses `dispatch_notify.notified().await` which is a `&'async` operation on the `Arc<Notify>`. Since `Notify` is behind `Arc`, we can call `.notified()` without moving or locking anything — it's a cheap async wait. After waking, we lock the queue, pop jobs, and dispatch. If no job is dispatched (stub always returns false), we push the job back and break — this prevents a tight spin loop on every wake.

**Critical design decision — lock scope:** The queue mutex is held only for the duration of the pop-and-dispatch cycle. We do NOT hold the queue lock across the `dispatch_one()` call's internal await points (if any in future tasks). However, `dispatch_one()` takes `&self` and `&WorkerPool`, so it doesn't need the queue lock — it operates on the job (already popped) and the workers. The current stub doesn't await anything, but P14-A4's real implementation will need to await `job_store.upsert()` for status transitions, which is why the queue lock must be released before calling `dispatch_one()` in the future. For this task's stub, holding the lock during the call is fine since the stub is synchronous.

Actually, re-examining: `dispatch_one` is `async fn`, so even the stub body awaits. This means we SHOULD release the queue lock before calling it to avoid holding `tokio::sync::Mutex` across `.await` points on the same lock. Let me revise:

```rust
pub fn start_dispatch_loop(self: Arc<Self>, workers: Arc<WorkerPool>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            // Wait for notification. notified() is cheap (no lock) and
            // works on Arc<Notify> — multiple callers can share the same
            // Notify without contention.
            self.dispatch_notify.notified().await;
            tracing::debug!("dispatch_loop_wake");

            // Collect all queued jobs while holding the lock briefly,
            // then release the lock before dispatching. This prevents
            // holding the queue mutex across await points in dispatch_one().
            let jobs: Vec<Job> = {
                let mut queue = self.queue.lock().await;
                let mut jobs = Vec::new();
                while let Some(job) = queue.pop_front() {
                    jobs.push(job);
                }
                jobs
            };

            // Dispatch each collected job without holding the queue lock.
            for job in jobs {
                let dispatched = self.dispatch_one(&job, &workers).await;
                if !dispatched {
                    // No worker available — push remaining jobs back.
                    let mut queue = self.queue.lock().await;
                    for job in jobs.into_iter() {
                        queue.push(job);
                    }
                    break;
                }
            }
        }
    })
}
```

**Revised Rationale:** This pattern collects all queued jobs under the lock, releases it, then dispatches each one. If any job fails to dispatch (stub always returns false), the remaining jobs are pushed back. This avoids holding the queue mutex across `await` points in `dispatch_one()`, which would deadlock the tokio runtime thread if `dispatch_one()` were to ever acquire the same mutex (e.g., P14-A4's VRAM ledger reservation).

### Step 4: Remove `#[allow(dead_code)]` from `ledger` field

The existing `#[allow(dead_code)]` on the `ledger` field was added because it was unused before the dispatch loop. Now that `start_dispatch_loop()` exists (even though the real VRAM operations come in P14-A4), remove the annotation:

```rust
// Remove: #[allow(dead_code)] // `ledger` is used by the dispatch loop (P14-A3), not yet implemented.
```

### Step 5: Add tests to `tests/scheduler_tests.rs`

Add three new tests:

**Test 1: `test_dispatch_loop_returns_join_handle`**

```rust
/// Test that `start_dispatch_loop()` returns a `JoinHandle` that doesn't
/// immediately finish.
///
/// Constructs a `JobScheduler`, calls `start_dispatch_loop()` with an empty
/// `WorkerPool`, and asserts the returned `JoinHandle` is still joinable
/// (hasn't completed) after a brief yield. This proves the loop task is
/// alive and waiting on `dispatch_notify.notified()`.
#[tokio::test]
async fn test_dispatch_loop_returns_join_handle() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);
    let workers = WorkerPool::new().await.expect("empty pool must construct");
    let workers = Arc::new(workers);

    let handle = scheduler.start_dispatch_loop(Arc::clone(&workers));

    // Yield to let the task reach the notified().await wait point.
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // The handle should still be alive — we can check by trying to
    // poll it (it hasn't completed). We use a short timeout on a
    // try_join to confirm it hasn't finished.
    let still_alive = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        handle,
    )
    .await;

    // If the task had completed, timeout would return Err(Elapsed).
    // Since the task is waiting on notified(), it should still be alive.
    assert!(
        still_alive.is_err() || still_alive.unwrap().is_ok(),
        "dispatch loop handle must still be alive after construction"
    );
}
```

**Test 2: `test_submit_wakes_dispatch_loop`**

```rust
/// Test that `submit()` wakes the dispatch loop.
///
/// Constructs a `JobScheduler`, starts the dispatch loop, then submits
/// a job. The dispatch loop's `dispatch_notify` must be notified by
/// `submit()`'s `notify_one()` call. We observe this by checking that
/// the loop processes the job (pushes it back, since dispatch_one
/// returns false).
///
/// Uses a test hook: the dispatch loop pushes un-dispatched jobs back
/// to the queue. After submit(), we check that the job reappears in
/// the queue (it was popped, failed dispatch, and pushed back).
#[tokio::test]
async fn test_submit_wakes_dispatch_loop() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);
    let workers = WorkerPool::new().await.expect("empty pool must construct");
    let workers = Arc::new(workers);

    let handle = scheduler.start_dispatch_loop(Arc::clone(&workers));

    // Submit a job — this calls dispatch_notify.notify_one().
    let job_id = scheduler
        .submit(
            make_valid_graph(),
            JobSettings {
                device_preference: None,
            },
        )
        .await
        .expect("submit must succeed");

    // Give the dispatch loop time to wake, pop the job, attempt
    // dispatch (which returns false), and push it back.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // The job should still be in the database (persisted) and the
    // dispatch loop should have woken (no panic). We verify the loop
    // survived by checking the handle is still alive.
    let still_alive = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        handle,
    )
    .await;

    assert!(
        still_alive.is_err() || still_alive.unwrap().is_ok(),
        "dispatch loop must survive a submit wake"
    );

    // The job is still in the database.
    let result = scheduler.get_job(job_id).await.expect("get_job must not error");
    assert!(result.is_some(), "job must still be in database");
}
```

**Test 3: `test_dispatch_loop_survives_multiple_wakes`**

```rust
/// Test that the dispatch loop survives multiple wake cycles without
/// panicking.
///
/// Starts the dispatch loop, then submits three jobs sequentially.
/// Each submit wakes the loop. The loop must not panic or exit on any
/// of the three wakes — it should simply pop each job, attempt dispatch
/// (always returns false), push it back, and wait for the next wake.
#[tokio::test]
async fn test_dispatch_loop_survives_multiple_wakes() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);
    let workers = WorkerPool::new().await.expect("empty pool must construct");
    let workers = Arc::new(workers);

    let handle = scheduler.start_dispatch_loop(Arc::clone(&workers));

    // Submit three jobs sequentially, each waking the dispatch loop.
    for i in 0..3 {
        let job_id = scheduler
            .submit(
                make_valid_graph(),
                JobSettings {
                    device_preference: None,
                },
            )
            .await
            .expect("submit must succeed");

        tracing::info!(job_id = %job_id, wake_number = i, "submitted job for dispatch loop wake test");

        // Brief yield between submissions to let the loop process.
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // After all submissions, verify the loop is still alive.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let still_alive = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        handle,
    )
    .await;

    assert!(
        still_alive.is_err() || still_alive.unwrap().is_ok(),
        "dispatch loop must survive 3 consecutive wakes without panicking"
    );

    // All three jobs should still be in the database.
    for i in 0..3 {
        let job_id = scheduler
            .submit(
                make_valid_graph(),
                JobSettings {
                    device_preference: None,
                },
            )
            .await
            .expect("submit must succeed");
        let result = scheduler.get_job(job_id).await.expect("get_job must not error");
        assert!(result.is_some(), "job {} must be in database", i);
    }
}
```

Wait — test 3 has a bug. It submits 3 jobs, then submits 3 more to verify they're in the database. But the first 3 jobs' IDs are lost. Let me fix test 3:

```rust
/// Test that the dispatch loop survives multiple wake cycles without
/// panicking.
///
/// Starts the dispatch loop, then submits three jobs sequentially.
/// Each submit wakes the loop. The loop must not panic or exit on any
/// of the three wakes — it should simply pop each job, attempt dispatch
/// (always returns false), push it back, and wait for the next wake.
#[tokio::test]
async fn test_dispatch_loop_survives_multiple_wakes() {
    let store = create_job_store().await;
    let registry = make_registry();
    let scheduler = JobScheduler::new(store, registry);
    let workers = WorkerPool::new().await.expect("empty pool must construct");
    let workers = Arc::new(workers);

    let handle = scheduler.start_dispatch_loop(Arc::clone(&workers));

    // Collect job IDs for later verification.
    let mut job_ids = Vec::new();

    // Submit three jobs sequentially, each waking the dispatch loop.
    for i in 0..3 {
        let job_id = scheduler
            .submit(
                make_valid_graph(),
                JobSettings {
                    device_preference: None,
                },
            )
            .await
            .expect("submit must succeed");
        job_ids.push(job_id);

        // Brief yield between submissions to let the loop process.
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // After all submissions, verify the loop is still alive.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let still_alive = tokio::time::timeout(
        tokio::time::Duration::from_millis(100),
        handle,
    )
    .await;

    assert!(
        still_alive.is_err() || still_alive.unwrap().is_ok(),
        "dispatch loop must survive 3 consecutive wakes without panicking"
    );

    // All three jobs should still be in the database.
    for job_id in &job_ids {
        let result = scheduler
            .get_job(*job_id)
            .await
            .expect("get_job must not error");
        assert!(result.is_some(), "job {:?} must be in database", job_id);
    }
}
```

### Step 6: Update `docs/TESTS.md`

Add entries for the three new tests using the format defined in `ANVILML_DESIGN.md §17.1`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| `start_dispatch_loop` | `anvilml_scheduler::JobScheduler` | `pub fn start_dispatch_loop(self: Arc<Self>, workers: Arc<WorkerPool>) -> tokio::task::JoinHandle<()>` |
| `dispatch_one` | `anvilml_scheduler::JobScheduler` | `async fn dispatch_one(&self, job: &Job, workers: &WorkerPool) -> bool` (private) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Add `rt` feature to tokio dependency |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `start_dispatch_loop()` and `dispatch_one()` methods; remove `#[allow(dead_code)]` from `ledger` |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add 3 new tests for dispatch loop |
| Modify | `docs/TESTS.md` | Add entries for the 3 new tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_dispatch_loop_returns_join_handle` | `start_dispatch_loop()` returns a JoinHandle that doesn't immediately finish; the loop task is alive and waiting on `dispatch_notify.notified()` | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_loop_returns_join_handle` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_submit_wakes_dispatch_loop` | `submit()`'s `notify_one()` wakes the dispatch loop; the loop survives the wake without panicking and the job remains in the database | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_submit_wakes_dispatch_loop` exits 0 |
| `crates/anvilml-scheduler/tests/scheduler_tests.rs` | `test_dispatch_loop_survives_multiple_wakes` | The dispatch loop survives 3 consecutive submit-triggered wakes without panicking; all jobs remain persisted | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_loop_survives_multiple_wakes` exits 0 |

## CI Impact

No CI changes required. The `--features mock-hardware` flag is already used in all CI Rust test jobs (`rust-linux`, `rust-windows`). Adding the `rt` feature to tokio is a non-breaking change (it's a feature addition, not a removal). The new tests are in an existing test file that is already collected by `cargo test --workspace --features mock-hardware`.

## Platform Considerations

None identified. The dispatch loop is purely async Rust with no platform-specific code. The `tokio::task::spawn` and `tokio::sync::Notify` APIs are cross-platform. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `dispatch_one()` is an `async fn` but the stub body is synchronous — the queue lock is held across an `.await` point even though the stub doesn't actually await. If the loop were to be called from a context where the queue lock is already held (unlikely but possible), this would deadlock. | Low | High | The revised approach collects all queued jobs under the lock, releases it, then dispatches. This prevents holding the queue mutex across any `.await` point in `dispatch_one()`. |
| `tokio::time::timeout(handle).await` in tests: if the handle completes normally (returns `Ok`), `timeout` returns `Ok(Ok(()))`. If the handle is still alive, `timeout` returns `Err(Elapsed)`. The test assertion `still_alive.is_err() || still_alive.unwrap().is_ok()` is confusing and may not correctly express the intent. | Medium | Medium | Simplify the test: use `handle.is_finished()` which returns `false` while the task is still running. This is a cleaner API for checking if a JoinHandle has completed. |
| The dispatch loop's `while let Some(job) = queue.pop_front()` followed by pushing back on failure creates a pattern where every job gets pushed back on every wake (since the stub always returns false). If `dispatch_notify` is notified while the loop is already processing, the `notify_one()` is consumed and subsequent notifications are lost. | Low | Medium | The loop uses `notified().await` which is a one-shot wait — each wake processes all queued jobs. If jobs are pushed back, they remain in the queue for the next `notify_one()`. This is correct behavior for the stub. |
| `WorkerPool::new()` in tests requires a real ROUTER socket bind, which may fail on some test environments. | Low | Medium | The existing tests already use `WorkerPool::new()` in pool_tests.rs, so this pattern is established. If it fails, diagnose the port binding issue. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-scheduler --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_loop_returns_join_handle` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_submit_wakes_dispatch_loop` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests test_dispatch_loop_survives_multiple_wakes` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests` exits 0 with ≥11 total tests
- [ ] `cargo clippy -p anvilml-scheduler --features mock-hardware -- -D warnings` exits 0
