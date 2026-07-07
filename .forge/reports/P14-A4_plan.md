# Plan Report: P14-A4

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P14-A4                                              |
| Phase       | 14 — Dispatch & Execute                             |
| Description | anvilml-scheduler: worker selection algorithm, real dispatch |
| Depends on  | P14-A3                                              |
| Project     | anvilml                                             |
| Planned at  | 2026-07-07T15:45:00Z                                |
| Attempt     | 1                                                   |

## Objective

Replace `dispatch_one()`'s always-false stub (introduced in P14-A3) with the real
two-step worker selection algorithm from `ANVILML_DESIGN.md §12.5`: first check if a
job's `device_preference` matches an `Idle` worker, otherwise rank all `Idle` workers
by `vram_free_mib` descending and pick the top candidate. On a successful match, reserve
VRAM via the ledger, transition the job to `Running`, persist the updated job to the
database, send `WorkerMessage::Execute` to the selected worker, and return `true`. If
no worker is `Idle`, return `false` without erroring — the job remains queued. This
completes the dispatch loop's job-side responsibilities; marking the assigned worker's
own status `Busy` is explicitly deferred to P14-A5.

## Scope

### In Scope
- Modify `crates/anvilml-scheduler/src/scheduler.rs`: replace `dispatch_one()`'s stub
  body with the full 2-step selection algorithm.
- Add `devices` field and `devices()` accessor method to
  `crates/anvilml-worker/src/pool.rs` so the scheduler can access each worker's
  `device_type` and `vram_free_mib` for the selection algorithm.
- Modify `crates/anvilml-scheduler/src/scheduler.rs`: update the dispatch loop to
  pass device info into `dispatch_one()` for selection decisions.
- Add `crates/anvilml-scheduler/tests/scheduler_tests.rs`: ≥6 new tests exercising
  device preference selection, VRAM ranking, no-idle-workers fallback, and multi-job
  dispatch.
- Bump `anvilml-worker` patch version from `0.1.6` to `0.1.7`.
- Bump `anvilml-scheduler` patch version from `0.1.12` to `0.1.13`.

### Out of Scope
- Marking the assigned worker's status `Busy` after dispatch — this is P14-A5's scope.
- Worker idle-triggered dispatch loop wake (deferred to a later task).
- VRAM release on job completion — handled by a later phase.
- Any changes to `backend/main.rs`, `anvilml-server`, or HTTP handlers.

## Existing Codebase Assessment

The scheduler module (`crates/anvilml-scheduler/`) already has a complete skeleton:
`JobScheduler` struct with `queue`, `ledger`, `job_store`, `node_registry`, and
`dispatch_notify` fields. `submit()`, `cancel()`, and `get_job()` are fully
implemented. `start_dispatch_loop()` (added in P14-A3) spawns a tokio task that
wakes on `dispatch_notify`, collects queued jobs, calls `dispatch_one()` for each,
and pushes back any un-dispatched jobs.

`dispatch_one()` is currently a stub that always returns `false`. It is private, takes
`&self`, `&Job`, and `&anvilml_worker::WorkerPool`, and is called from within the
dispatch loop task. The dispatch loop already handles the "push back un-dispatched jobs"
pattern correctly — it breaks the per-job iteration when `dispatch_one()` returns `false`,
then pushes the remaining jobs back to the queue.

The `WorkerPool` in `crates/anvilml-worker/` currently stores `Vec<WorkerHandle>` and
the shared `RouterTransport`. Each `WorkerHandle` exposes `worker_id` (pub field, a
string matching the device index like `"0"`), `status()` (async method returning
`WorkerStatus`), and `set_status()` (async method). However, `WorkerHandle` does NOT
expose `device_type` or `vram_free_mib` — these fields live on `GpuDevice`, which is
passed to `spawn_all()` but not stored in the pool after spawning. This is the gap
that the `devices()` accessor addition will close.

The `VramLedger` (in `ledger.rs`) provides `reserve(device_index, vram_mib)`,
`release(device_index, vram_mib)`, and `free_mib(device_index, total_mib)` — all
synchronous methods on `&mut self`. The ledger is already wrapped in
`tokio::sync::Mutex<VramLedger>` in `JobScheduler`, so `dispatch_one()` can call
`reserve()` while holding the mutex lock.

The `JobStore` (in `anvilml-registry/src/job_store.rs`) provides `upsert(&job)` for
persisting a `Job` — including its status field. This is the method used to transition
a job from `Queued` to `Running`.

The `WorkerMessage::Execute` variant (in `anvilml-ipc/src/messages.rs`) carries
`job_id`, `graph`, `settings`, and `device_index` — all of which are available in
`dispatch_one()`'s scope. The `RouterTransport::send()` method (from `anvilml-ipc`)
sends a message to a specific worker by identity string.

## Resolved Dependencies

No new external crates are introduced by this task. All dependencies already exist in
the workspace manifests. The task uses only existing types and methods from:

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | anvilml-worker    | 0.1.6 (local)   | local code | mock-hardware          |
| crate  | anvilml-core      | 0.1.8 (local)   | local code | mock-hardware          |
| crate  | anvilml-ipc       | 0.1.4 (local)   | local code | mock-hardware          |
| crate  | anvilml-registry  | 0.1.5 (local)   | local code | mock-hardware          |

All version numbers are from local `Cargo.toml` files — no MCP lookup needed for
workspace-internal path dependencies.

## Approach

### Step 1: Add `devices` field and `devices()` accessor to `WorkerPool`

**File:** `crates/anvilml-worker/src/pool.rs`

Add a `devices: Vec<GpuDevice>` field to the `WorkerPool` struct, stored alongside
`handles: Vec<WorkerHandle>`. This preserves the device metadata from `spawn_all()`
so the scheduler can query it later for dispatch decisions.

In `spawn_all_impl()`, after the device loop completes, assign `self.devices = devices`
to store the device list. The `devices` parameter is already available as `&[GpuDevice]`
in `spawn_all_impl()`.

Add a public `devices(&self) -> &[GpuDevice]` accessor method. This returns a slice of
all devices in the pool, indexed by `device.index`. Each `GpuDevice` carries:
- `index: u32` — the device index (matches `WorkerHandle.worker_id` as a string)
- `device_type: DeviceType` — `"cuda"`, `"rocm"`, or `"cpu"`
- `vram_free_mib: u32` — free VRAM at time of detection

Rationale: `WorkerHandle` intentionally does not carry device metadata (it is a
lightweight lifecycle handle). The scheduler needs device-level info for selection,
so storing it separately in the pool keeps both concerns clean. This is the minimal
change to `anvilml-worker` — one new field, one new method.

### Step 2: Implement the 2-step selection algorithm in `dispatch_one()`

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

Replace the stub body of `dispatch_one(&self, job: &Job, workers: &WorkerPool)` with
the full algorithm, per `ANVILML_DESIGN.md §12.5`:

```
async fn dispatch_one(&self, job: &Job, workers: &WorkerPool) -> bool {
    // 1. Collect all idle workers with their device info.
    // 2. Step 1: device_preference match.
    //    If job.settings.device_preference is Some(id), find an Idle worker whose
    //    worker_id (device index as string) matches. Use the first match.
    // 3. Step 2: VRAM ranking.
    //    If no device_preference match (or device_preference is None), rank all
    //    Idle workers by vram_free_mib descending (from workers.devices()), pick
    //    the top candidate.
    // 4. If no Idle workers found, return false — job stays queued.
    // 5. On match: reserve VRAM via ledger, transition job to Running, persist,
    //    send WorkerMessage::Execute, return true.
}
```

The implementation details:

a) **Filter idle workers:** Iterate `workers.handles()`, call `handle.status().await`
   for each, collect those with `WorkerStatus::Idle`. This is an async iteration because
   `status()` acquires a read lock on the shared status.

b) **Step 1 — device_preference match:** If `job.settings.device_preference` is `Some(id)`,
   filter the idle workers to those whose `handle.worker_id == id`. If any match exists,
   use the first one. The `worker_id` is the bare device index as a string (e.g., `"0"`),
   and `device_preference` is expected to be the same format — this is the documented
   convention from `ANVILML_DESIGN.md §12.5`.

c) **Step 2 — VRAM ranking:** If step 1 yields no match (or `device_preference` is `None`),
   use `workers.devices()` to get device info for each idle worker. Look up the device
   by matching `device.index` to the worker's `worker_id` (parsed as `u32`). Rank by
   `device.vram_free_mib` descending and pick the top.

d) **No idle workers:** If the idle list is empty, return `false`. The job remains
   queued — this is not an error condition per the design doc.

e) **On match — four steps must happen together:**
   i. **Reserve VRAM:** Acquire `self.ledger.lock().await`, call
      `ledger.reserve(device_index, vram_mib)` where `vram_mib` is a conservative
      estimate. The design doc does not specify an exact reservation amount for this
      task — use the device's `vram_free_mib` as a placeholder (the actual reservation
      amount will be refined in later tasks based on model metadata).

   ii. **Transition to Running:** Set `job.status = JobStatus::Running` and
       `job.worker_id = Some(worker_id.clone())`. Set `job.started_at = Some(Utc::now())`.

   iii. **Persist:** Call `self.job_store.upsert(&job).await`. This writes the updated
        job (now `Running`) to the database.

   iv. **Send Execute:** Build `WorkerMessage::Execute { job_id, graph, settings,
       device_index }` and send it via `workers.transport().send(&worker_id, &msg).await`.
       The `device_index` is the matched worker's index (parsed from `worker_id`).

   v. **Return true.**

Rationale: Steps (i) through (iv) are sequenced in this order because VRAM reservation
must happen before the job is considered "in progress" — if the persist or send fails,
the VRAM reservation should ideally be rolled back. For this initial implementation,
we accept the risk of over-reservation if persist/send fails (the ledger is advisory
anyway, per its doc comment). A future task will add proper rollback on failure.

### Step 3: Update the dispatch loop to work with the new algorithm

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

No changes needed to the dispatch loop structure itself. The loop already:
1. Collects jobs from the queue (holding the lock briefly).
2. Iterates each job, calling `dispatch_one()`.
3. Breaks on `false` and pushes remaining jobs back.

The only change is that `dispatch_one()` now has real logic instead of returning `false`.
The loop's break-on-false behavior is correct: when a job cannot be dispatched (no idle
workers), remaining jobs stay queued for the next cycle.

### Step 4: Add test helper method for `dispatch_one()`

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

Add a `pub async fn dispatch_one_test(...)` method (or use the existing `test-util`
feature) that exposes `dispatch_one()` for integration tests. Since `dispatch_one()` is
private, tests in `tests/scheduler_tests.rs` (a separate crate) cannot call it directly.

The cleanest approach: add a `#[cfg(feature = "test-util")]` public method that wraps
`dispatch_one()`:

```rust
#[cfg(feature = "test-util")]
pub async fn dispatch_one_test(
    &self,
    job: &Job,
    workers: &anvilml_worker::WorkerPool,
) -> bool {
    self.dispatch_one(job, workers).await
}
```

This follows the existing pattern: the `test-util` feature (already declared in
`Cargo.toml`) gates test-only public methods.

### Step 5: Write ≥6 new tests in `scheduler_tests.rs`

**File:** `crates/anvilml-scheduler/tests/scheduler_tests.rs`

Write the following tests, each using the `mock-hardware` feature (which is already
enabled via the acceptance command `--features mock-hardware`):

**test_device_preference_wins_over_vram_ranking:**
- Setup: 2 idle workers. Worker 0 has less VRAM but matches `device_preference = Some("0")`.
  Worker 1 has more VRAM but does not match.
- Action: Submit a job with `device_preference = Some("0")`, start dispatch loop, wait.
- Verify: The job is dispatched to worker 0 (the matching one), NOT worker 1 (higher VRAM).
- Verification: Check that the job's `worker_id` is `"0"` after dispatch.

**test_vram_ranking_picks_highest_free_idle:**
- Setup: 2 idle workers with no device_preference. Worker 0 has 16384 MiB free, worker 1
  has 8192 MiB free.
- Action: Submit a job with `device_preference = None`, start dispatch loop, wait.
- Verify: The job is dispatched to worker 0 (higher VRAM).
- Verification: Check that the job's `worker_id` is `"0"`.

**test_no_idle_workers_leaves_job_queued:**
- Setup: All workers are Busy (set status to Busy before starting dispatch loop).
- Action: Submit a job, start dispatch loop, wait.
- Verify: The job remains in `Queued` status (not dispatched). The dispatch loop does
  not error or panic.
- Verification: `get_job()` returns the job with status `Queued`.

**test_multiple_queued_jobs_get_distinct_workers:**
- Setup: 2 idle workers.
- Action: Submit 2 jobs, start dispatch loop, wait for one wake cycle.
- Verify: Both jobs are dispatched to different workers (one to each).
- Verification: Check that the two jobs have different `worker_id` values.

**test_device_preference_none_falls_back_to_vram_ranking:**
- Setup: 2 idle workers with different VRAM. `device_preference = None`.
- Action: Submit a job, start dispatch loop, wait.
- Verify: The job is dispatched to the worker with the most VRAM (same as vram ranking
  test but explicitly tests the None branch of device_preference).
- Verification: Check the dispatched worker has the highest VRAM.

**test_dispatch_one_returns_false_when_no_idle_workers:**
- Setup: All workers Busy.
- Action: Call `dispatch_one_test()` directly with a queued job.
- Verify: Returns `false`. The job is NOT dispatched.
- Verification: Assert return value is `false`.

**test_dispatch_one_reserves_vram_on_match:**
- Setup: One idle worker with known VRAM.
- Action: Call `dispatch_one_test()` with a job.
- Verify: VRAM is reserved in the ledger for the matched device.
- Verification: Check ledger state (if accessible) or verify that the dispatch succeeds
  and the job transitions to Running.

**test_dispatch_one_persists_job_to_running:**
- Setup: One idle worker.
- Action: Call `dispatch_one_test()` with a queued job.
- Verify: Job status transitions from `Queued` to `Running`, `worker_id` is set,
  `started_at` is set.
- Verification: `get_job()` returns the job with `status == Running`.

This gives 8 new tests, exceeding the ≥6 minimum and bringing the total to 19 (11 existing
+ 8 new), which exceeds the ≥17 acceptance threshold.

### Step 6: Bump crate versions

- `crates/anvilml-worker/Cargo.toml`: version `0.1.6` → `0.1.7` (new `devices` field
  and `devices()` method are a new public API).
- `crates/anvilml-scheduler/Cargo.toml`: version `0.1.12` → `0.1.13` (modified
  `dispatch_one()` behavior).

### Step 7: Logging

Add `#[tracing::instrument]` to `dispatch_one()` if not already present. Add DEBUG log
calls at key decision points:
- After filtering idle workers: `tracing::debug!(idle_count = ..., "dispatch_one_idle_workers")`
- When device_preference matches: `tracing::debug!(worker_id = ..., "dispatch_one_device_preference_match")`
- When VRAM ranking selects: `tracing::debug!(worker_id = ..., vram_free_mib = ..., "dispatch_one_vram_ranking_select")`
- When no idle workers: `tracing::debug!("dispatch_one_no_idle_workers")`
- On successful dispatch: `tracing::info!(worker_id = ..., job_id = ..., "dispatched job to worker")`

### Step 8: Documentation

Add `///` doc comments to the new `devices()` method on `WorkerPool` describing what it
returns and why it exists (scheduler dispatch decisions).

## Public API Surface

### New items in `anvilml-worker`:

```rust
// In WorkerPool (crates/anvilml-worker/src/pool.rs):
impl WorkerPool {
    /// Return the device list for all workers in this pool.
    ///
    /// Each `GpuDevice` carries `device_type`, `vram_free_mib`, and `index`.
    /// The device at index `i` corresponds to the worker handle at
    /// `handles()[i]`. This is used by the scheduler's dispatch loop
    /// to select workers based on device type and VRAM availability.
    pub fn devices(&self) -> &[GpuDevice] { ... }
}
```

### New items in `anvilml-scheduler` (test-util gated):

```rust
// In JobScheduler (crates/anvilml-scheduler/src/scheduler.rs):
#[cfg(feature = "test-util")]
impl JobScheduler {
    /// Test helper: expose `dispatch_one()` for integration tests.
    ///
    /// See `dispatch_one()` for the full algorithm description.
    /// This method is only available when the `test-util` feature is enabled.
    pub async fn dispatch_one_test(
        &self,
        job: &Job,
        workers: &anvilml_worker::WorkerPool,
    ) -> bool { ... }
}
```

### Modified items:

```rust
// In JobScheduler (crates/anvilml-scheduler/src/scheduler.rs):
// dispatch_one() body changes from:
//   false  (stub)
// to the full 2-step selection algorithm (see Approach Step 2).
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/pool.rs` | Add `devices: Vec<GpuDevice>` field, populate in `spawn_all_impl()`, add `devices()` accessor |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Replace `dispatch_one()` stub with real algorithm, add `dispatch_one_test()` test helper |
| Modify | `crates/anvilml-scheduler/tests/scheduler_tests.rs` | Add ≥6 new tests (8 planned) for worker selection algorithm |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump version `0.1.6` → `0.1.7` |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump version `0.1.12` → `0.1.13` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `scheduler_tests.rs` | `test_device_preference_wins_over_vram_ranking` | device_preference match takes priority over VRAM ranking | 2 idle workers, worker 0 has less VRAM than worker 1 | Job with `device_preference = Some("0")` | Job dispatched to worker 0 (lower VRAM) | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_device_preference_wins_over_vram_ranking` exits 0 |
| `scheduler_tests.rs` | `test_vram_ranking_picks_highest_free_idle` | VRAM ranking selects the idle worker with the most free VRAM | 2 idle workers, different VRAM amounts | Job with `device_preference = None` | Job dispatched to worker with highest VRAM | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_vram_ranking_picks_highest_free_idle` exits 0 |
| `scheduler_tests.rs` | `test_no_idle_workers_leaves_job_queued` | No idle workers leaves job in Queued status without erroring | All workers set to Busy | Job with `device_preference = None` | Job remains Queued, dispatch loop alive | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_no_idle_workers_leaves_job_queued` exits 0 |
| `scheduler_tests.rs` | `test_multiple_queued_jobs_get_distinct_workers` | Multiple queued jobs dispatched to distinct workers in one wake cycle | 2 idle workers | 2 jobs submitted | Both jobs dispatched, different worker_ids | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_multiple_queued_jobs_get_distinct_workers` exits 0 |
| `scheduler_tests.rs` | `test_device_preference_none_falls_back_to_vram_ranking` | None device_preference triggers VRAM ranking path | 2 idle workers, different VRAM | Job with `device_preference = None` | Job dispatched to highest VRAM worker | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_device_preference_none_falls_back_to_vram_ranking` exits 0 |
| `scheduler_tests.rs` | `test_dispatch_one_returns_false_when_no_idle` | dispatch_one returns false when no idle workers | All workers Busy | Any queued job | `dispatch_one_test()` returns `false` | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_dispatch_one_returns_false_when_no_idle` exits 0 |
| `scheduler_tests.rs` | `test_dispatch_one_reserves_vram_on_match` | VRAM is reserved in ledger on successful dispatch | 1 idle worker | Any queued job | VRAM reserved for matched device | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_dispatch_one_reserves_vram_on_match` exits 0 |
| `scheduler_tests.rs` | `test_dispatch_one_persists_job_to_running` | Job status transitions to Running and is persisted | 1 idle worker | Any queued job | Job status is Running, worker_id set | `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests -- test_dispatch_one_persists_job_to_running` exits 0 |

Acceptance command for full suite:
```bash
cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests
# -> >=17 total tests, exits 0
```

## CI Impact

No CI changes required. The existing `cargo test --workspace --features mock-hardware`
CI job (rust-linux, rust-windows) already runs the full scheduler test suite with the
`mock-hardware` feature. No new CI gates or matrix entries are needed.

## Platform Considerations

None identified. The dispatch loop and worker selection algorithm operate on in-memory
data structures and async I/O. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are
required. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerHandle::status()` is async — iterating handles in `dispatch_one()` requires an async loop over potentially many workers. If the pool has many workers, this could hold the dispatch loop for a noticeable duration. | Low | Medium | The pool is expected to have at most a handful of workers (one per GPU). Even with 8 GPUs, 8 async status() calls complete in milliseconds. No fix needed unless the pool grows to hundreds of workers, which is outside the MVP scope. |
| `workers.devices()` returns device info from the time of `spawn_all()`, which may be stale if VRAM changes between spawn and dispatch. The design doc's "rank by vram_free_mib" uses this snapshot value. | Low | Low | Per `ANVILML_DESIGN.md §6.2`, VRAM is "refreshed on each dispatch" — but that refresh is a separate concern (handled by `DeviceDetector::refresh_vram()`). For this task, we use the snapshot value as documented. A future task will wire up VRAM refresh before dispatch. |
| VRAM reservation amount is not yet determined — the plan uses `vram_free_mib` as a placeholder. Over-reserving could prevent other jobs from being dispatched. | Medium | Medium | The ledger is advisory (per its doc comment). Over-reservation is recoverable via `release()` on job completion/failure. For this task, the exact reservation amount is a known gap that will be refined when model metadata is available. |
| `dispatch_one_test()` requires the `test-util` feature to be enabled in dev-dependencies. If the feature is not properly wired, tests cannot compile. | Low | High | The `test-util` feature already exists in `anvilml-scheduler/Cargo.toml` and is activated by the dev-dependency self-reference. Verify it works during the test-writing step. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests` exits 0 (≥17 total tests)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `dispatch_one()` returns `true` when an idle worker matches device_preference
- [ ] `dispatch_one()` returns `true` when VRAM ranking selects the top idle worker
- [ ] `dispatch_one()` returns `false` when no workers are idle (job stays queued)
- [ ] On dispatch success: job status is `Running`, VRAM is reserved, `WorkerMessage::Execute` is sent
- [ ] `anvilml-worker` version bumped to `0.1.7`
- [ ] `anvilml-scheduler` version bumped to `0.1.13`
