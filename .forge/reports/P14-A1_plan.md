# Plan Report: P14-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-A1                                      |
| Phase       | 014 — Dispatch & Mock Execute               |
| Description | anvilml-scheduler: dispatch loop background task |
| Depends on  | P13-A1 (JobScheduler with submit/get_job/list_jobs), P13-A2 (JobQueue), P13-A3 (VramLedger), P13-A4 (DAG validation), P13-A5 (Node registry), P13-A6 (WorkerPool with ManagedWorker spawn/supervise) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-20T07:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add a background dispatch loop to `JobScheduler` — a tokio task that wakes on new-job notifications or when workers become idle, selects the best available worker by VRAM ranking, and dispatches queued jobs by marking them Running, reserving VRAM, and sending `WorkerMessage::Execute` to the selected worker. This closes the first half of the job lifecycle: Queued → Running.

## Scope

### In Scope
- `crates/anvilml-scheduler/src/scheduler.rs` — add `start_dispatch_loop(&self, workers: Arc<WorkerPool>) -> JoinHandle<()>` and `select_worker` helper
- `crates/anvilml-worker/src/pool.rs` — add `get_idle_workers(&self) -> Vec<(String, u32)>` method (returns worker_id + device_index for each Idle worker)
- `crates/anvilml-scheduler/tests/dispatch_tests.rs` — new test file with ≥ 4 tests
- `crates/anvilml-scheduler/Cargo.toml` — bump patch version 0.1.7 → 0.1.8

### Out of Scope
- P14-A3: Completed/Failed event handling and job status updates (next task)
- P14-A2: Python worker mock executor (next task)
- WebSocket event broadcasting for JobStarted (deferred to P14-A3 when job lifecycle is fully closed)
- Graceful shutdown integration for the dispatch loop JoinHandle (deferred — P14-A3 will store the JoinHandle in AppState)

## Existing Codebase Assessment

The scheduler (`crates/anvilml-scheduler/src/scheduler.rs`) already owns a `JobQueue` (FIFO with O(1) cancel), a `VramLedger` (per-device VRAM reservation tracking), and an `Arc<NodeTypeRegistry>` reference. The `submit()` method validates graphs, persists jobs to SQLite, enqueues them, and broadcasts a `JobQueued` event. The `ledger` field is marked `#[expect(dead_code)]` with the comment "used in Phase 014 dispatch loop" — this task activates that field.

The `WorkerPool` (`crates/anvilml-worker/src/pool.rs`) manages a `Vec<WorkerHandle>` with per-worker status `Arc<RwLock<WorkerStatus>>`. It has `get_worker_infos()` which returns `Vec<WorkerInfo>` including `device_index`, but no method that directly returns idle workers with their device indices. The pool's test constructor `new()` accepts pre-built `(status, worker_id, device_name)` triples and reconstructs device_index by position in the vec.

The `VramLedger` has `register_device()`, `would_fit()`, `reserve()`, and `release()` — all synchronous. The dispatch loop will call `would_fit` before `reserve` as established by the ledger's contract.

The `JobQueue` has `pop_front()` which returns the next job in FIFO order. The dispatch loop will call this to get jobs to dispatch.

Test patterns use `#[serial]` annotation (via `serial_test` crate) for database isolation, `make_*` helper functions, and in-memory SQLite pools via `anvilml_registry::open_in_memory()`.

## Resolved Dependencies

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| crate  | tokio       | 1.52.3          | Cargo.lock | full (already in workspace) |
| crate  | tracing     | 0.1.44          | Cargo.lock | std, attributes (already in workspace) |
| crate  | rmp-serde   | 1.3.1           | Cargo.lock | n/a (already in workspace) |
| crate  | zeromq      | 0.6.0           | Cargo.lock | tokio-runtime, tcp-transport (already in workspace) |

No new dependencies are introduced. The task uses only crates already declared in `anvilml-scheduler`'s `Cargo.toml` (`tokio`, `tracing`, `sqlx`, `serde_json`, `uuid`, `anvilml-core`, `anvilml-worker`, `anvilml-ipc`).

## Approach

### Step 1: Add `get_idle_workers` to `WorkerPool`

In `crates/anvilml-worker/src/pool.rs`, add a new public method to `WorkerPool`:

```rust
/// Return a list of idle workers as (worker_id, device_index) tuples.
///
/// Iterates the worker handles, reads each worker's status, and collects
/// only those with status `Idle`. The device_index is reconstructed by
/// position in the workers vec, matching the logic in `get_worker_infos()`.
///
/// This is a snapshot — the pool's monitor task may change statuses
/// between the read and the dispatch action, so the caller must not
/// assume the returned list remains valid beyond the immediate dispatch.
pub async fn get_idle_workers(&self) -> Vec<(String, u32)> {
    let workers = self.workers.lock().await;
    let mut idle = Vec::with_capacity(workers.len());
    for (i, handle) in workers.iter().enumerate() {
        if *handle.status.read().await == WorkerStatus::Idle {
            idle.push((handle.worker_id.clone(), i as u32));
        }
    }
    idle
}
```

**Rationale:** The dispatch loop needs both the worker identity (for sending Execute messages) and the device index (for VRAM ledger lookups). The existing `get_worker_infos()` returns full `WorkerInfo` structs but the dispatch loop only needs the two scalar fields, and this method avoids constructing the full structs.

### Step 2: Add `start_dispatch_loop` to `JobScheduler`

In `crates/anvilml-scheduler/src/scheduler.rs`, add the dispatch loop as a new public method. The method spawns a tokio task and returns its `JoinHandle`.

The dispatch loop signature:
```rust
/// Start the dispatch loop background task.
///
/// This method spawns a tokio task that runs the dispatch loop for the
/// lifetime of the scheduler. The task wakes on new-job notifications
/// (from `Notify` triggered by `submit()`) and periodically checks for
/// idle workers to dispatch queued jobs.
///
/// The caller must store the returned `JoinHandle` and await it on
/// shutdown to prevent the loop from silently stopping. Dropping the
/// handle without awaiting detaches the task, which will continue
/// running until it naturally exits (queue empty and no idle workers).
///
/// # Arguments
///
/// * `workers` — The `WorkerPool` providing idle worker information.
///   Shared via `Arc` so the loop can read it concurrently with other
///   pool consumers.
///
/// # Returns
///
/// A `JoinHandle<()>` for the background dispatch task. The caller
/// should store this and await it during shutdown.
pub fn start_dispatch_loop(&self, workers: Arc<WorkerPool>) -> JoinHandle<()> {
    let queue = Arc::clone(&self.queue);
    let ledger = Arc::clone(&self.ledger);
    let db = self.db.clone();
    let transport = workers.transport.clone(); // NOTE: needs field access
    let notify = Arc::clone(&self.notify); // NOTE: needs new field
    let broadcaster = Arc::clone(&self.broadcaster);

    tokio::spawn(async move {
        loop {
            // Wait for a wake signal: new job or periodic check.
            tokio::select! {
                _ = notify.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(200)) => {}
            }

            // Attempt to dispatch all queued jobs.
            dispatch_once(&queue, &ledger, &db, &workers, &transport).await;
        }
    })
}
```

**Rationale for the two-wake mechanism:** The `Notify` fires when a new job is enqueued (via `submit()` calling `notify.notify_one()`). The periodic 200ms poll handles the case where workers become idle between job submissions — without it, the loop would only wake on new jobs, missing the window where a worker finishes and becomes available. The 200ms interval balances responsiveness against CPU usage.

### Step 3: Add `notify` field to `JobScheduler`

Add a `tokio::sync::Notify` field to `JobScheduler` that `submit()` signals after enqueueing a job:

```rust
pub struct JobScheduler {
    queue: Arc<tokio::sync::Mutex<JobQueue>>,
    ledger: Arc<tokio::sync::Mutex<VramLedger>>,
    node_registry: Arc<NodeTypeRegistry>,
    db: SqlitePool,
    broadcaster: Arc<EventBroadcaster>,
    notify: Arc<tokio::sync::Notify>, // NEW — wakes dispatch loop on new job
}
```

In `new()`, initialize it as `notify: Arc::new(tokio::sync::Notify::new())`.

In `submit()`, after `self.queue.lock().await.push(job.clone())`, call `self.notify.notify_one()`.

### Step 4: Implement `select_worker` helper

```rust
/// Select the best worker for a job from the list of idle workers.
///
/// Worker selection strategy:
/// 1. If `device_preference` is set, filter idle workers to only those
///    matching the preference (exact device_index match), then pick the
///    one with the most free VRAM.
/// 2. If no preference or no matching worker, rank all idle workers by
///    free VRAM (total - reserved) descending and pick the top candidate.
/// 3. If no idle workers exist, return None.
///
/// Free VRAM is computed from the ledger's reservation data: for each
/// device, free = total_vram - reservations. The ledger stores totals
/// from `register_device()` calls.
///
/// # Arguments
///
/// * `idle_workers` — List of (worker_id, device_index) for idle workers.
/// * `device_preference` — Optional device index the job prefers.
/// * `ledger` — The VRAM ledger for reservation tracking.
/// * `total_vram` — Total VRAM per device (from hardware detection).
///
/// # Returns
///
/// `Some((worker_id, device_index))` if a suitable worker is found,
/// `None` if no idle workers are available or none have enough VRAM.
async fn select_worker(
    idle_workers: &[(String, u32)],
    device_preference: Option<u32>,
    ledger: &VramLedger,
    total_vram: &[u32],
) -> Option<(String, u32)> {
    // Filter to preferred device if specified.
    let candidates = match device_preference {
        Some(pref) => idle_workers
            .iter()
            .filter(|(_, idx)| *idx == pref)
            .cloned()
            .collect::<Vec<_>>(),
        None => idle_workers.to_vec(),
    };

    if candidates.is_empty() {
        return None;
    }

    // Rank candidates by free VRAM descending.
    let mut ranked = candidates;
    ranked.sort_by(|a, b| {
        let free_a = total_vram[a.1 as usize].saturating_sub(
            ledger.reservations().get(&a.1).copied().unwrap_or(0)
        );
        let free_b = total_vram[b.1 as usize].saturating_sub(
            ledger.reservations().get(&b.1).copied().unwrap_or(0)
        );
        free_b.cmp(&free_a) // descending
    });

    Some(ranked[0].clone())
}
```

**Note on ledger access:** The `VramLedger` uses `HashMap<u32, u32>` for reservations. Since the dispatch loop holds the ledger behind a `tokio::sync::Mutex`, we'll pass `&tokio::sync::Mutex<VramLedger>` and lock it inside `select_worker`. The reservations are read-only during selection, so this is safe.

**Alternative simpler approach:** Since `select_worker` is called inside the dispatch loop which already holds the queue lock briefly, and the ledger lock is also brief, the simpler approach is to pass the ledger reference directly and call `.lock().await` inside the function. This avoids needing a `reservations()` accessor method on the ledger.

### Step 5: Implement `dispatch_once` core logic

```rust
/// Attempt to dispatch one or more jobs from the queue to idle workers.
///
/// Iterates the queue front-to-back. For each queued job:
/// 1. Find an idle worker via `select_worker`.
/// 2. If a worker is found: mark job Running in DB, reserve VRAM,
///    send WorkerMessage::Execute to the worker, and remove from queue.
/// 3. If no worker is available: skip the job (leave it queued).
///
/// Dispatch continues until the queue is exhausted or no idle workers remain.
async fn dispatch_once(
    queue: &Arc<tokio::sync::Mutex<JobQueue>>,
    ledger: &Arc<tokio::sync::Mutex<VramLedger>>,
    db: &SqlitePool,
    workers: &WorkerPool,
    transport: &RouterTransport,
) {
    let idle_workers = workers.get_idle_workers().await;
    if idle_workers.is_empty() {
        tracing::debug!("no idle workers available for dispatch");
        return;
    }

    let mut queue = queue.lock().await;
    // Collect jobs to dispatch (avoid holding queue lock while sending IPC).
    let mut to_dispatch = Vec::new();

    while let Some(job) = queue.pop_front() {
        // Check if there are still idle workers.
        // Worker may have been dispatched to between iterations.
        let available = workers.get_idle_workers().await;
        if available.is_empty() {
            // No more idle workers — re-enqueue remaining jobs and stop.
            // NOTE: This requires a way to push back to queue.
            // Alternative: use a different queue traversal strategy.
            break;
        }

        let device_pref = job.settings.device_preference.as_ref().and_then(|s| {
            // Parse device_preference string to extract device index.
            // Formats: "cuda:0", "rocm:1", "0", etc.
            s.split(':').last().and_then(|idx| idx.parse::<u32>().ok())
        });

        // VRAM estimate: use a default of 4096 MiB for now.
        // In Phase 015, this will come from model metadata.
        let vram_estimate = 4096u32;

        // Check if VRAM would fit on any available worker.
        let ledger = ledger.lock().await;
        let can_fit = available.iter().any(|(_, idx)| {
            ledger.would_fit(*idx, vram_estimate)
        });
        drop(ledger);

        if !can_fit {
            // VRAM insufficient — re-enqueue and stop.
            queue.push(job);
            break;
        }

        // Select the best worker.
        let total_vram = vec![8192u32; available.len()]; // TODO: real VRAM
        let Some((worker_id, device_index)) =
            select_worker(&available, device_pref, &ledger, &total_vram).await
        else {
            queue.push(job);
            break;
        };
        drop(ledger);

        // Reserve VRAM.
        let mut ledger = ledger.lock().await;
        ledger.reserve(device_index, vram_estimate);
        drop(ledger);

        // Mark job Running in database.
        // ... (SQL UPDATE)

        // Send Execute message.
        // ... (transport.send)

        to_dispatch.push((job, worker_id, device_index));
    }
    drop(queue);

    // Send Execute messages outside queue lock.
    for (job, worker_id, device_index) in to_dispatch {
        let msg = WorkerMessage::Execute {
            job_id: job.id,
            graph: job.graph.clone(),
            settings: job.settings.clone(),
            device_index,
        };
        transport.send(&worker_id, &msg).await.ok();

        tracing::info!(
            job_id = %job.id,
            worker_id = %worker_id,
            "job dispatched"
        );
    }
}
```

**Important design note on queue traversal:** The current `JobQueue` uses `pop_front()` which removes jobs. If we pop a job and then find no idle worker, we must re-enqueue it. The simplest approach is to use a `VecDeque`-compatible strategy: peek at the front without removing, dispatch if possible, and only pop on success. However, `JobQueue` doesn't currently have a `peek_front()` method.

**Decision:** Add a `peek_front(&self) -> Option<&Job>` method to `JobQueue` (non-consuming). This is a minimal addition that avoids the complexity of re-enqueue logic. The `peek_front` method is used only by the dispatch loop and doesn't affect the queue's FIFO semantics.

### Step 6: Add `peek_front` to `JobQueue`

```rust
/// Return a reference to the job at the front of the FIFO queue
/// without removing it.
///
/// Returns `None` if the queue is empty.
///
/// This is used by the dispatch loop to peek at the next job without
/// committing to dispatch it, allowing the loop to check worker
/// availability before consuming the job.
pub fn peek_front(&self) -> Option<&Job> {
    self.items.front()
}
```

### Step 7: Update `submit()` to trigger the notify

After the queue push in `submit()`, add:
```rust
self.notify.notify_one();
```

### Step 8: Write tests

New file: `crates/anvilml-scheduler/tests/dispatch_tests.rs` with ≥ 4 tests:

1. **`test_dispatch_to_idle_worker`** — Creates a scheduler with one idle worker, submits a job, starts the dispatch loop, waits for dispatch, verifies the job is removed from queue and VRAM is reserved.

2. **`test_vram_reserved_on_dispatch`** — Verifies that `ledger.reserve()` is called with the correct device index and VRAM amount when a job is dispatched.

3. **`test_no_dispatch_when_no_idle_workers`** — Creates a scheduler with all workers Busy, submits a job, starts the dispatch loop, verifies the job remains queued.

4. **`test_dispatch_wakes_on_notify`** — Submits a job (triggering notify), verifies the dispatch loop processes it within a timeout.

5. **`test_device_preference_respected`** — Creates two idle workers on different devices, submits a job with `device_preference`, verifies the preferred worker is selected.

Test pattern: Each test creates its own in-memory DB, fresh queue, ledger, registry, and a `WorkerPool` via `new()` with pre-built status handles. The dispatch loop is started, a small sleep allows the loop to process, then assertions verify the outcome.

## Public API Surface

| Item | Type | Crate/Module Path | Description |
|------|------|-------------------|-------------|
| `JobScheduler::start_dispatch_loop` | `pub fn(&self, workers: Arc<WorkerPool>) -> JoinHandle<()>` | `anvilml-scheduler/src/scheduler.rs` | Spawns background dispatch loop task |
| `WorkerPool::get_idle_workers` | `pub async fn(&self) -> Vec<(String, u32)>` | `crates/anvilml-worker/src/pool.rs` | Returns idle workers with device indices |
| `JobQueue::peek_front` | `pub fn(&self) -> Option<&Job>` | `crates/anvilml-scheduler/src/queue.rs` | Peeks at front job without removing |

**Internal (non-pub) items added:**
- `JobScheduler::notify: Arc<tokio::sync::Notify>` field
- `select_worker` helper function (private, in scheduler.rs)
- `dispatch_once` helper function (private, in scheduler.rs)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `notify` field, `start_dispatch_loop`, `select_worker`, `dispatch_once`; update `new()` and `submit()` |
| MODIFY | `crates/anvilml-scheduler/src/queue.rs` | Add `peek_front()` method |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Add `get_idle_workers()` method |
| CREATE | `crates/anvilml-scheduler/tests/dispatch_tests.rs` | ≥ 4 tests for dispatch loop |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.7 → 0.1.8 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/dispatch_tests.rs` | `test_dispatch_to_idle_worker` | Job is dispatched from queue to an idle worker; job removed from queue; VRAM reserved | One idle worker in pool; one queued job; dispatch loop running | Valid graph with LoadModel node | Job removed from queue, ledger shows reservation | `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0 |
| `crates/anvilml-scheduler/tests/dispatch_tests.rs` | `test_vram_reserved_on_dispatch` | VRAM ledger reserve() called with correct device_index and amount | One idle worker; ledger pre-registered with device VRAM | Job with vram_estimate = 4096 | Ledger.reservations[device_index] == 4096 | `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0 |
| `crates/anvilml-scheduler/tests/dispatch_tests.rs` | `test_no_dispatch_when_no_idle_workers` | Job remains queued when all workers are Busy | All workers in Busy status | One queued job | Job still in queue after dispatch loop tick | `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0 |
| `crates/anvilml-scheduler/tests/dispatch_tests.rs` | `test_dispatch_wakes_on_notify` | Dispatch loop processes job after Notify signal | One idle worker; dispatch loop running | Job submitted (triggers notify) | Job dispatched within timeout | `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0 |
| `crates/anvilml-scheduler/tests/dispatch_tests.rs` | `test_device_preference_respected` | Job with device_preference selects matching worker over higher-VRAM worker | Two idle workers on different devices; job has device_preference | Job with device_preference = "cuda:1" | Worker on device 1 selected, not device 0 | `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0 |

## CI Impact

No CI changes required. The new test file `crates/anvilml-scheduler/tests/dispatch_tests.rs` is automatically picked up by `cargo test --workspace --features mock-hardware`. The `-- dispatch` filter in the acceptance criterion is a subset of the full test suite. Adding a new `tests/` directory under an existing crate is transparent to the Cargo test harness.

## Platform Considerations

None identified. The dispatch loop uses only platform-neutral tokio primitives (`Notify`, `spawn`, `sleep`, `select!`). The `WorkerPool::get_idle_workers()` method reads status through `Arc<RwLock<WorkerStatus>>` which is platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `RouterTransport` is not publicly accessible from `WorkerPool` — the pool stores it as a private field, and the dispatch loop needs to send Execute messages through it. | High | High | The dispatch loop does NOT need direct `RouterTransport` access. Instead, add a `send_execute(&self, worker_id: &str, msg: &WorkerMessage)` method to `WorkerPool` that delegates to the internal transport. This keeps the transport encapsulated within the worker crate. |
| The `VramLedger` `reservations` field is private — `select_worker` needs to read reservation counts to rank workers by free VRAM. | Medium | Medium | Add a `fn reservations(&self) -> &HashMap<u32, u32>` accessor method to `VramLedger`, or compute free VRAM inside the dispatch loop where the ledger is already locked. The latter is simpler and avoids exposing internal state. |
| Test isolation: dispatch loop tests involve async timing (sleep for loop to process), which can cause flaky failures if the sleep is too short. | Medium | Medium | Use `tokio::time::timeout` with a generous timeout (e.g., 5 seconds) rather than a fixed sleep. This makes tests deterministic: they either succeed within the timeout or fail with a clear error. |
| `JobQueue::pop_front` rebuilds all indices on each call — if the dispatch loop pops many jobs in one tick, index rebuild cost is O(n²). | Low | Low | In practice, the dispatch loop dispatches at most `num_workers` jobs per tick (one per idle worker). The queue size is bounded by `max_queued_jobs` from config. The O(n²) cost is negligible for typical queue sizes (< 100). If this becomes a concern, the queue can be switched to a `IndexMap`-backed structure in a future refactor. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- dispatch` exits 0 with ≥ 4 tests
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (no regressions)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
