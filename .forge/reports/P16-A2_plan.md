# Plan Report: P16-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-A2                                      |
| Phase       | 16 — Live Events                              |
| Description | anvilml-scheduler: event_loop updates Job status in JobStore on events |
| Depends on  | P16-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-09T10:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Close the job-status-persistence gap that has existed since Phase 14: the first time anything in the project sets a job's status to a terminal state (`Completed`, `Failed`, or `Cancelled`) based on the worker's actual `WorkerEvent` response, and releases the VRAM ledger reservation on every terminal event. After this task, `GET /v1/jobs/:id` (Phase 14) will observe terminal states correctly in the database, and the Runnable Proof for Phase 14 will be functionally correct rather than coincidentally passing.

## Scope

### In Scope
- **`crates/anvilml-scheduler/src/event_loop.rs`**: Add status-persistence and VRAM-release logic for `WorkerEvent::Completed`, `WorkerEvent::Failed`, and `WorkerEvent::Cancelled` in the `spawn_event_loop()` match arms, alongside the existing `ImageReady` path.
- **`crates/anvilml-scheduler/src/scheduler.rs`**: Add `release_reservation()` method on `JobScheduler` (acquires `ledger` mutex internally), `ledger_reservations()` test-only accessor, and expose the `ledger` field as `pub(crate)` for the event loop to access.
- **`crates/anvilml-scheduler/src/lib.rs`**: Remove `interim_job_completion` module declaration and re-export (`pub mod interim_job_completion;` and `pub use interim_job_completion::spawn_interim_job_completion_listener;`).
- **`crates/anvilml-scheduler/tests/event_loop_tests.rs`**: Add ≥5 new integration tests covering terminal event status persistence and VRAM release.
- **`crates/anvilml-scheduler/Cargo.toml`**: Bump patch version from `0.1.21` to `0.1.22`.
- **`crates/anvilml-scheduler/src/interim_job_completion.rs`**: Delete this file (the interim stopgap module is replaced by the real event-loop persistence).

### Out of Scope
- Restoring the worker's own status to `Idle` on terminal events — deferred to **P16-A3** (which calls `worker_handle.set_status(WorkerStatus::Idle)` and `dispatch_notify.notify_one()`).
- Waking the dispatch loop (`dispatch_notify.notify_one()`) on terminal events — deferred to **P16-A3**.
- Removing the interim-patch wiring in `crates/anvilml-worker/src/managed.rs` (`job_completion_tx` field and `set_job_completion_tx()` setter), `crates/anvilml-worker/src/pool.rs`, and `backend/src/main.rs` — these are P16-B1's responsibility per TASKS_PHASE016.md's interim-patch removal checklist.

## Existing Codebase Assessment

The `event_loop.rs` module (created in Phase 15, P15-C1) already has a complete `spawn_event_loop()` function that receives `WorkerEvent`s from a `RouterTransport`, routes `ImageReady` through artifact save logic, and publishes all other events via `map_worker_event()` + `EventBroadcaster::publish()`. The `map_worker_event()` function already maps all terminal events (`Completed`, `Failed`, `Cancelled`) to their `WsEvent` counterparts — the mapping logic exists but the status persistence side-effect does not.

The `JobScheduler` struct (in `scheduler.rs`) already owns a `Mutex<VramLedger>` and a `JobStore` (wrapped in `Arc`). The dispatch loop (`dispatch_one()`) already reserves VRAM via `ledger.reserve()` and persists job state transitions (`Queued` → `Running`) via `job_store.upsert()`. The `VramLedger::release()` method exists and uses saturating subtraction — it is fully functional.

The `JobStore` (in `anvilml-registry/src/job_store.rs`) has `upsert()` and `get()` methods but no dedicated `update_terminal_status()` method. The interim patch (`interim_job_completion.rs`) implements terminal status persistence by calling `job_store.get()`, mutating the returned `Job`, and calling `job_store.upsert()` — this is the pattern to replicate in the event loop.

An interim stopgap (`interim_job_completion.rs`) currently handles terminal status persistence via an unbounded `mpsc` channel populated by `ManagedWorker`. This task replaces that stopgap with the real `EventBroadcaster`-subscribed event loop path.

## Resolved Dependencies

| Type   | Name      | Version verified | MCP source   | Feature flags confirmed |
|--------|-----------|-----------------|--------------|------------------------|
| crate  | zeromq    | 0.6.0           | rust-docs MCP| tokio-runtime, all-transport |
| crate  | rmp-serde | 1.3.1           | rust-docs MCP| (none needed)           |

No new external dependencies are introduced. All types and methods referenced exist in the resolved versions.

## Approach

### Step 1: Add `release_reservation()` method to `JobScheduler` (scheduler.rs)

Add a `pub(crate)` method that handles VRAM ledger release for a terminal event. It acquires the `ledger` mutex internally, parses the worker_id as the device index, and calls `ledger.release(device_index, vram_mib)`.

```rust
/// Release the VRAM reservation for a job on the given device.
///
/// Called from the event loop when a terminal `WorkerEvent` (Completed/Failed/Cancelled)
/// arrives. Acquires the ledger mutex internally and calls `VramLedger::release()`.
/// The reservation amount is the same `vram_free_mib` value that was reserved at dispatch
/// time (the dispatch path uses `vram_free_mib` as a placeholder reservation per §12.4).
///
/// # Arguments
///
/// * `device_index` — The device index parsed from the worker's `worker_id`.
/// * `vram_mib` — The VRAM amount to release (the same value that was reserved at dispatch).
#[tracing::instrument(fields(device_index, vram_mib), skip(self))]
pub(crate) async fn release_reservation(&self, device_index: u32, vram_mib: u32) {
    let mut ledger = self.ledger.lock().await;
    ledger.release(device_index, vram_mib);
    tracing::debug!(device_index, vram_mib, "released VRAM reservation");
}
```

**Rationale:** The ledger is a `tokio::sync::Mutex<VramLedger>`. Wrapping the lock acquisition inside the method keeps the event loop code clean and ensures the lock is always released even if a panic occurs (the `MutexGuard` drop handler runs on unwind).

### Step 2: Add `ledger_reservations()` test accessor to `VramLedger` (ledger.rs)

Add a test-only method that returns a reference to the reservations map, so tests can verify VRAM release:

```rust
#[cfg(test)]
pub fn reservations(&self) -> &std::collections::HashMap<u32, u32> {
    &self.reservations
}
```

**Rationale:** The `tests/` directory is compiled as a separate test crate. The `#[cfg(test)]` attribute ensures this method only exists in test builds. The event_loop_tests.rs already depends on `anvilml-scheduler` with `test-util` feature, so this is accessible.

### Step 3: Modify `spawn_event_loop()` match arms for terminal events (event_loop.rs)

In the `spawn_event_loop()` function, replace the `_` catch-all arm with explicit arms for each terminal event type. Each arm:
1. Extracts `job_id` and (for `Failed`) `error` from the event.
2. Parses `worker_id` as `device_index` (u32).
3. Calls `scheduler.release_reservation(device_index, vram_mib)` to release VRAM.
4. Calls `update_job_terminal_status()` (new helper) to persist the status change.
5. Publishes the mapped `WsEvent` via the broadcaster.

The helper function `update_job_terminal_status()` on `JobScheduler` handles the database persistence:

```rust
/// Update a job's terminal status in the database.
///
/// Fetches the current job row, mutates its status/completed_at/error fields,
/// and persists via `upsert()`. Used by the event loop when a terminal
/// `WorkerEvent` arrives.
///
/// # Arguments
///
/// * `job_id` — The job to update.
/// * `status` — The terminal `JobStatus` to set.
/// * `error` — Optional error string (used for `Failed` events).
#[tracing::instrument(fields(job_id, ?status), skip(self))]
pub async fn update_job_terminal_status(&self, job_id: Uuid, status: JobStatus, error: Option<String>) {
    match self.job_store.get(job_id).await {
        Ok(Some(mut job)) => {
            job.status = status;
            job.completed_at = Some(Utc::now());
            job.error = error;
            if let Err(e) = self.job_store.upsert(&job).await {
                tracing::error!(
                    job_id = %job_id,
                    error = %e,
                    status = ?status,
                    "event_loop: failed to persist terminal status"
                );
            } else {
                tracing::info!(
                    job_id = %job_id,
                    status = ?status,
                    "event_loop: persisted terminal status"
                );
            }
        }
        Ok(None) => {
            tracing::warn!(
                job_id = %job_id,
                "event_loop: received terminal event for unknown job"
            );
        }
        Err(e) => {
            tracing::error!(
                job_id = %job_id,
                error = %e,
                "event_loop: failed to fetch job for terminal status update"
            );
        }
    }
}
```

The modified `spawn_event_loop()` match for terminal events (replacing the `_` arm):

```rust
WorkerEvent::Completed { job_id, elapsed_ms } => {
    // Parse worker_id from the job's stored worker_id field.
    // The job was assigned a worker_id during dispatch_one().
    let worker_id = self.get_job_worker_id(job_id).await;
    let device_index = worker_id.as_ref().and_then(|wid| wid.parse::<u32>().ok()).unwrap_or(0);
    let vram_mib = worker_id.as_ref().map(|wid| {
        workers.devices().get(wid.parse::<usize>().unwrap_or(0))
    }).flatten().map(|d| d.vram_free_mib).unwrap_or(0);

    // Release VRAM reservation before persisting — order doesn't matter
    // functionally (the ledger and DB are independent), but releasing
    // first matches the dispatch path's reserve-before-execute ordering.
    if vram_mib > 0 {
        self.release_reservation(device_index, vram_mib).await;
    }

    // Persist the terminal status.
    self.update_job_terminal_status(job_id, JobStatus::Completed, None).await;

    // Publish the mapped WsEvent.
    let ws_event = WsEvent::JobCompleted { job_id, elapsed_ms };
    broadcaster.publish(ws_event);
    tracing::debug!(job_id = %job_id, from = "Completed", to = "JobCompleted", "event transition");
}
```

Wait — I realize the event loop doesn't have direct access to `workers` (the `WorkerPool`). Looking at the current `spawn_event_loop()` signature:

```rust
pub fn spawn_event_loop(
    scheduler: Arc<JobScheduler>,
    transport: Arc<RouterTransport>,
    broadcaster: Arc<EventBroadcaster>,
) -> JoinHandle<()>
```

The `workers` handle is NOT passed to the event loop. The event loop only receives the `JobScheduler`, `RouterTransport`, and `EventBroadcaster`. This means I cannot access `workers.devices()` from within the event loop.

I need to rethink the VRAM release approach. Options:
1. Pass `workers` to the event loop (changes the signature).
2. Store the reservation info on the `Job` struct (changes the domain type).
3. Add a method on `JobScheduler` that looks up the reservation internally.

Looking at the current code more carefully, the `JobScheduler` doesn't store worker info per-job. The `Job` struct has `worker_id: Option<String>` but no `device_index` or `vram_reserved_mib`.

The cleanest approach: add a method on `JobScheduler` that, given a `job_id`, looks up the job's `worker_id`, resolves the device index, and releases the reservation. But this still requires access to `workers` to get `vram_free_mib`.

Alternative: Since the dispatch path reserves `vram_free_mib` (the device's free VRAM at dispatch time), and the ledger tracks reservations per device index, I can look up the current reservation amount from the ledger itself. The `VramLedger::free_mib()` method computes `total - reserved`, so if I know `total_mib` I can derive the reservation. But I don't have `total_mib` either.

Simplest correct approach: Add a `reservations()` accessor on `VramLedger` that returns the reservation map, and look up the reservation amount for the device index from there.

Let me revise the approach:

```rust
// In VramLedger:
pub fn get_reservation(&self, device_index: u32) -> u32 {
    *self.reservations.get(&device_index).unwrap_or(&0)
}
```

Then in the event loop, after parsing `worker_id` from the job:
```rust
let device_index = worker_id.parse::<u32>().unwrap_or(0);
let vram_mib = scheduler.ledger().get_reservation(device_index);
if vram_mib > 0 {
    scheduler.release_reservation(device_index, vram_mib).await;
}
```

But wait — the ledger is `Mutex<VramLedger>` inside the scheduler, not directly accessible. I need to either:
a) Add a `pub(crate) fn ledger(&self) -> &VramLedger` accessor that reads the lock.
b) Add a `get_reservation(device_index)` method on `JobScheduler` that acquires the lock and delegates.

Option (b) is cleaner:

```rust
#[tracing::instrument(fields(device_index), skip(self))]
pub(crate) async fn get_reservation(&self, device_index: u32) -> u32 {
    let ledger = self.ledger.lock().await;
    ledger.get_reservation(device_index)
}
```

Now the event loop can:
```rust
let vram_mib = self.get_reservation(device_index).await;
if vram_mib > 0 {
    self.release_reservation(device_index, vram_mib).await;
}
```

This is the cleanest approach — no new struct fields, no API changes to `spawn_event_loop()`, and the ledger access is fully encapsulated within the scheduler.

Let me now write the complete revised approach.

### Step 3 (Revised): Add ledger accessor methods to `JobScheduler` (scheduler.rs)

Add two `pub(crate)` methods:

```rust
/// Look up the current VRAM reservation for a device index.
///
/// Returns the amount currently reserved (MiB). Returns 0 if the device
/// has no reservation. Used by the event loop to determine how much
/// VRAM to release when a terminal event arrives.
#[tracing::instrument(fields(device_index), skip(self))]
pub(crate) async fn get_reservation(&self, device_index: u32) -> u32 {
    let ledger = self.ledger.lock().await;
    ledger.get_reservation(device_index)
}
```

And on `VramLedger`:
```rust
/// Get the current reservation amount for a device index.
///
/// Returns the amount currently reserved (MiB). Returns 0 if the device
/// has no reservation entry.
pub fn get_reservation(&self, device_index: u32) -> u32 {
    *self.reservations.get(&device_index).unwrap_or(&0)
}
```

### Step 4: Add `update_job_terminal_status()` method to `JobScheduler` (scheduler.rs)

Same as described above — fetches the job, mutates terminal fields, persists via `upsert()`.

### Step 5: Modify `spawn_event_loop()` match arms (event_loop.rs)

Replace the `_` catch-all arm with explicit arms for `Completed`, `Failed`, and `Cancelled`. Each arm follows this pattern:

```rust
WorkerEvent::Completed { job_id, elapsed_ms } => {
    // Look up the worker_id from the job's persisted record.
    // The worker_id was set during dispatch_one() when the job was
    // assigned to a worker and transitioned to Running.
    let job = match self.get_job(job_id).await {
        Ok(Some(job)) => job,
        Ok(None) => {
            tracing::warn!(job_id = %job_id, "event_loop: Completed event for unknown job");
            // Still publish the event — the WebSocket client should see it.
            broadcaster.publish(WsEvent::JobCompleted { job_id, elapsed_ms });
            continue;
        }
        Err(e) => {
            tracing::error!(job_id = %job_id, error = %e, "event_loop: failed to fetch job for Completed");
            broadcaster.publish(WsEvent::JobCompleted { job_id, elapsed_ms });
            continue;
        }
    };

    // Release the VRAM reservation. The worker_id encodes the device index
    // (bare device index as string, e.g. "0"). We parse it to look up
    // the reservation amount from the ledger.
    let worker_id = job.worker_id.clone().unwrap_or_default();
    let device_index = worker_id.parse::<u32>().unwrap_or(0);
    let vram_mib = self.get_reservation(device_index).await;
    if vram_mib > 0 {
        self.release_reservation(device_index, vram_mib).await;
    }

    // Persist the terminal status.
    self.update_job_terminal_status(job_id, JobStatus::Completed, None).await;

    // Publish the mapped WsEvent.
    broadcaster.publish(WsEvent::JobCompleted { job_id, elapsed_ms });
    tracing::debug!(job_id = %job_id, from = "Completed", to = "JobCompleted", "event transition");
}
```

The `Failed` arm is similar but also persists the error string:
```rust
WorkerEvent::Failed { job_id, error, traceback: _ } => {
    // ... same job lookup and VRAM release pattern ...
    self.update_job_terminal_status(job_id, JobStatus::Failed, Some(error)).await;
    broadcaster.publish(WsEvent::JobFailed { job_id, error });
    tracing::debug!(job_id = %job_id, from = "Failed", to = "JobFailed", "event transition");
}
```

The `Cancelled` arm:
```rust
WorkerEvent::Cancelled { job_id } => {
    // ... same job lookup and VRAM release pattern ...
    self.update_job_terminal_status(job_id, JobStatus::Cancelled, None).await;
    broadcaster.publish(WsEvent::JobCancelled { job_id });
    tracing::debug!(job_id = %job_id, from = "Cancelled", to = "JobCancelled", "event transition");
}
```

The remaining non-terminal events (`Progress`, and the events that should never reach this point — `Ready`, `Pong`, `Dying`, `MemoryReport`) continue through the existing `map_worker_event()` path.

**Rationale for pattern-matching explicitly instead of using the `_` arm:** The `_` arm would route terminal events through `map_worker_event()` + `broadcaster.publish()`, which only publishes the `WsEvent` without persisting status or releasing VRAM. By handling terminal events explicitly, we ensure both side-effects (DB persistence + VRAM release) happen before publishing.

### Step 6: Remove `interim_job_completion` module (lib.rs)

Delete the two lines from `lib.rs`:
```rust
pub mod interim_job_completion;
pub use interim_job_completion::spawn_interim_job_completion_listener;
```

And delete the file `crates/anvilml-scheduler/src/interim_job_completion.rs`.

**Note:** The interim-patch wiring in `backend/src/main.rs` and `crates/anvilml-worker/src/managed.rs`/`pool.rs` is P16-B1's responsibility per TASKS_PHASE016.md. This task only removes the scheduler-side module. The `backend/src/main.rs` will still compile because the `spawn_interim_job_completion_listener` import will fail — this is expected and will be fixed in P16-B1.

Wait — this would cause a compilation error. Let me reconsider.

Actually, looking at the TASKS_PHASE016.md more carefully:

> **INTERIM-P14-PATCH (manual, pre-Phase-16) is present and must be removed as part of this phase — read `docs/PHASES_GRAPH.md`'s "Interim Job-Completion Patch" section before starting `P16-A1`.**

And the Known Constraints section:

> - **Interim-patch removal checklist (do this as part of `P16-A2`/`P16-B1`):**
>   delete `job_completion_tx` and `set_job_completion_tx()` from
>   `crates/anvilml-worker/src/managed.rs` and `pool.rs`; delete
>   `crates/anvilml-scheduler/src/interim_job_completion.rs` and its `lib.rs`
>   re-export; delete the channel construction and listener spawn in
>   `backend/src/main.rs` (search for `INTERIM-P14-PATCH`).

So the checklist says P16-A2/P16-B1 together remove the patch. Since P16-A2 handles the scheduler module deletion and P16-B1 handles the server-side wiring, I should include the scheduler-side cleanup in this task. The `backend/src/main.rs` changes can be noted as a P16-B1 follow-up.

But there's a compilation issue: if I remove the `interim_job_completion` module from lib.rs but `backend/src/main.rs` still imports it, the build will fail. Since this is a PLAN session and the ACT agent will implement both tasks sequentially (P16-A2 first, then P16-B1), the plan should note that the backend/main.rs removal is deferred to P16-B1.

Actually, looking at the task context again: "Acceptance: >=5 new tests: Completed sets completed_at, Failed persists the error string, Cancelled persists correctly, all three release the ledger; `cargo test -p anvilml-scheduler --test event_loop_tests` exits 0 (>=15 total)."

The acceptance criterion only tests the scheduler crate, not the full workspace build. So it's acceptable to remove the scheduler-side interim module in this task and leave the backend/main.rs wiring for P16-B1. The ACT agent will need to handle the compilation break between the two tasks.

Hmm, but FORGE_AGENT_RULES.md §9.2 says "Build failures within the task's scope MUST be fixed before writing the report." If the build fails because backend/main.rs still references the removed module, that's a build failure.

Let me re-read the TASKS_PHASE016.md interim-patch section more carefully:

> delete `crates/anvilml-scheduler/src/interim_job_completion.rs` and its `lib.rs` re-export

This says "and its lib.rs re-export" — so both the module file AND the lib.rs re-export. But it doesn't say to also delete the backend/main.rs wiring in P16-A2's scope. The "do this as part of P16-A2/P16-B1" phrasing suggests the work is split between the two tasks.

For the plan, I'll note that the scheduler-side interim-patch removal (module deletion + lib.rs cleanup) is in scope for P16-A2, and the backend/main.rs wiring removal is deferred to P16-B1. The ACT agent should handle this by either:
1. Removing the backend/main.rs wiring in P16-A2 as well (since it's part of the same checklist), or
2. Making the `spawn_interim_job_completion_listener` import conditional.

I'll include the full interim-patch removal in P16-A2's scope since TASKS_PHASE016.md lists it under "Known Constraints and Gotchas" for Phase 16, and P16-A2 is the task that removes the scheduler-side module. The backend/main.rs changes are naturally part of the same cleanup.

Actually, let me re-read one more time. The checklist says:

> - **Interim-patch removal checklist (do this as part of `P16-A2`/`P16-B1`):**

The `/` between P16-A2 and P16-B1 suggests the work is split. P16-A2 handles the scheduler-side, P16-B1 handles the server-side. But removing just the scheduler module would break the build.

I'll include the backend/main.rs removal in P16-A2's scope since it's part of the same checklist and the build must be clean. P16-B1 will then just handle the `AppState` broadcaster wiring without the interim-patch cleanup.

Let me finalize the plan now.

## Public API Surface

| Item | Crate/Module | Signature | Description |
|------|-------------|-----------|-------------|
| `JobScheduler::update_job_terminal_status` | `anvilml-scheduler/src/scheduler.rs` | `pub async fn update_job_terminal_status(&self, job_id: Uuid, status: JobStatus, error: Option<String>)` | Updates a job's terminal status in the database (status, completed_at, error). Called by the event loop on Completed/Failed/Cancelled events. |
| `JobScheduler::release_reservation` | `anvilml-scheduler/src/scheduler.rs` | `pub(crate) async fn release_reservation(&self, device_index: u32, vram_mib: u32)` | Releases a VRAM reservation from the ledger. Called by the event loop on all three terminal events. |
| `JobScheduler::get_reservation` | `anvilml-scheduler/src/scheduler.rs` | `pub(crate) async fn get_reservation(&self, device_index: u32) -> u32` | Returns the current VRAM reservation amount for a device index. Used by the event loop to determine how much to release. |
| `VramLedger::get_reservation` | `anvilml-scheduler/src/ledger.rs` | `pub fn get_reservation(&self, device_index: u32) -> u32` | Returns the reservation amount for a device index from the internal HashMap. |
| `VramLedger::reservations` | `anvilml-scheduler/src/ledger.rs` | `#[cfg(test)] pub fn reservations(&self) -> &HashMap<u32, u32>` | Test-only accessor returning the reservations map for verification in tests. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Add terminal event arms (Completed/Failed/Cancelled) with status persistence and VRAM release; remove `_` catch-all for these events. |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `update_job_terminal_status()`, `get_reservation()`, `release_reservation()` methods. |
| Modify | `crates/anvilml-scheduler/src/ledger.rs` | Add `get_reservation()` and `#[cfg(test)]` `reservations()` accessor methods. |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Remove `interim_job_completion` module declaration and re-export. |
| Delete | `crates/anvilml-scheduler/src/interim_job_completion.rs` | Remove the interim stopgap module (replaced by real event-loop persistence). |
| Modify | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | Add ≥5 new integration tests for terminal event handling. |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.21 → 0.1.22. |
| Modify | `backend/src/main.rs` | Remove interim-patch wiring: `spawn_interim_job_completion_listener` import, channel construction, and listener spawn. |
| Modify | `crates/anvilml-worker/src/managed.rs` | Remove `job_completion_tx` field and `set_job_completion_tx()` setter. |
| Modify | `crates/anvilml-worker/src/pool.rs` | Remove `job_completion_tx` field and wiring. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_completed_persists_status_and_releases_ledger` | `WorkerEvent::Completed` sets `Job.status=Completed`, `completed_at=now` in JobStore, and releases the VRAM reservation in the ledger | In-memory JobStore with a Running job; ledger has a reservation for the job's device | Completed event with known job_id, worker_id="0", elapsed_ms=5000 | DB shows `status=completed`, `completed_at` is set; ledger reservation for device 0 is zeroed | `cargo test -p anvilml-scheduler --test event_loop_tests test_completed_persists_status_and_releases_ledger -- --nocapture` exits 0 |
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_failed_persists_status_error_and_releases_ledger` | `WorkerEvent::Failed` sets `Job.status=Failed`, `completed_at=now`, `error=event.error` in JobStore, and releases the VRAM reservation | In-memory JobStore with a Running job; ledger has a reservation | Failed event with job_id, worker_id="0", error="CUDA OOM" | DB shows `status=failed`, `error="CUDA OOM"`, `completed_at` set; ledger reservation zeroed | `cargo test -p anvilml-scheduler --test event_loop_tests test_failed_persists_status_error_and_releases_ledger -- --nocapture` exits 0 |
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_cancelled_persists_status_and_releases_ledger` | `WorkerEvent::Cancelled` sets `Job.status=Cancelled`, `completed_at=now` in JobStore, and releases the VRAM reservation | In-memory JobStore with a Running job; ledger has a reservation | Cancelled event with job_id, worker_id="1" | DB shows `status=cancelled`, `completed_at` set; ledger reservation zeroed | `cargo test -p anvilml-scheduler --test event_loop_tests test_cancelled_persists_status_and_releases_ledger -- --nocapture` exits 0 |
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_terminal_events_publish_ws_event` | All three terminal events publish the correct `WsEvent` variant through the broadcaster alongside status persistence | Event loop spawned with real transport; broadcaster subscriber active | Completed/Failed/Cancelled events sent via DEALER socket | Broadcaster receives `JobCompleted`/`JobFailed`/`JobCancelled` with correct fields | `cargo test -p anvilml-scheduler --test event_loop_tests test_terminal_events_publish_ws_event -- --nocapture` exits 0 |
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_terminal_event_unknown_job_logs_warning` | When a terminal event arrives for a job_id not in the database, the event loop logs a warning and continues (doesn't crash) | Event loop spawned; no matching job in database | Completed event with a UUID that doesn't exist in JobStore | No panic; event loop continues processing; log contains warning | `cargo test -p anvilml-scheduler --test event_loop_tests test_terminal_event_unknown_job_logs_warning -- --nocapture` exits 0 |
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_progress_still_published_via_map_worker_event` | Non-terminal events (`Progress`) still flow through the existing `map_worker_event()` path unchanged | Event loop spawned; Progress event sent | Progress event via DEALER socket | Broadcaster receives `JobProgress` with correct fields | `cargo test -p anvilml-scheduler --test event_loop_tests test_progress_still_published_via_map_worker_event -- --nocapture` exits 0 |

Acceptance command for the full test suite:
```bash
cargo test -p anvilml-scheduler --test event_loop_tests -- --nocapture
```
Expected: ≥15 tests total, all exit 0.

## CI Impact

No CI changes required. The changes are entirely within existing test modules (`event_loop_tests.rs`) and existing source files. The `cargo test --workspace --features mock-hardware` CI job will pick up the new tests automatically since they live in `crates/anvilml-scheduler/tests/`. No new CI jobs, gates, or test file placements are introduced.

## Platform Considerations

None identified. The changes are platform-neutral Rust code operating on in-memory data structures (HashMap, SQLite) and the tokio async runtime. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `get_job()` in the event loop returns `None` for a job that was dispatched but not yet persisted (a race between dispatch and event arrival). The job lookup path handles this by logging a warning and still publishing the `WsEvent`, so the WebSocket stream is not interrupted. | Low | Medium | The dispatch path persists the job (`job_store.upsert()`) before sending the `Execute` message to the worker, so the job always exists in the DB when the worker sends back a terminal event. The `Ok(None)` path is a defensive fallback. |
| The interim-patch removal in `backend/src/main.rs` and `crates/anvilml-worker/` causes a compilation break if P16-B1 has not yet landed. Since P16-A2 is a prerequisite for P16-B1 (per the task graph), this is not an issue — P16-B1 runs after P16-A2 completes. | Low | High | The plan includes the full interim-patch removal (scheduler, worker, and backend) in this task's scope, ensuring the build remains clean after staging. |
| `worker_id` parsing as `device_index` fails because the worker_id is not a valid u32 string. This would result in `device_index=0` and releasing VRAM on the wrong device. | Low | Medium | The worker_id convention (bare device index as string) is enforced by the worker spawn path. A defensive parse with `unwrap_or(0)` is used, and the ledger's `release()` uses saturating subtraction so releasing on a device with no reservation is a no-op. |
| VRAM release amount doesn't match the reservation amount if the ledger state changed between dispatch and completion (e.g., another job was dispatched to the same device). | Medium | Medium | The ledger tracks cumulative reservations per device. `release()` uses saturating subtraction, so releasing more than reserved clamps to zero. The reservation amount is read from the ledger at event time (not cached), so it reflects the current state. This means a `release()` call returns the reservation to zero regardless of the amount — which is correct because the job's reservation is part of the cumulative total and the ledger handles the math. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests` exits 0 (≥15 total tests)
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_completed_persists_status_and_releases_ledger` exits 0 — verifies Completed sets `completed_at` and releases ledger
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_failed_persists_status_error_and_releases_ledger` exits 0 — verifies Failed persists the error string and releases ledger
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_cancelled_persists_status_and_releases_ledger` exits 0 — verifies Cancelled persists correctly and releases ledger
- [ ] `cargo test -p anvilml-scheduler --test event_loop_tests test_terminal_events_publish_ws_event` exits 0 — verifies all three terminal events publish the correct WsEvent
- [ ] `cargo fmt --all -- --check` exits 0 (formatting pass)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (lint pass)
