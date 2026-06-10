# Plan Report: P18-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-A1                                      |
| Phase       | 018 — Worker Restart API & Preflight        |
| Description | anvilml-worker: WorkerPool.restart + shutdown_all |
| Depends on  | P17-A3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-10T17:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Add two lifecycle methods to `WorkerPool` in `crates/anvilml-worker/src/pool.rs`:
`restart(&self, worker_id)` — gracefully shut down a single worker, wait up to 5 s for it
to reach `Dying`, force-kill if needed, re-spawn, and re-send `InitializeHardware`; and
`shutdown_all(&self)` — send `Shutdown` to every worker, wait up to 10 s for all to reach
`Dying`, and force-kill stragglers.  Provide a public `ManagedWorker::restart` method that
handles the per-worker kill / respawn / re-init sequence.  Add tests exercising both paths
with mock hardware.

## Scope

### In Scope
- `ManagedWorker::restart(&self, device: &GpuDevice, cfg: &ServerConfig) -> Result<(), AnvilError>`
  — public method that: set status to `Respawning`, broadcast `WorkerStatusChanged(Respawning)`,
  send `Shutdown`, wait up to 5 s for `Dying`, force-kill, reset IPC, re-spawn, wait for
  `Idle`, broadcast `WorkerStatusChanged(Idle)`, return `Ok`.
- `WorkerPool::restart(&self, worker_id: &str, cfg: &ServerConfig) -> Result<(), AnvilError>`
  — find the worker by ID, look up its `device_index`, fetch the corresponding `GpuDevice`
  from `hardware_info()`, call `ManagedWorker::restart`, re-start keepalive + listener.
- `WorkerPool::shutdown_all(&self)`
  — iterate all workers, send `Shutdown`, collect `JoinHandle`s to await, wait up to 10 s,
  force-kill stragglers that are still alive.
- Tests in `pool.rs` — `restart` exits 0 and worker returns to `Idle`; `shutdown_all` stops all.
- Version bump: `anvilml-worker` `0.1.20 → 0.1.21`.
- Logging per §11.3–11.5 (worker lifecycle, IPC send, status transitions).

### Out of Scope
- HTTP handler for `POST /v1/workers/:id/restart` (task P18-A2).
- Graceful shutdown signal wiring (task P18-A4).
- Python preflight check (task P18-A3).
- Any changes to IPC framing, message enums, or the Python worker.

## Approach

### Step 1 — `ManagedWorker::restart` in `managed.rs`

Add a new `pub async fn restart` method to `ManagedWorker`:

```rust
/// Restart this worker: send Shutdown, wait for Dying, force-kill if needed,
/// re-spawn, and wait for Idle.
#[allow(clippy::too_many_arguments)]
pub async fn restart(
    &self,
    device: &GpuDevice,
    cfg: &ServerConfig,
) -> Result<(), AnvilError> {
    // 1. Set status to Respawning and broadcast.
    self.set_status(WorkerStatus::Respawning).await;
    let _ = self.event_tx.send((
        self.worker_id.clone(),
        WorkerEvent::WorkerStatusChanged {
            status: WorkerStatus::Respawning,
        },
    ));
    info!(worker_id = %self.worker_id, "worker restart initiated");

    // 2. Send Shutdown.
    let _ = self.tx.send(WorkerMessage::Shutdown).await;
    debug!(worker_id = %self.worker_id, message_type = "Shutdown", "sent shutdown for restart");

    // 3. Wait up to 5 s for Dying.
    let timeout = std::time::Duration::from_secs(5);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if self.get_status().await == WorkerStatus::Dead {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // 4. Force-kill if still alive.
    if self.get_status().await != WorkerStatus::Dead {
        warn!(worker_id = %self.worker_id, "worker did not die in 5s — force-killing");
        if let Some(mut ch) = self.child.lock().await.take() {
            let _ = ch.kill().await;
        }
        // Also abort the keepalive to stop pings.
        // (The generation counter handles invalidation; just set status.)
        let mut s = self.status.write().await;
        *s = WorkerStatus::Dead;
    }

    // 5. Reset IPC channels (new run_loop).
    self.reset_ipc_tx();

    // 6. Re-spawn (sends InitializeHardware, waits for Idle).
    self.spawn(device, cfg).await?;

    info!(worker_id = %self.worker_id, "worker restarted");
    Ok(())
}
```

Key design decisions:
- `restart` is `pub` and takes `device` + `cfg` because the pool needs to pass the device
  info (from `hardware_info()`) and the server config.  This matches the existing `spawn`
  signature.
- The method re-uses `spawn()` which already sends `InitializeHardware` and waits for
  `Idle` status (lines 297–336 of managed.rs).
- The 5-second timeout matches the task spec.
- Force-kill path sets status to `Dead` so the pool listener (if still running) sees the
  correct state.

### Step 2 — `WorkerPool::restart` in `pool.rs`

Add to `impl WorkerPool`:

```rust
/// Restart a specific worker: send Shutdown, wait for Dying, force-kill,
/// re-spawn, and re-send InitializeHardware.
pub async fn restart(
    &self,
    worker_id: &str,
    cfg: &ServerConfig,
) -> Result<(), anvilml_core::AnvilError> {
    let locked = self.workers.read().await;
    let worker = locked.iter().find(|w| w.worker_id() == worker_id).ok_or_else(|| {
        anvilml_core::AnvilError::WorkerDead(format!("worker not found: {worker_id}"))
    })?;
    let device_index = worker.device_index();
    drop(locked);

    // Look up the device from hardware info.
    let device = {
        let h = self.hardware.lock().await;
        h.gpus
            .iter()
            .find(|g| g.index == device_index)
            .cloned()
    }
    .ok_or_else(|| {
        anvilml_core::AnvilError::WorkerDead(format!("device index {device_index} not found"))
    })?;

    info!(worker_id = %worker_id, device_index, "restarting worker");

    // Call ManagedWorker::restart (handles shutdown, kill, respawn, init).
    worker.restart(&device, cfg).await?;

    // Re-start keepalive for the restarted worker.
    let _ka = worker.start_keepalive();

    info!(worker_id = %worker_id, "worker restarted successfully");
    Ok(())
}
```

Key design decisions:
- Acquires `workers` read lock to find the worker, then drops it before calling
  `restart()` (which may acquire write locks internally), avoiding deadlock.
- Fetches the `GpuDevice` from `hardware_info()` — this is the same device the worker was
  originally spawned with.
- Re-starts the keepalive after restart because the old keepalive may have been interrupted
  by the force-kill path.  The new keepalive gets a fresh generation counter.
- Does NOT re-spawn the listener task — the existing listener is still alive (it was waiting
  for events from the old worker).  The old listener will see the `Dying` event from the
  respawn path inside `spawn()`, but since we're not going through the Dead→respawn path,
  the listener stays.  After restart completes, the worker is back in Idle and the listener
  continues forwarding events normally.

### Step 3 — `WorkerPool::shutdown_all` in `pool.rs`

Add to `impl WorkerPool`:

```rust
/// Send Shutdown to all workers, wait up to 10 s for Dying, force-kill stragglers.
pub async fn shutdown_all(&self) {
    let locked = self.workers.read().await;
    let workers: Vec<Arc<ManagedWorker>> = locked.iter().cloned().collect();
    drop(locked);

    // Send Shutdown to each worker.
    for w in &workers {
        debug!(worker_id = %w.worker_id(), "sending shutdown");
        let _ = w.tx.send(WorkerMessage::Shutdown).await;
    }

    info!("shutdown_all: waiting for workers to exit");

    // Wait up to 10 s for all workers to reach Dead.
    let timeout = std::time::Duration::from_secs(10);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        let all_dead = {
            let locked = self.workers.read().await;
            locked.iter().all(|w| {
                matches!(w.get_status().await, WorkerStatus::Dead)
            })
        };
        if all_dead {
            info!("shutdown_all: all workers exited cleanly");
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Force-kill stragglers.
    for w in &workers {
        let status = w.get_status().await;
        if status != WorkerStatus::Dead {
            warn!(worker_id = %w.worker_id(), "force-killing straggler");
            if let Some(mut ch) = w.child.lock().await.take() {
                let _ = ch.kill().await;
            }
            let mut s = w.status.write().await;
            *s = WorkerStatus::Dead;
        }
    }

    info!("shutdown_all: completed (some workers were force-killed)");
}
```

Key design decisions:
- Collects worker references under a read lock, then drops the lock before sending
  shutdown messages (avoids holding the read lock while doing async I/O).
- Polls every 200 ms instead of a single `sleep(10s)` to allow early return when all
  workers have exited.
- Force-kill path mirrors the pattern in `restart`.
- No broadcast of `WorkerStatusChanged(Dead)` during force-kill — the listener tasks
  (spawned by `spawn_listener`) will have already been aborted or will see the dead
  child and broadcast naturally.

### Step 4 — Logging

Per §11.3 and §11.5:

| Method | Event | Level | Fields |
|--------|-------|-------|--------|
| `ManagedWorker::restart` | Restart initiated | INFO | `worker_id=` |
| `ManagedWorker::restart` | Shutdown sent (IPC) | DEBUG | `worker_id=`, `message_type=` |
| `ManagedWorker::restart` | Force-kill (timeout) | WARN | `worker_id=` |
| `ManagedWorker::restart` | Restart complete | INFO | `worker_id=` |
| `WorkerPool::restart` | Restart started | INFO | `worker_id=`, `device_index=` |
| `WorkerPool::restart` | Restart success | INFO | `worker_id=` |
| `WorkerPool::shutdown_all` | Shutdown per worker | DEBUG | `worker_id=` |
| `WorkerPool::shutdown_all` | All exited cleanly | INFO | — |
| `WorkerPool::shutdown_all` | Force-kill straggler | WARN | `worker_id=` |
| `WorkerPool::shutdown_all` | Shutdown complete | INFO | — |

### Step 5 — Tests

Add two tests to the `#[cfg(test)] mod tests` block in `pool.rs`:

**Test 1: `restart_exits_0_and_returns_to_idle`**
- Build a pool with one mock worker (using `new_test_pool_with_workers`).
- Set the worker status to `Idle`.
- Call `pool.restart("worker-0", &cfg)`.
- Assert the call returns `Ok(())`.
- Assert `worker.get_status().await == WorkerStatus::Idle`.
- Uses `ANVILML_WORKER_MOCK=1` via `temp_env::async_with_vars`.
- Skips if Python interpreter not found (same pattern as `spawn_ping_pong`).

**Test 2: `shutdown_all_stops_all`**
- Build a pool with two mock workers (device indices 0 and 1).
- Set both to `Idle`.
- Call `pool.shutdown_all()`.
- Assert both workers have status `Dead` after return.
- Uses `ANVILML_WORKER_MOCK=1` via `temp_env::async_with_vars`.
- Skips if Python interpreter not found.

Both tests follow the existing `venv_cfg_or_skip()` pattern for graceful skip when no
Python interpreter is available.

### Step 6 — Version Bump

Bump `crates/anvilml-worker/Cargo.toml`: `version = "0.1.20"` → `version = "0.1.21"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `pub async fn restart` method to `ManagedWorker` |
| Modify | `crates/anvilml-worker/src/pool.rs` | Add `pub async fn restart` and `pub async fn shutdown_all` to `WorkerPool`; add 2 tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.20 → 0.1.21` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `pool.rs` | `restart_exits_0_and_returns_to_idle` | Restart a mock worker → sends Shutdown, respawns, returns to Idle |
| `pool.rs` | `shutdown_all_stops_all` | Shutdown all workers → all reach Dead status within 10 s |

## CI Impact

No CI workflow files are modified.  The new tests run under the existing gate
`cargo test --workspace --features mock-hardware`.  The `anvilml-worker` crate's mock-hardware
feature already compiles with `mock-hardware` (no new dependencies).  No OpenAPI drift
(gate 2) or config drift (gate 1) is triggered.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ManagedWorker::spawn` waits for `Idle` status which requires a live Python worker; if the mock worker doesn't respond to `InitializeHardware`, the test hangs. | Medium | High | Use `timeout()` around the spawn call (5 s); skip test if Python not found via `venv_cfg_or_skip()`. |
| The existing event listener task for the restarted worker may receive stale events from the old lifecycle. | Low | Medium | The keepalive generation counter prevents stale pong-timeouts from acting. The listener only acts on `Ready` (merges capabilities) and `Dead` (triggers respawn). After restart, the new worker sends a fresh `Ready` which overwrites capabilities. |
| `shutdown_all` may deadlock if a worker's `Shutdown` send blocks on a full channel. | Low | Medium | Use `.send()` with `mpsc` which returns `Err` on full — we already do `let _ = w.tx.send(...)` so it's fire-and-forget. If the channel is full, the worker is already unresponsive and will be force-killed. |
| `restart` sets status to `Dead` during force-kill, but the pool listener may also try to set it. | Low | Low | Both paths set `WorkerStatus::Dead` — idempotent. The listener's Dead→respawn path won't trigger because we bypassed the `Dead` event broadcast. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- restart` exits 0: a mock worker is restarted and returns to `Idle`.
- [ ] `cargo test -p anvilml-worker --features mock-hardware -- shutdown_all` exits 0: all workers reach `Dead` status.
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes with zero warnings.
- [ ] `cargo fmt --all -- --check` passes with zero formatting drift.
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` passes (Windows cross-check).
- [ ] `anvilml-worker` crate version bumped to `0.1.21`.
- [ ] All new code paths include mandatory INFO/DEBUG/WARN log points per §11.3–11.5.
