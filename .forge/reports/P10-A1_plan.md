# Plan Report: P10-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-A1                                            |
| Phase       | 010 — Worker Crash Recovery                       |
| Description | anvilml-worker: keepalive Ping + Pong-timeout force-kill |
| Depends on  | P9-A6                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T14:35:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a per-worker keepalive watchdog to `ManagedWorker` in `anvilml-worker`: send `Ping{seq}` every 30 seconds over the existing IPC channel, expect a matching `Pong{seq}` response within 10 seconds, and force-kill the child process via `tokio::process::Child::kill()` on timeout. The ping interval and pong timeout must be configurable via `ANVILML_PING_INTERVAL_MS` and `ANVILML_PONG_TIMEOUT_MS` environment variables for testability.

## Scope

### In Scope
- **managed.rs**: Add `ping_interval`, `pong_timeout`, and `next_seq` fields to `ManagedWorker`. Implement a `start_keepalive()` method that spawns a `keepalive_task` async task. The keepalive task sends `Ping{seq}` at each interval tick, tracks pending pongs via a `HashMap<u64, tokio::time::Instant>`, and calls `child.kill()` when a pong times out.
- **managed.rs**: Add a `#[cfg(test)]` helper method `inject_handles_for_test()` that allows tests to bypass the Python process spawn and directly inject mock IPC handles into the worker.
- **pool.rs**: Read `ANVILML_PING_INTERVAL_MS` / `ANVILML_PONG_TIMEOUT_MS` from environment in `spawn_all()`, pass them to `ManagedWorker::new()`, and call `start_keepalive()` after each worker's `spawn()` returns.
- **managed.rs tests**: Add a test that creates a mock worker (no Python process), starts keepalive with short intervals, sends a few Pongs then stops, verifies the worker transitions to Dead within the timeout window.

### Out of Scope
- Respawn logic (P10-A2)
- WebSocket broadcasting of status changes (P10-A3)
- Worker PID accessor (P10-A4)
- Any changes to `anvilml-core`, `anvilml-ipc`, or Python worker code
- Changes to `Cargo.toml` dependency versions

## Approach

### Step 1 — Add configurable parameters to ManagedWorker (managed.rs)

Add three fields to the `ManagedWorker` struct:

```rust
/// Configurable ping interval (default: 30_000 ms).
ping_interval: std::time::Duration,
/// Configurable pong timeout (default: 10_000 ms).
pong_timeout: std::time::Duration,
/// Monotonically increasing sequence counter for Ping messages.
next_seq: u64,
```

Modify `ManagedWorker::new()` to accept these as parameters and read from environment variables with built-in defaults:

```rust
pub fn new(worker_id: String, device_index: u32) -> Self {
    // ... existing setup ...
    let ping_interval = std::env::var("ANVILML_PING_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or(std::time::Duration::from_secs(30));

    let pong_timeout = std::env::var("ANVILML_PONG_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(std::time::Duration::from_millis)
        .unwrap_or(std::time::Duration::from_secs(10));

    Self {
        // ... existing fields ...
        ping_interval,
        pong_timeout,
        next_seq: 0,
    }
}
```

### Step 2 — Implement the keepalive task (managed.rs)

Add a `start_keepalive()` method on `ManagedWorker` that spawns the watchdog task. This method is called **after** `spawn()` returns, when the child handle is stored and the worker is in Idle status:

```rust
pub fn start_keepalive(&self, child: &tokio::process::Child) {
    let child = child.id().map(tokio::process::Command::new); // No — use Arc<Mutex<>>
    // Actually: spawn using references to existing shared state
}
```

The actual implementation uses the shared fields already on `ManagedWorker`:

```rust
pub fn start_keepalive(&self, child_id: Option<u32>) {
    let ping_interval = self.ping_interval;
    let pong_timeout = self.pong_timeout;
    let worker_id = self.worker_id.clone();
    let tx = self.tx.clone(); // Clone the mpsc sender for keepalive sends
    let status = self.status.clone();
    let event_tx = self.event_tx.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(ping_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut next_seq: u64 = 0;
        let mut pending_pongs: std::collections::HashMap<u64, tokio::time::Instant> =
            std::collections::HashMap::new();

        loop {
            interval.tick().await;
            let seq = next_seq;
            next_seq += 1;

            debug!(worker_id = %worker_id, seq = seq, "sending ping");

            // Send Ping message. If channel is closed, worker is dead — exit.
            if tx.send(WorkerMessage::Ping { seq }).await.is_err() {
                warn!(worker_id = %worker_id, "keepalive: send failed, worker may be dead");
                break;
            }

            // Record the pending pong deadline.
            let deadline = tokio::time::Instant::now() + pong_timeout;
            pending_pongs.insert(seq, deadline);

            // Spawn a per-pong timeout task.
            let pw_id = worker_id.clone();
            let child_id = child_id;
            let status_clone = status.clone();
            tokio::spawn(async move {
                if tokio::time::timeout(pong_timeout, async {
                    // Wait for matching Pong via event channel subscription.
                    let mut rx = event_tx.subscribe();
                    loop {
                        match rx.recv().await {
                            Ok((_, WorkerEvent::Pong { seq: rseq })) => {
                                if rseq == seq {
                                    debug!(worker_id = %pw_id, seq = seq, "received pong");
                                    return; // Pong received — clear this pending entry.
                                }
                            }
                            Ok(_) => continue,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                })
                .await
                .is_err()
                {
                    // Timeout — force-kill the child.
                    warn!(worker_id = %pw_id, seq = seq, "pong timeout — killing worker");
                    if let Some(pid) = child_id {
                        if let Ok(mut cmd) = tokio::process::Command::new("kill") {
                            // Actually: use Child::kill() directly.
                            // We need the Child handle, not just the PID.
                        }
                    }
                    let mut s = status_clone.write().await;
                    *s = WorkerStatus::Dead;
                }
            });

            // Clean up expired pending pongs.
            pending_pongs.retain(|_, deadline| *deadline > tokio::time::Instant::now());
        }
    });
}
```

**Refined approach for child kill**: The keepalive task needs to call `Child::kill()`. The cleanest way is to store the `child` handle in an `Arc<Mutex<Option<tokio::process::Child>>>` that both the existing code (for EOF detection) and the keepalive task can access. However, since the current code already has `self.child: Mutex<Option<tokio::process::Child>>`, we can use that directly — just clone the Arc<Mutex<>> into the keepalive task.

Revised `start_keepalive()`:

```rust
pub fn start_keepalive(&self) {
    let ping_interval = self.ping_interval;
    let pong_timeout = self.pong_timeout;
    let worker_id = self.worker_id.clone();
    let tx = self.tx.clone();
    let status = self.status.clone();
    let event_tx = self.event_tx.clone();
    let child = self.child.clone(); // Arc<Mutex<Option<Child>>> clone

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(ping_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut next_seq: u64 = 0;

        loop {
            interval.tick().await;
            let seq = next_seq;
            next_seq += 1;

            debug!(worker_id = %worker_id, seq = seq, "sending ping");

            if tx.send(WorkerMessage::Ping { seq }).await.is_err() {
                warn!(worker_id = %worker_id, "keepalive: channel closed");
                break;
            }

            // Per-pong timeout: spawn a task that kills the child if no Pong{seq} arrives.
            let pw_id = worker_id.clone();
            let child_handle = child.clone();
            tokio::spawn(async move {
                let mut rx = event_tx.subscribe();
                let pong_received = tokio::time::timeout(pong_timeout, async {
                    loop {
                        match rx.recv().await {
                            Ok((_, WorkerEvent::Pong { seq: rseq })) if rseq == seq => return true,
                            Ok(_) => continue,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break false,
                        }
                    }
                })
                .await;

                match pong_received {
                    Ok(true) => { /* Pong received — nothing to do */ }
                    _ => {
                        warn!(worker_id = %pw_id, seq = seq, "pong timeout — killing worker");
                        // Force-kill the child process.
                        if let Some(mut ch) = child_handle.lock().await.take() {
                            if let Err(e) = ch.kill().await {
                                warn!(worker_id = %pw_id, error = %e, "failed to kill worker on pong timeout");
                            } else {
                                info!(worker_id = %pw_id, "killed worker on pong timeout");
                            }
                        }
                        let mut s = status.write().await;
                        *s = WorkerStatus::Dead;
                    }
                }
            });
        }

        debug!(worker_id = %worker_id, "keepalive task exiting");
    });
}
```

### Step 3 — Wire keepalive start in spawn_all() (pool.rs)

In `WorkerPool::spawn_all()`, after each worker's `spawn()` returns (which means the child is alive and Idle), call `start_keepalive()`:

```rust
// In spawn_all(), after:
worker.spawn(device, cfg).await.expect("spawn gpu worker");
workers.push(worker);

// Add:
if let Some(child) = /* need to get child PID or handle */ {
    worker.start_keepalive();
}
```

Since `start_keepalive()` now only needs the shared state already on `ManagedWorker` (it accesses `self.child` internally), it can be called without any additional parameters:

```rust
worker.spawn(device, cfg).await.expect("spawn gpu worker");
worker.start_keepalive(); // Start keepalive after spawn completes.
workers.push(worker);
```

### Step 4 — Test helper for mock workers (managed.rs)

Add a `#[cfg(test)]` method to allow tests to inject IPC handles without spawning a real Python process:

```rust
#[cfg(test)]
pub async fn inject_handles_for_test(&self, stdin: tokio::process::ChildStdin, stdout: tokio::process::ChildStdout) {
    let mut guard = self.ipc_tx.lock().await;
    if let Some(tx) = guard.take() {
        tx.send(IpcHandles { stdin, stdout }).expect("run_loop alive");
    }
}
```

This reuses the existing `ipc_tx` oneshot channel — when `inject_handles_for_test()` is called, it delivers handles to the run_loop just as `spawn()` does, but without spawning a real process.

### Step 5 — Keepalive test (managed.rs)

Add a new test function `keepalive_pings_and_kills_on_timeout`:

```rust
#[tokio::test]
#[cfg(feature = "mock-hardware")]
async fn keepalive_pings_and_kills_on_timeout() {
    // 1. Create worker with short intervals for fast testing.
    std::env::set_var("ANVILML_PING_INTERVAL_MS", "50");
    std::env::set_var("ANVILML_PONG_TIMEOUT_MS", "150");

    let worker = ManagedWorker::new("keepalive-test".to_string(), 0);

    // 2. Inject mock duplex handles (no Python process).
    let (mut tx, mut rx) = tokio::io::duplex(4096);
    // Write a Ready frame so the reader processes it and sets status to Idle.
    let ready_event = WorkerEvent::Ready { /* ... */ };
    write_ready_frame(&mut tx, &ready_event).await;
    worker.inject_handles_for_test(tx, rx).await;

    // 3. Wait for reader to process Ready → Idle.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(worker.get_status().await, WorkerStatus::Idle);

    // 4. Start keepalive (needs child handle — inject a dummy Child).
    let mut dummy_cmd = Command::new("true");
    let child = dummy_cmd.spawn().expect("spawn dummy");
    worker.start_keepalive(); // Uses self.child internally.

    // 5. Send 2 Pongs, then stop sending.
    send_pong(&worker, 0).await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    send_pong(&worker, 1).await;
    // No more pongs — seq=2 will time out at 150ms.

    // 6. Wait for timeout and verify worker is Dead.
    let result = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if worker.get_status().await == WorkerStatus::Dead { break; }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }).await;

    assert!(result.is_ok(), "worker should be Dead after pong timeout");
    assert_eq!(worker.get_status().await, WorkerStatus::Dead);

    // Cleanup.
    std::env::remove_var("ANVILML_PING_INTERVAL_MS");
    std::env::remove_var("ANVILML_PONG_TIMEOUT_MS");
}
```

The `send_pong()` helper serializes a `Pong{seq}` frame and writes it to the mock stdout pipe. The `write_ready_frame()` helper does the same for the initial Ready event.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `ping_interval`, `pong_timeout`, `next_seq` fields to `ManagedWorker`; add `start_keepalive()` method; add `#[cfg(test)] inject_handles_for_test()` helper; add keepalive test |
| Modify | `crates/anvilml-worker/src/pool.rs` | In `spawn_all()`, call `worker.start_keepalive()` after each `worker.spawn()` returns |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `managed.rs` (tests module) | `keepalive_pings_and_kills_on_timeout` | Mock worker with short intervals: sends Pongs for seq 0-1, stops; verifies worker transitions to Dead within pong timeout after seq=2 times out |

## CI Impact

No CI workflow file changes required. The test uses the existing `mock-hardware` feature flag and runs under `cargo test -p anvilml-worker --features mock-hardware -- keepalive`. The platform cross-check (`--target x86_64-pc-windows-gnu`) exercises the same code paths since `Child::kill()` is cross-platform via tokio.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tx.send(Ping)` in keepalive races with writer_task consuming from the same channel, causing duplicate sends or lost messages | Low | Medium — Ping sent twice would cause a spurious timeout. However, the reader task consumes sequentially and the mpsc ensures single-consumer ordering; the keepalive is the only other sender of WorkerMessage::Ping. | The keepalive task is the sole producer of `WorkerMessage::Ping`; the writer_task forwards messages from the same channel. No race condition — tokio mpsc guarantees at-most-once delivery per send. |
| `self.child.lock().await` in keepalive races with reader_task's EOF detection, causing double-kill | Low | Medium — Double kill is harmless (second kill returns error that is logged and ignored). | The keepalive takes the child handle via `.take()`, so only one task holds it. After take, the other task sees `None` and skips kill. |
| Test flakiness due to timing-sensitive pong timeout | Medium | Low — Test could fail intermittently if system is under load. | Use generous timeouts (150ms pong timeout vs 50ms ping interval gives 3x margin). The test waits up to 2s for Dead status, well beyond the 150ms timeout. |
| `tokio::process::Command::new("kill")` approach is wrong — need actual Child handle | Low | High — Would not compile or kill the process. | Use `self.child.clone()` (Arc<Mutex<>>) already on ManagedWorker; keepalive task takes ownership via `.take()`. No external command needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- keepalive` exits 0
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] The keepalive test verifies a mock worker transitions to Dead after pong timeout with no Python process running
- [ ] `ANVILML_PING_INTERVAL_MS` and `ANVILML_PONG_TIMEOUT_MS` environment variables are read in `ManagedWorker::new()` and override the 30s / 10s defaults
- [ ] The keepalive task sends `Ping{seq}` messages with monotonically increasing sequence numbers
- [ ] On pong timeout, the child process is killed via `tokio::process::Child::kill()` and status transitions to `Dead`
