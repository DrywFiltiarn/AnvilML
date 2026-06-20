# Plan Report: P14-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-A3                                        |
| Phase       | 014 — Dispatch & Mock Execute                 |
| Description | anvilml-scheduler: handle Completed/Failed events, update job status |
| Depends on  | P14-A1 (dispatch loop background task)        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-20T09:50:00Z                          |
| Attempt     | 1                                             |

## Objective

Close the job lifecycle event loop in `JobScheduler` by subscribing to worker events and updating job status when workers report `Completed` or `Failed`. On `Completed`, the scheduler updates the database (`status=Completed, completed_at=now`), releases the VRAM reservation via `VramLedger::release`, broadcasts `WsEvent::JobCompleted`, and emits a mandatory INFO log. On `Failed`, it updates the database (`status=Failed, error`), releases VRAM, broadcasts `WsEvent::JobFailed`, and emits a mandatory INFO log. The acceptance criterion is that `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with tests covering both event paths.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/event_loop.rs` — the event subscription loop module with `start_event_loop` method.
- Modify `crates/anvilml-scheduler/src/scheduler.rs` — add `start_event_loop` public method to `JobScheduler`, update the TODO comment in `dispatch_once`.
- Modify `crates/anvilml-scheduler/src/lib.rs` — re-export `event_loop` module.
- Modify `crates/anvilml-ipc/src/ws/broadcaster.rs` — add `broadcast_worker_event` method to `EventBroadcaster` for sending `WorkerEvent` values to subscribers.
- Create `crates/anvilml-scheduler/tests/event_loop_tests.rs` — ≥ 3 tests covering Completed, Failed, and unknown event handling.

### Out of Scope
- Handling `ImageReady`, `Progress`, `Cancelled`, or other `WorkerEvent` variants (handled in later phases).
- Modifying `WorkerPool` or `demux` to wire the broadcast channel (the scheduler subscribes independently).
- Artifact storage (Phase 015).
- Modifying `managed.rs` or `worker_main.py`.

## Existing Codebase Assessment

The `JobScheduler` in `scheduler.rs` already implements job submission (`submit`), querying (`get_job`, `list_jobs`), and the dispatch loop (`start_dispatch_loop`, `dispatch_once`, `select_worker`). The dispatch loop marks jobs `Running` in the database, reserves VRAM via `VramLedger::reserve`, and sends `WorkerMessage::Execute` to workers. A `TODO` comment at line 553 of `scheduler.rs` explicitly notes: "P14-A3 will add job failure handling and re-enqueue."

The `EventBroadcaster` in `anvilml-ipc/src/ws/broadcaster.rs` provides a `tokio::sync::broadcast` channel for `WsEvent` messages (WebSocket client notifications). It has a `subscribe()` method that returns a `broadcast::Receiver<WsEvent>`. The `WorkerPool` uses this to broadcast `WorkerStatusChanged` events.

The `WorkerEvent` enum (in `anvilml-ipc/src/messages.rs`) already defines `Completed { job_id, elapsed_ms }` and `Failed { job_id, error, traceback }` variants. The `WsEvent` enum (in `anvilml-core/src/types/events.rs`) already defines `JobCompleted { job_id, elapsed_ms }` and `JobFailed { job_id, error }` variants.

The `VramLedger` in `ledger.rs` has a `release(index, mib)` method that decrements reservations and panics on underflow. The dispatch loop already reserves VRAM; the event loop needs to release it.

The database schema (migration 001) has `completed_at` (TEXT, nullable) and `error` (TEXT, nullable) columns in the `jobs` table, both of which are used by `insert_job` and `row_to_job`.

Test patterns established in `dispatch_tests.rs` and `scheduler_tests.rs`: use `#[serial]` annotation, create in-memory databases via `open_in_memory()`, build `JobScheduler` with `make_scheduler()` helper, use `WorkerPool::new()` with pre-built status handles for tests, and abort dispatch/event handles at test teardown.

## Resolved Dependencies

| Type   | Name           | Version verified | MCP source     | Feature flags confirmed |
|--------|----------------|-----------------|----------------|------------------------|
| crate  | tokio          | (workspace)     | Cargo.lock     | sync::broadcast        |
| crate  | sqlx           | (workspace)     | Cargo.lock     | sqlite                 |

No new external dependencies are introduced. The task uses existing dependencies: `tokio::sync::broadcast` (already used by `EventBroadcaster` and `WorkerPool`), `sqlx` (already used for all database operations), `chrono::Utc` (already imported in `scheduler.rs`), and `tracing` (already imported).

## Approach

### Step 1: Add worker event broadcast channel to `EventBroadcaster`

**File:** `crates/anvilml-ipc/src/ws/broadcaster.rs`

Add a new `broadcast::Sender<WorkerEvent>` field and a `subscribe_worker_events()` method to `EventBroadcaster`. This creates a separate broadcast channel dedicated to `WorkerEvent` messages, so the scheduler can subscribe independently of the WebSocket client channel.

```rust
// New fields in EventBroadcaster struct:
worker_event_tx: broadcast::Sender<WorkerEvent>,

// In new() — create the channel alongside the existing one:
let (worker_event_tx, _worker_event_rx) = broadcast::channel(1024);

// New method:
pub fn subscribe_worker_events(&self) -> broadcast::Receiver<WorkerEvent> {
    self.worker_event_tx.subscribe()
}

// New method for sending events:
pub fn broadcast_worker_event(&self, event: WorkerEvent) {
    if self.worker_event_tx.send(event).is_err() {
        tracing::debug!("worker event broadcast: no subscribers");
    }
}
```

Rationale: Using a separate channel avoids mixing `WorkerEvent` (internal IPC) with `WsEvent` (external WebSocket) types. The scheduler subscribes to one, WebSocket clients subscribe to the other.

### Step 2: Create the event loop module

**File:** `crates/anvilml-scheduler/src/event_loop.rs` (new file)

Implement `pub fn start_event_loop(scheduler: &JobScheduler, broadcaster: Arc<EventBroadcaster>) -> JoinHandle<()>` that:

1. Clones necessary references from the scheduler (queue, ledger, db, broadcaster).
2. Subscribes to the worker event channel via `broadcaster.subscribe_worker_events()`.
3. Enters a `tokio::select!` loop that receives events and dispatches to handler functions.
4. On `WorkerEvent::Completed { job_id, elapsed_ms }`:
   a. Update the database: `UPDATE jobs SET status='completed', completed_at=? WHERE id=?` with `Utc::now()`.
   b. Query the job's `worker_id` from the database to determine which device to release VRAM on.
   c. Query the job's `vram_estimate` — but since the scheduler doesn't store this per-job, use the same 4096 MiB default that the dispatch loop uses.
   d. Call `ledger.release(device_index, 4096)`.
   e. Broadcast `WsEvent::JobCompleted { job_id, elapsed_ms }` via the broadcaster.
   f. Emit mandatory INFO log: `tracing::info!(job_id = %job_id, elapsed_ms = elapsed_ms, "job completed")`.
5. On `WorkerEvent::Failed { job_id, error, .. }`:
   a. Update the database: `UPDATE jobs SET status='failed', error=? WHERE id=?`.
   b. Query the job's `worker_id` and release VRAM (same 4096 MiB default).
   c. Broadcast `WsEvent::JobFailed { job_id, error }`.
   d. Emit mandatory INFO log: `tracing::info!(job_id = %job_id, error = %error, "job failed")`.
6. For any other `WorkerEvent` variant: log at DEBUG level and continue (future phases will handle `ImageReady`, `Progress`, etc.).
7. If the broadcast receiver is closed (sender dropped), log at WARN and exit the loop.

Key implementation detail — VRAM release: The scheduler does not currently store the VRAM amount per-job. The dispatch loop uses a hardcoded 4096 MiB default. To keep the event loop simple and consistent, it will use the same 4096 MiB default for VRAM release. This is acceptable because:
- VRAM reservations are advisory, not enforced (§11.4 of DESIGN.md).
- The ledger panics on over-release, so releasing more than reserved would catch a bug.
- Phase 015 will replace the hardcoded value with model-specific metadata.

### Step 3: Wire the event loop into `JobScheduler`

**File:** `crates/anvilml-scheduler/src/scheduler.rs`

Add a `start_event_loop` public method to `JobScheduler`:

```rust
pub fn start_event_loop(&self) -> tokio::task::JoinHandle<()> {
    let queue = Arc::clone(&self.queue);
    let ledger = Arc::clone(&self.ledger);
    let db = self.db.clone();
    let broadcaster = Arc::clone(&self.broadcaster);
    event_loop::start(queue, ledger, db, broadcaster)
}
```

Update the `TODO` comment at line 553 of `scheduler.rs` to indicate completion.

### Step 4: Export the module

**File:** `crates/anvilml-scheduler/src/lib.rs`

Add `pub mod event_loop;` after the existing module declarations.

### Step 5: Write tests

**File:** `crates/anvilml-scheduler/tests/event_loop_tests.rs` (new file)

Write ≥ 3 tests:

1. **`test_completed_event_updates_job_status`**: Submit a job, start the event loop, manually insert the job as `Running` in the DB (simulating dispatch), then send a `WorkerEvent::Completed` through the broadcaster's worker event channel. Verify the DB status is `completed`, `completed_at` is set, and VRAM was released.

2. **`test_failed_event_updates_job_status`**: Same setup, but send `WorkerEvent::Failed { job_id, error: "test error", traceback: None }`. Verify DB status is `failed`, `error` column is set, and VRAM was released.

3. **`test_event_loop_ignores_unknown_event`**: Send a `WorkerEvent::Pong` through the channel. Verify the event loop does not crash and the job remains in its current state.

Each test follows the established pattern: in-memory database, pre-built registry, `make_scheduler()` helper, `#[serial]` annotation, and handle abort on teardown.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `pub fn EventBroadcaster::subscribe_worker_events(&self) -> broadcast::Receiver<WorkerEvent>` | `anvilml-ipc/src/ws/broadcaster.rs` | Subscribe to worker events channel |
| `pub fn EventBroadcaster::broadcast_worker_event(&self, event: WorkerEvent)` | `anvilml-ipc/src/ws/broadcaster.rs` | Send a worker event to all subscribers |
| `pub fn JobScheduler::start_event_loop(&self) -> JoinHandle<()>` | `anvilml-scheduler/src/scheduler.rs` | Spawn the event subscription loop task |
| `pub mod event_loop` | `anvilml-scheduler/src/lib.rs` | Re-exported event loop module |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/event_loop.rs` | Event subscription loop: receives WorkerEvents, updates DB, releases VRAM, broadcasts WsEvent |
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `start_event_loop()` method; remove TODO comment at dispatch_once |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod event_loop;` re-export |
| MODIFY | `crates/anvilml-ipc/src/ws/broadcaster.rs` | Add `worker_event_tx` field, `subscribe_worker_events()`, `broadcast_worker_event()` |
| CREATE | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | ≥ 3 tests for Completed, Failed, and unknown event handling |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_completed_event_updates_job_status` | Completed event transitions job from Running→Completed, sets completed_at, releases VRAM, broadcasts WsEvent::JobCompleted | In-memory DB with a Running job; event loop started | WorkerEvent::Completed { job_id, elapsed_ms: 1234 } | DB status='completed', completed_at is set, reservations for device=0 decrease by 4096, broadcast received | `cargo test -p anvilml-scheduler --features mock-hardware -- event_loop_tests::test_completed_event_updates_job_status` exits 0 |
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_failed_event_updates_job_status` | Failed event transitions job from Running→Failed, sets error, releases VRAM, broadcasts WsEvent::JobFailed | In-memory DB with a Running job; event loop started | WorkerEvent::Failed { job_id, error: "test failure", traceback: None } | DB status='failed', error='test failure', reservations decrease by 4096, broadcast received | `cargo test -p anvilml-scheduler --features mock-hardware -- event_loop_tests::test_failed_event_updates_job_status` exits 0 |
| `crates/anvilml-scheduler/tests/event_loop_tests.rs` | `test_event_loop_ignores_unknown_event` | Unknown WorkerEvent (Pong) does not crash the event loop and leaves job in current state | In-memory DB with a Running job; event loop started | WorkerEvent::Pong { seq: 42 } | Job remains Running, no DB changes, no broadcast | `cargo test -p anvilml-scheduler --features mock-hardware -- event_loop_tests::test_event_loop_ignores_unknown_event` exits 0 |

## CI Impact

No CI changes required. The new test file is in `crates/anvilml-scheduler/tests/`, which is automatically picked up by `cargo test --workspace --features mock-hardware`. The `rust-linux` and `rust-windows` CI jobs already run this command. No new file types, gates, or configuration changes are introduced.

## Platform Considerations

None identified. The event loop uses only platform-neutral Tokio primitives (`broadcast::channel`, `tokio::spawn`, `tokio::time::timeout`). SQLite operations are via sqlx which abstracts platform differences. The VRAM ledger is a pure Rust data structure. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| VRAM release amount mismatch: the event loop releases 4096 MiB (hardcoded default) but the actual dispatch may have reserved a different amount if model-specific VRAM estimation is added in a future phase. This could cause ledger underflow panic. | Medium | High | The 4096 MiB default matches the dispatch loop's current default. Phase 015 will replace both with model-specific values atomically. The ledger panics on underflow, which catches the mismatch immediately during testing rather than silently corrupting state. |
| The event loop subscribes to a broadcast channel that the `EventBroadcaster` creates internally. If the scheduler is started before the `EventBroadcaster` is fully initialised, the subscription may miss early events. | Low | Medium | The scheduler is always constructed before the event loop is started (construction in `main.rs` or `AppState`), and the event loop subscribes at spawn time. Events sent before subscription are not delivered by `tokio::sync::broadcast`, but in practice no Completed/Failed events can occur before the scheduler is ready since jobs must first be submitted and dispatched. |
| The `WorkerEvent::Failed` variant carries a `traceback: Option<String>` field that the current plan does not store in the database. The DB `error` column only stores the `error` field. | Low | Low | The `error` field is the primary diagnostic message; `traceback` is supplementary. Storing the full traceback in the `error` column would make error messages unwieldy. The traceback can be logged at DEBUG level if needed, but it is not required for the P14-A3 scope. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- event_loop_tests` exits 0 (≥ 3 tests)
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (all scheduler tests pass)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (no formatting drift)
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (compiles)
- [ ] The event loop logs `tracing::info!(job_id = %job_id, elapsed_ms = elapsed_ms, "job completed")` on Completed events
- [ ] The event loop logs `tracing::info!(job_id = %job_id, error = %error, "job failed")` on Failed events
