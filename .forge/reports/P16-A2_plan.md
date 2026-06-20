# Plan Report: P16-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P16-A2                                            |
| Phase       | 016 — Live Job Events                             |
| Description | anvilml-scheduler: relay Progress and ImageReady events to WebSocket |
| Depends on  | P15-A2, P16-A1                                    |
| Project     | anvilml                                           |
| Planned at  | 2026-06-20T20:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Extend the scheduler's event loop to handle `WorkerEvent::Progress` and relay it as `WsEvent::JobProgress` to connected WebSocket clients. Additionally, add the missing `JobStarted` broadcast in the dispatch loop so the full 7-event sequence (`JobQueued → JobStarted → JobProgress×3 → JobImageReady → JobCompleted`) is observable. This enables frontend clients to display live per-step progress during job execution.

## Scope

### In Scope
- **`crates/anvilml-scheduler/src/event_loop.rs`**: Add a `WorkerEvent::Progress` arm to the `handle_event` match, broadcasting `WsEvent::JobProgress { job_id, step, total_steps, preview_b64 }` and emitting a `tracing::debug!` log.
- **`crates/anvilml-scheduler/src/scheduler.rs`**: Add `broadcaster.send(WsEvent::JobStarted { job_id, worker_id })` in the `dispatch_once` loop after sending each Execute message, so `JobStarted` appears in the event sequence.
- **`crates/anvilml-scheduler/tests/progress_tests.rs`**: New integration test file that subscribes to the WS event channel, manually sends a 7-event sequence through the broadcaster, and asserts the received events are in the correct order with correct fields.

### Out of Scope
- Changes to `worker/executor.py` (handled by P16-A1).
- Changes to the Python worker's IPC layer.
- DB writes for Progress events (explicitly excluded by task constraints).
- Progress persistence or aggregation.
- Any changes to `anvilml-core`, `anvilml-ipc`, or `anvilml-server` crates.

## Existing Codebase Assessment

The event loop (`event_loop.rs`) already handles three worker event variants:
- `WorkerEvent::Completed` → DB update, VRAM release, `WsEvent::JobCompleted` broadcast
- `WorkerEvent::Failed` → DB update with error, VRAM release, `WsEvent::JobFailed` broadcast
- `WorkerEvent::ImageReady` → base64 decode, artifact persistence, `WsEvent::JobImageReady` broadcast

All other variants (including `Progress`, `Cancelled`, `Ready`, `Pong`, `Dying`, `MemoryReport`) fall through to the catch-all `_ => { tracing::debug!(...) }` arm and are silently ignored. This is the gap: Progress events are currently dropped.

The dispatch loop (`scheduler.rs`, `dispatch_once`) sends Execute messages to workers but does **not** broadcast `WsEvent::JobStarted`. This means the 7-event sequence is incomplete without this addition.

The `WsEvent::JobProgress` variant already exists in `anvilml-core/src/types/events.rs` with fields `{ job_id, step, total_steps, preview_b64: Option<String> }`. The `WorkerEvent::Progress` variant exists in `anvilml-ipc/src/messages.rs` with identical fields. No new types need to be created.

Existing test patterns (in `event_loop_tests.rs` and `image_ready_tests.rs`) use: in-memory DB via `open_in_memory()`, a fresh `JobScheduler`, manual `set_job_running()` to bypass the dispatch loop, and direct `broadcaster.broadcast_worker_event()` calls to inject events. The same pattern should be followed for the new test.

Established patterns to follow:
- Structured logging with `tracing::debug!(key = %value, "message")` syntax.
- `#[serial]` annotation on all tests (SQLite in-memory isolation).
- Helper functions (`make_registry`, `make_scheduler`, `register_device`, `submit_job`, `set_job_running`) extracted to module level.
- Event loop handle aborted at test end with `.abort()`.

## Resolved Dependencies

No new external dependencies. All types and methods used are already present in existing workspace crates.

| Type   | Name            | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------------|-----------------|----------------|------------------------|
| crate  | tokio           | (workspace)     | lockfile       | sync::broadcast        |
| crate  | anvilml-ipc     | 0.1.x (path)    | lockfile       | WorkerEvent enum       |
| crate  | anvilml-core    | 0.1.x (path)    | lockfile       | WsEvent enum           |

All types (`WorkerEvent::Progress`, `WsEvent::JobProgress`, `EventBroadcaster::send`, `EventBroadcaster::broadcast_worker_event`) are confirmed to exist in the current source code — no MCP lookup required since these are internal workspace types already on disk.

## Approach

1. **Add `JobStarted` broadcast to `dispatch_once` in `scheduler.rs`.**
   After the `workers.send_execute(&worker_id, &msg).await` call succeeds (inside the for loop iterating over `to_dispatch`), add:
   ```rust
   // Broadcast JobStarted so WebSocket clients know execution has begun
   // on a specific worker. This is the second event in the lifecycle
   // sequence: JobQueued → JobStarted → ...
   broadcaster.send(WsEvent::JobStarted {
       job_id: job.id,
       worker_id: worker_id.clone(),
   });
   ```
   Rationale: The acceptance criterion requires `JobStarted` in the 7-event sequence. It is not currently broadcast anywhere. Adding it here (immediately after the Execute message succeeds) ensures the event appears before any Progress events from the worker.

2. **Add `WorkerEvent::Progress` arm to `handle_event` in `event_loop.rs`.**
   Insert a new match arm between the existing `ImageReady` arm and the catch-all `_` arm:
   ```rust
   WorkerEvent::Progress {
       job_id,
       step,
       total_steps,
       preview_b64,
   } => {
       // Relay Progress to WebSocket clients without DB write.
       // Progress events are transient UI updates — only terminal
       // statuses (Completed, Failed, Cancelled) are persisted.
       tracing::debug!(
           job_id = %job_id,
           step = step,
           total_steps = total_steps,
           "progress event relayed"
       );
       broadcaster.send(WsEvent::JobProgress {
           job_id,
           step,
           total_steps,
           preview_b64,
       });
   }
   ```
   Rationale: Follows the exact same pattern as `ImageReady` (broadcast + log) but without DB or artifact operations. No VRAM release needed for Progress events.

3. **Write integration test file `crates/anvilml-scheduler/tests/progress_tests.rs`.**
   Create a new test file following the established patterns from `event_loop_tests.rs` and `image_ready_tests.rs`. The test:
   - Creates in-memory DB, scheduler, registry (with `LoadModel` node).
   - Subscribes to the WsEvent channel.
   - Starts the event loop.
   - Submits a job and manually sets it Running in DB.
   - Sends events through the broadcaster in exact sequence: `JobStarted` (simulating dispatch), `Progress(step=1)`, `Progress(step=2)`, `Progress(step=3)`, `ImageReady`, `Completed`.
   - Collects all events via `ws_rx.recv()` with timeouts.
   - Asserts the sequence order and field values.

## Public API Surface

No new public items. All changes are internal to existing modules:
- `event_loop.rs`: `handle_event()` gains one new match arm (private function).
- `scheduler.rs`: `dispatch_once()` gains one `broadcaster.send()` call (private function).
- `tests/progress_tests.rs`: New test file — tests are `pub` by convention but are test-only.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Add `WorkerEvent::Progress` arm to `handle_event`, broadcasting `WsEvent::JobProgress` with debug log |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `broadcaster.send(WsEvent::JobStarted)` in `dispatch_once` after successful Execute send |
| Create | `crates/anvilml-scheduler/tests/progress_tests.rs` | Integration test: verify full 7-event sequence order |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/progress_tests.rs` | `test_full_event_sequence_order` | All 7 events arrive in correct order: JobQueued→JobStarted→Progress(1)→Progress(2)→Progress(3)→JobImageReady→JobCompleted, with correct field values | In-memory DB, scheduler started, event loop running, job manually set to Running | 7 events sent via `broadcaster.broadcast_worker_event()` (Progress ×3, ImageReady, Completed) + JobStarted simulated | All 7 WsEvent variants received in order; Progress steps are 1,2,3; ImageReady has artifact_hash; Completed has elapsed_ms | `cargo test -p anvilml-scheduler --features mock-hardware -- test_full_event_sequence_order` exits 0 |

## CI Impact

No CI changes required. The new test file is in `crates/anvilml-scheduler/tests/` which is automatically picked up by `cargo test --workspace --features mock-hardware`. No new CI jobs, gates, or configuration files are added.

## Platform Considerations

None identified. The changes are platform-neutral:
- `EventBroadcaster::send()` uses `tokio::sync::broadcast` which is cross-platform.
- No `#[cfg(unix)]` or `#[cfg(windows)]` guards needed.
- The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `JobStarted` event timing: if the test sends `JobStarted` via the broadcaster (since we can't trigger the real dispatch loop without a real worker), it will arrive after events already sent through the event loop's worker event channel. The test must send `JobStarted` through the WsEvent channel directly (not through `broadcast_worker_event`), since `JobStarted` is broadcast by the dispatch loop as a `WsEvent`, not a `WorkerEvent`. | Low | Medium | The test sends `JobStarted` via `broadcaster.send(WsEvent::JobStarted{...})` directly (not through the worker event channel), matching how the dispatch loop broadcasts it. The event loop does not need to handle this — it's a direct WsEvent broadcast. |
| Test event ordering: the test sends events sequentially through `broadcast_worker_event()`, but the event loop processes them asynchronously. If the event loop hasn't consumed one event before the next is sent, the broadcast channel buffer (1024) could hold multiple events, but the `recv()` order on the WsEvent side depends on when the event loop calls `broadcaster.send()`. | Medium | High | The test collects events with timeouts and verifies order explicitly. Since events are sent sequentially with no delays between them, and the event loop processes them synchronously (one at a time, no async in Progress handler), the order is deterministic. The `JobStarted` event is sent directly by the test (not through event loop), so its position is fixed by the test's send order. |
| Pre-existing test breakage: modifying `dispatch_once` to add a `broadcaster.send()` call could affect existing dispatch tests that check broadcast channel contents. | Low | Medium | The existing dispatch tests (`dispatch_tests.rs`) do not subscribe to the WsEvent channel, so they are unaffected. If any test does subscribe and sees the new `JobStarted` event, it will need an additional `recv()` call — but inspection shows no existing dispatch test subscribes to WsEvents. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- test_full_event_sequence_order` exits 0
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 (all scheduler tests pass, no regressions)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no workspace-level regressions)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (code is formatted)
- [ ] `grep -n "JobProgress" crates/anvilml-scheduler/src/event_loop.rs` returns at least one match (Progress relay is present)
- [ ] `grep -n "JobStarted" crates/anvilml-scheduler/src/scheduler.rs` returns at least one match (JobStarted broadcast is present)
