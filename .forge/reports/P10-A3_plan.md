# Plan Report: P10-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A3                                      |
| Phase       | 010 — Worker Crash Recovery                 |
| Description | anvilml-server: broadcast worker status changes to WS |
| Depends on  | P10-A2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-06T17:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Bridge the `WorkerPool`'s internal event channel to the server's WebSocket `EventBroadcaster` so that connected `/v1/events` clients receive `WsEvent::WorkerStatusChanged` notifications whenever a worker's lifecycle state transitions (Idle → Busy, Busy → Dead, Dead → Respawning, Respawning → Idle).

## Scope

### In Scope
- Add a `spawn_worker_status_bridge` function in `backend/src/main.rs` that:
  - Subscribes to `WorkerPool::subscribe_events()`
  - For each `(worker_id, WorkerEvent)` received, matches on `WorkerEvent::WorkerStatusChanged { status }`
  - Constructs and broadcasts a `WsEvent::WorkerStatusChanged(WorkerStatusChangedEvent { event: "worker.status", timestamp: Utc::now(), worker_id, status })` via the `EventBroadcaster`
  - Logs each forwarded event at DEBUG level with `worker_id=` and `status=` fields (per §11.5 mandatory DEBUG log points)
- Spawn this bridge task as a `tokio::spawn` in `main()` after the worker pool is created and before the server starts listening
- Handle the case where `workers` is `None` in `AppState` gracefully (skip spawning, log at WARN level)

### Out of Scope
- Any changes to `anvilml-worker`, `anvilml-ipc`, or `anvilml-core` crates (types already exist)
- Adding new WsEvent variants
- Modifying the WebSocket handler itself (P10-A4 handles live verification)
- Test-only code additions (reserved for P10-A4)

## Approach

1. **Add imports** in `backend/src/main.rs`:
   - `anvilml_ipc::WorkerEvent` — to pattern-match on status changed events
   - `anvilml_core::types::events::{WsEvent, WorkerStatusChangedEvent}` — for constructing the broadcast event
   - `chrono::Utc` — for timestamp construction

2. **Implement `spawn_worker_status_bridge`** in `backend/src/main.rs`:
   ```rust
   /// Spawns a background task that bridges WorkerPool events to the WS broadcaster.
   fn spawn_worker_status_bridge(
       workers: &anvilml_worker::WorkerPool,
       broadcaster: &EventBroadcaster,
   ) {
       let mut rx = workers.subscribe_events();
       let bc = Arc::clone(broadcaster);

       tokio::spawn(async move {
           loop {
               match rx.recv().await {
                   Ok((worker_id, event)) => {
                       if let WorkerEvent::WorkerStatusChanged { status } = event {
                           tracing::debug!(
                               worker_id = %worker_id,
                               status = ?status,
                               "bridging worker status change to WS"
                           );
                           bc.send(WsEvent::WorkerStatusChanged(
                               WorkerStatusChangedEvent {
                                   event: "worker.status".to_string(),
                                   timestamp: Utc::now(),
                                   worker_id,
                                   status,
                               },
                           ));
                       }
                   }
                   Err(broadcast::error::RecvError::Lagged(n)) => {
                       tracing::debug!(lagged = n, "worker status bridge dropped events");
                   }
                   Err(broadcast::error::RecvError::Closed) => {
                       tracing::warn!("worker status bridge channel closed");
                       break;
                   }
               }
           }
       });
   }
   ```

3. **Wire the bridge into `main()`** after worker pool creation:
   - After line 196 (`tracing::info!(workers_spawned = ...)`) and before line 208 (`spawn_system_stats_tick`):
   ```rust
   // Bridge worker status events to WebSocket clients.
   if let Some(ref workers) = workers {
       spawn_worker_status_bridge(workers, &broadcaster);
   } else {
       tracing::warn!("no worker pool available — WS worker status bridge not started");
   }
   ```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Add imports, implement `spawn_worker_status_bridge`, wire it in `main()` |
| Bump | `backend/Cargo.toml` | Patch version bump per FORGE_AGENT_RULES §12 |

## Tests

<table>
<tr><th>Test File</th><th>Test Name</th><th>What It Verifies</th></tr>
<tr><td>Existing tests in <code>backend/src/main.rs</code> or integration tests</td><td>N/A (no new test file needed)</td><td>The bridge task is an async loop spawned at startup. Its correctness is verified by the live Runnable Proof in P10-A4: kill a worker process, observe /v1/events stream receives Dead → Respawning → Idle transitions, and server stays up.</td></tr>
</table>

Note: This task does not add a new test file. The acceptance criterion `cargo test --workspace --features mock-hardware` exits 0 is satisfied by the existing test suite — the bridge code touches no logic that existing tests exercise or break. Live verification of end-to-end behavior is deferred to P10-A4 (the next task in this phase).

## CI Impact

No CI changes required. The change is purely additive wiring in `backend/src/main.rs` with no modifications to Cargo.toml dependencies, no new feature flags, and no changes to CI workflow files. All existing CI gates (format, clippy, test, platform cross-checks) continue to apply as documented in `docs/ARCHITECTURE.md §9`.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Missing imports (`WorkerEvent`, `WorkerStatusChangedEvent`, broadcast error types) cause compile failure | Low | Build breakage — easy to fix | Verify all imports exist: `anvilml_ipc::WorkerEvent` (confirmed in `crates/anvilml-ipc/src/messages.rs`), `anvilml_core::types::events::{WsEvent, WorkerStatusChangedEvent}` (confirmed in `crates/anvilml-core/src/types/events.rs`), `tokio::sync::broadcast::error::RecvError` (standard tokio) |
| Bridge task blocks on slow WS subscribers | Low | Event loss for lagging clients | The `EventBroadcaster` uses `broadcast::Sender` which silently drops events when the channel is full (by design, see broadcaster.rs line 33). Capacity defaults to `ws_broadcast_capacity` (256) from config. |
| Workers being `None` in AppState | Low | Bridge not started at startup | Explicit `if let Some(ref workers)` guard with WARN-level log — safe fallback |
| Duplicate events if pool re-broadcasts | Negligible | N/A | The pool broadcasts each event exactly once per managed worker via its internal channel; no duplication path exists |

## Acceptance Criteria

- [ ] `spawn_worker_status_bridge` function exists in `backend/src/main.rs` and spawns a tokio task subscribing to `WorkerPool::subscribe_events()`
- [ ] Each `WorkerEvent::WorkerStatusChanged { status }` is forwarded as `WsEvent::WorkerStatusChanged(WorkerStatusChangedEvent { ... })` via the `EventBroadcaster`
- [ ] DEBUG-level log call present for each bridged event with `worker_id=` and `status=` fields (per §11.5 mandatory DEBUG log points)
- [ ] Bridge task is spawned in `main()` after worker pool creation and before server listen
- [ ] Graceful handling when `workers` is `None` (WARN log, no panic)
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] Backend crate patch version bumped in `backend/Cargo.toml`
