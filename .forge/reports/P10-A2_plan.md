# Plan Report: P10-A2

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P10-A2                                                      |
| Phase       | 010 — Worker Crash Recovery                                 |
| Description | Extend ManagedWorker's run loop to detect unexpected child exit and transition to Dead |
| Depends on  | P901-A3 (continuous run loop), P10-A1 (RespawnPolicy type)  |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-17T14:30:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Add a `child.wait()` arm to `ManagedWorker::run()`'s `tokio::select!` loop so that when the Python worker subprocess exits unexpectedly (without sending a Dying event), the worker's status transitions to `Dead`, a `WorkerStatusChanged(Dead)` event is broadcast to WebSocket subscribers, and a structured `tracing::info!` log records the exit code. This is the crash detection half of Phase 010 — respawn logic is deferred to P10-A3.

## Scope

### In Scope
- Add `device_index: u32` field to `ManagedWorker` struct (currently missing, needed for `WorkerStatusChanged` broadcast)
- Populate `device_index` from the `WorkerEvent::Ready` event in the existing event processing branch
- Add third `select!` arm (`child.wait()`) that detects unexpected subprocess exit
- On child exit: transition status to `Dead`, broadcast `WsEvent::WorkerStatusChanged(Dead)`, log with `tracing::info!(worker_id, exit_code, "worker exited unexpectedly")`
- Add `// TODO(P14)` comment at the Dead transition for future job-failure notification
- Add `test_child_exit_transitions_dead` to `managed_tests.rs` — spawns a real subprocess, kills it, asserts `Dead` status is observed
- Bump `anvilml-worker` crate patch version from `0.1.10` to `0.1.11`

### Out of Scope
- Respawn cycle (deferred to P10-A3)
- Job-failure notification (deferred to Phase 14 — `JobScheduler` does not exist yet)
- Manual worker restart endpoint (deferred to P10-B1)
- Changes to `pool.rs`, `bridge.rs`, `spawn.rs`, or `respawn.rs`

## Existing Codebase Assessment

The `managed.rs` file contains `ManagedWorker` with a `run()` method that has a `loop { tokio::select! { ... } }` with two arms: a ready-timeout arm (armed only in `Initializing` state) and an `event_rx.recv()` arm for processing IPC events from the bridge reader. The loop breaks on `RecvError::Closed` (bridge reader exit).

The struct currently stores `worker_id: String` and `device_name: String` but does NOT store `device_index: u32` — this field is available on `WorkerEvent::Ready` but is never captured into the struct. The `RespawnPolicy` is already imported and default-constructed, and the `WorkerStatus` enum already includes the `Dead` variant.

The broadcast channel (`event_tx: broadcast::Sender<(String, WorkerEvent)>`) is used exclusively for IPC events from the bridge reader. There is no separate channel for `WsEvent` status changes — the `ManagedWorker` struct does not currently have a `broadcast::Sender<WsEvent>` field.

Test style in `managed_tests.rs` uses `make_test_worker()` helper which constructs workers via `ManagedWorker::new()` with `child: None`. No test currently spawns a real subprocess.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| crate  | tokio   | 1.52.3          | workspace Cargo.toml | full (includes tokio::process) |

No new dependencies introduced. `tokio::process::Child::wait()` is part of the `full` feature set already enabled in the workspace.

## Approach

1. **Add `device_index: u32` field to `ManagedWorker` struct.** Add a new pub field `device_index: u32` after `device_name: String`. This field is populated from the `Ready` event and needed for the `WorkerStatusChanged` broadcast.

2. **Populate `device_index` in the Ready event branch.** In the existing `WorkerEvent::Ready` match arm (line ~407 of managed.rs), extract `device_index` alongside `device_name` and store it: `self.device_index = device_index;`. Add an inline comment explaining that device_index is stored for status broadcast.

3. **Add `device_index` to `ManagedWorker::new()` constructor.** Add `device_index: u32` parameter and initialize the field in the struct literal. Update the `#[allow(dead_code)]` attribute on the struct to also suppress the warning for the new field (it is populated at runtime from the Ready event).

4. **Add third `select!` arm for `child.wait()`.** Add a new arm to the existing `tokio::select!` block:
   ```rust
   // Child process exited unexpectedly.
   // This arm only fires when child is Some (production).
   // In tests (child = None), the arm uses a never-firing placeholder.
   _ = async {
       match self.child.as_mut() {
           Some(child) => {
               // Wait for the subprocess to exit. This is the crash
               // detection mechanism — if the child exits without
               // sending a Dying event, we detect it here.
               let exit_status = child.wait().await;
               // Pin the future since tokio::process::Child::wait()
               // returns a !Unpin type.
               std::pin::pin!(exit_status).as_mut().await
           }
           None => {
               // No child process (test mode). Use a never-firing
               // sleep so this arm never wins the select.
               tokio::time::sleep(std::time::Duration::MAX).await;
           }
       }
   } => {
       // The subprocess has exited. Transition to Dead, broadcast
       // the status change, and log the exit code.
       //
       // TODO(P14): notify JobScheduler of in-flight job failure
       // once it exists. The scheduler is introduced in P13-A3 and
       // wired to dispatch in P14-A1.
       let exit_code = match self.child.as_mut().and_then(|c| c.try_wait().ok()).flatten() {
           Some(status) => status.code(),
           None => None,
       };
       *self.status.write().await = WorkerStatus::Dead;
       tracing::info!(
           worker_id = %self.worker_id,
           exit_code = ?exit_code,
           "worker exited unexpectedly"
       );
       // Broadcast the Dead status to WebSocket subscribers.
       // The event_tx channel carries (String, WorkerEvent) from
       // the bridge reader, but WorkerStatusChanged is a WsEvent
       // sent by the state machine. We use the same broadcast
       // channel since it is the only sender available on the
       // struct. The bridge reader's clone remains the writer;
       // our send adds a status event alongside IPC events.
       //
       // Note: if the bridge reader has already exited (channel
       // closed), this send will return Err(SendError) which we
       // ignore — subscribers will observe the Dead status via
       // the next GET /v1/workers poll anyway.
       let _ = self.event_tx.send((
           self.worker_id.clone(),
           WorkerEvent::Dying {
               reason: format!("child process exited with code {:?}", exit_code),
           },
       ));
       break;
   }
   ```
   The arm uses `child.as_mut()` to get a mutable borrow of the child, then `child.wait().await` to wait for exit. A `None` arm uses a never-firing `Duration::MAX` sleep.

5. **Update `ManagedWorker::spawn()` to pass `device_index`.** In the `spawn()` method, pass `device.index` as the `device_index` parameter when constructing the `ManagedWorker` at the end of the method.

6. **Add test `test_child_exit_transitions_dead`.** Create a new test in `managed_tests.rs` that:
   - Spawns a real `ManagedWorker` via `ManagedWorker::spawn()` (not `new()`)
   - Waits briefly for initialization
   - Kills the child process with `kill()`
   - Asserts the status transitions to `Dead` within a generous timeout
   - Uses `#[serial]` annotation since it spawns a real subprocess (avoids race with other tests)
   - Sets `ANVILML_WORKER_MOCK=1` env var and restores it unconditionally

7. **Bump `anvilml-worker` crate version.** Change `version = "0.1.10"` to `version = "0.1.11"` in `crates/anvilml-worker/Cargo.toml`.

## Public API Surface

No new `pub` items introduced. Changes are to existing private/`pub(crate)` fields and methods:

| Item | Path | Change |
|------|------|--------|
| `device_index: u32` | `ManagedWorker` (managed.rs) | New field on existing struct |
| `device_index` parameter | `ManagedWorker::new()` | New parameter in existing method |
| `device_index` argument | `ManagedWorker::spawn()` | New argument passed in existing method |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/managed.rs` | Add `device_index` field, populate from Ready event, add `child.wait()` select! arm |
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | Add `test_child_exit_transitions_dead` test |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_child_exit_transitions_dead` | When a spawned subprocess exits unexpectedly, the worker status transitions to Dead and the exit code is logged | `mock-hardware` feature active; Python venv available | Real subprocess via `ManagedWorker::spawn()` | Status becomes `Dead` within timeout; no panic | `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_child_exit_transitions_dead` exits 0 |

## CI Impact

No CI changes required. The new test is an existing test module (`managed_tests.rs`) within the `anvilml-worker` crate, which is already picked up by `cargo test --workspace --features mock-hardware`. The test uses `#[serial]` annotation to prevent concurrent subprocess spawning races.

## Platform Considerations

None identified. The `tokio::process::Child::wait()` API is cross-platform (works on Linux, Windows, macOS). The `child.as_mut().unwrap().wait()` pattern from the task description is implemented as `child.as_mut()` with `match` to handle `None` in tests. The `try_wait()` call is also cross-platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Race between `child.wait()` arm and bridge reader exit — both may fire nearly simultaneously when the subprocess dies. The bridge reader may exit first (closing the channel), causing the `child.wait()` arm to not fire before the loop breaks on `RecvError::Closed`. | Medium | High | Use `child.as_mut()` with `match` so the arm always exists in the select! but only fires when `Some`. The `RecvError::Closed` arm remains as a fallback. Test with a fast-exiting subprocess to verify the `child.wait()` arm fires before channel close. |
| The broadcast send for status change may fail if the bridge reader has already exited and closed the channel. | Low | Medium | Use `let _ = self.event_tx.send(...)` to ignore the `SendError`. The Dead status is still observable via the status field and the next `GET /v1/workers` poll. Add a log at DEBUG level if the send fails. |
| Test flakiness — spawning a real subprocess and waiting for it to exit introduces timing variability. | Medium | Medium | Use a generous timeout (10 seconds) on `tokio::time::timeout`. Kill the child explicitly with `child.kill().await` for deterministic exit. Use `#[serial]` to prevent concurrent subprocess spawning. Restore env vars unconditionally. |
| `device_index` field access in the `child.wait()` arm — the field is populated from the Ready event which may not have arrived yet if the child exits very quickly. | Low | Medium | The `device_index` is used only for the `WorkerStatusChanged` broadcast. If the Ready event hasn't arrived, `device_index` is `0` (the default from `ManagedWorker::new()`), which is acceptable for crash detection — the worker is Dead regardless. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- managed_tests::test_child_exit_transitions_dead` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all existing tests still pass)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no regression in other crates)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (code is formatted)
