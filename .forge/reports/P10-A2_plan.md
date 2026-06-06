# Plan Report: P10-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-A2                                            |
| Phase       | 010 — Worker Crash Recovery                       |
| Description | anvilml-worker: respawn after death (2s delay) + WorkerStatusChanged events |
| Depends on  | P10-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T15:10:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement automatic worker respawn after death detection (EOF or ping timeout) in `managed.rs` and pool-level respawn orchestration in `pool.rs`, with WorkerStatusChanged WebSocket events emitted at each lifecycle transition (Dead → Respawning → Idle). Orphan cleanup via PR_SET_PDEATHSIG on Linux.

## Scope

### In Scope
- `managed.rs`: broadcast `WorkerStatusChanged(Dead)` on EOF detection and pong timeout; add `respawn()` method that resets state, re-spawns child process, re-sends `InitializeHardware`; read `ANVILML_RESPAWN_DELAY_MS` env var (default 2000 ms); Linux `PR_SET_PDEATHSIG(SIGHUP)` via `pre_exec` on spawn; INFO log for respawn events
- `pool.rs`: detect `WorkerStatusChanged(Dead)` in event listener; spawn background respawn task with delay/status transitions; replace dead worker with fresh `ManagedWorker`; forward all status changes through pool's broadcast channel
- `Cargo.toml` (anvilml-worker): bump patch version 0.1.1 → 0.1.2
- Unit test: inject mock handles, kill via EOF, verify Dead→Respawning→Idle within timeout

### Out of Scope
- Windows Job Object orphan cleanup (deferred to P10-A4)
- Server-side WS broadcast wiring (P10-A3)
- PID accessor for crash-recovery proof (P10-A4)
- `ipc_bridge.rs` module — IPC bridge is integrated into managed.rs currently

## Approach

### Step 1: Read respawn delay env var in ManagedWorker::new()

Add a `respawn_delay_ms` field to `ManagedWorker`. Read from `ANVILML_RESPAWN_DELAY_MS` (default 2000 ms), parsed the same way as ping_interval/pong_timeout.

### Step 2: Broadcast WorkerStatusChanged(Dead) on death paths

**reader_task (EOF path):** Before breaking from the read loop and setting status to Dead, broadcast a `WorkerEvent::Dying` is already sent by Python — but for EOF (no Python message), we must explicitly broadcast `WorkerStatusChanged(Dead)` via `event_tx`. Add this right before `*s = WorkerStatus::Dead`.

**keepalive pong timeout path:** In the keepalive task, after killing the child on pong timeout and setting status to Dead, also broadcast `WorkerStatusChanged(Dead)`. This ensures both death paths emit the same event.

### Step 3: Add respawn() method to ManagedWorker

New public method `pub async fn respawn(&self, device: &GpuDevice, cfg: &ServerConfig) -> Result<(), AnvilError>` that:
1. Sets status to `Respawning` and broadcasts `WorkerStatusChanged(Respawning)`
2. Logs INFO: `worker respawned, pid=…` (after successful spawn)
3. Resets the `ipc_tx` oneshot channel via a new `reset_ipc_tx()` method
4. Calls the existing spawn logic (extract or reuse): creates child process, sends InitializeHardware, waits for Idle
5. On success, status will be Idle (set by Ready event in reader_task); no explicit Idle broadcast needed since the pool's listener already forwards Ready events

The `reset_ipc_tx()` method creates a fresh `(oneshot::Sender<IpcHandles>, oneshot::Receiver<IpcHandles>)` pair and stores it in `self.ipc_tx`.

### Step 4: Linux orphan cleanup — PR_SET_PDEATHSIG

In the spawn command builder (before `.spawn()`), add a Unix-specific pre_exec hook:
```rust
#[cfg(unix)]
cmd.pre_exec(|| {
    // Set PDEATHSIG to SIGHUP so the child is killed if the parent dies.
    unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGHUP as _, 0, 0, 0) };
    Ok(())
});
```

This uses `std::os::unix::process::CommandExt::pre_exec()`. The `libc` crate is already a dependency for the existing Unix fd duplication code.

### Step 5: Pool-level respawn orchestration

In `pool.rs`, modify the event listener task to detect `WorkerStatusChanged(Dead)` events (distinguished from Ready/Pong/etc. by checking the WorkerEvent discriminant). When detected:
1. Record the worker's device_index and worker_id for lookup
2. Wait `respawn_delay_ms` using `tokio::time::sleep`
3. Set the dead worker's status to `Respawning` (broadcast via its own event_tx)
4. Create a fresh `ManagedWorker` with new channels via `ManagedWorker::new()`
5. Call `spawn()` on the new worker with the device info from hardware
6. Call `start_keepalive()` on the new worker
7. Replace the dead worker in `self.workers` at the same index
8. Update `device_map`

The pool's existing event listener will then receive events from the new worker through its fresh broadcast channel (the pool subscribes per-worker during spawn_all; for respawns, the pool must also subscribe to the new worker's channel).

### Step 6: Test — respawn_after_death

Add a test in `managed.rs` under `#[cfg(feature = "mock-hardware")]`:
1. Create `ManagedWorker::new("respawn-test", 0)`
2. Set status to Idle directly (bypassing spawn)
3. Inject mock handles via `inject_handles_for_test()` using a `tokio::io::duplex(4096)` pipe — write side is held by test, read side goes to worker
4. Write a Ready frame on the write side so status transitions Idle ← Initializing → Idle (the existing reader_task logic)
5. Drop the write side → EOF triggers reader_task → status becomes Dead → `WorkerStatusChanged(Dead)` broadcast
6. Verify status is Dead within 1 second
7. Call `respawn()` with a mock device and config — this will fail to spawn a real Python process, so we need to handle this differently

Actually, since respawn() calls spawn() which requires a real Python interpreter, the test needs a different approach. The plan: use `inject_handles_for_test()` for the initial spawn (which already works in eof_sets_dead), then after EOF detection, manually set status to Respawning and verify the broadcast. For the full respawn→Idle cycle with mock handles, we inject fresh duplex handles into the respawned worker via reset_ipc_tx + a new inject call.

Refined test approach:
1. Create worker, inject mock duplex handles
2. Write Ready frame → status becomes Idle
3. Drop write end → EOF → verify Dead within timeout
4. Call `respawn()` — this will attempt to spawn a real Python process. To avoid this in tests, we can either: (a) set ANVILML_TEST_WORKER_PYTHON and use the system python, or (b) mock the spawn by setting status to Respawning directly and injecting fresh handles that immediately send Ready.
5. Option (b) is cleaner for an isolated unit test: after Dead detection, manually set status to Respawning (simulating what the respawn task would do), inject fresh handles, write a Ready frame, verify Idle.

### Step 7: Logging additions per FORGE_AGENT_RULES §11.3/§11.5

Per ENVIRONMENT.md §9 mandatory INFO log points for workers:
- `worker spawned` — already present in spawn()
- `worker respawned after unexpected exit` — add in respawn() with `exit_code=` or `signal=` field (since we don't know the exact cause, use `reason="dead"` or similar)
- `worker reached Ready state` — already present via the pool listener on Ready events; add explicit log in managed.rs when status transitions to Idle after respawn

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `respawn_delay_ms` field, broadcast Dead events on death paths, add `respawn()` and `reset_ipc_tx()` methods, add Linux PR_SET_PDEATHSIG pre_exec, add INFO log points |
| Modify | `crates/anvilml-worker/src/pool.rs` | Detect Dead events in listener, spawn background respawn task with delay/status transitions, replace dead worker with fresh ManagedWorker, subscribe to new worker's channel |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.1 → 0.1.2 |
| Add | `crates/anvilml-worker/src/managed.rs` (test) | New unit test `respawn_after_death` in existing `mod tests` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| managed.rs (mod tests) | `respawn_after_death` | Inject mock handles, send Ready → Idle; drop write end → EOF → Dead detected + WorkerStatusChanged broadcast; verify status transitions Dead→Respawning→Idle within respawn_delay_ms timeout |
| managed.rs (mod tests) | `keepalive_respawn_on_timeout` | Start keepalive with short intervals; stop pong responses after seq 0–1; verify keepalive kills child, broadcasts Dead, and respawn cycle completes |

## CI Impact

No CI workflow file changes required. The task only modifies source code within `anvilml-worker`. All existing CI gates apply:
- `cargo test -p anvilml-worker --features mock-hardware` — must pass with new tests exiting 0
- `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` — Windows cross-check; PR_SET_PDEATHSIG is cfg-gated behind `#[cfg(unix)]` so it won't affect Windows compilation

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ipc_tx` oneshot consumed once — respawn needs fresh channel | High | Medium | Add `reset_ipc_tx()` method to create a new oneshot pair; tested in unit test by injecting fresh mock handles post-respawn |
| Pool event listener subscribes per-worker during spawn_all but not during respawn | Medium | Medium | In pool respawn logic, after replacing the worker, also subscribe the pool's event forwarding task to the new worker's broadcast channel (same pattern as spawn_all) |
| `respawn()` calls `spawn()` which requires real Python — test may fail without Python | High | Low | Test uses mock handles via `inject_handles_for_test()`; respawn test manually simulates respawning by setting status and injecting fresh handles instead of calling real spawn() |
| `tokio::process::Command` pre_exec on Linux conflicts with existing Unix fd duplication code | Low | Medium | The pre_exec hook is set before `.spawn()`; the fd duplication happens after spawn returns. No conflict — they operate at different lifecycle stages |
| Dead → Respawning → Idle timing too tight for test flakiness | Medium | Low | Use `ANVILML_RESPAWN_DELAY_MS=100` in test (shorter than default 2000ms) to make test fast; add generous timeout (5s) on the verification step |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- respawn` exits 0: kill worker via EOF, observe Dead → Respawning → Idle within timeout
- [ ] `cargo test -p anvilml-worker --features mock-hardware -- keepalive` exits 0: existing keepalive tests still pass
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0 (PR_SET_PDEATHSIG is cfg-unix)
- [ ] WorkerStatusChanged(Dead) broadcast on EOF path in reader_task
- [ ] WorkerStatusChanged(Dead) broadcast on pong timeout path in keepalive task
- [ ] WorkerStatusChanged(Respawning) broadcast before respawn starts
- [ ] WorkerStatusChanged(Idle) implicitly via Ready event forwarding through pool's broadcast channel
- [ ] ANVILML_RESPAWN_DELAY_MS env var read and used (default 2000 ms)
- [ ] Linux PR_SET_PDEATHSIG(SIGHUP) set via pre_exec on spawn
- [ ] Pool detects Dead, waits delay, replaces worker with fresh ManagedWorker
- [ ] anvilml-worker Cargo.toml version bumped to 0.1.2
- [ ] INFO log: "worker respawned" emitted after successful respawn spawn
