# Plan Report: P10-A2

| Field       | Value                                                                        |
|-------------|------------------------------------------------------------------------------|
| Task ID     | P10-A2                                                                       |
| Phase       | 010 — Worker Crash Recovery                                                  |
| Description | anvilml-worker/server: complete crash detection, respawn cycle, and operator restart |
| Depends on  | P901-A3, P10-A1                                                              |
| Project     | anvilml                                                                      |
| Planned at  | 2026-06-18T12:00:00Z                                                         |
| Attempt     | 2                                                                            |

## Objective

Implement the complete Phase 010 worker crash recovery surface across `anvilml-worker` and
`anvilml-server`: automatic crash detection (subprocess exit via `child.wait()`), automatic
heartbeat-timeout detection (no pong within `pong_timeout`), the full respawn cycle
(`Dead → Respawning → Initializing → Idle`) gated by `RespawnPolicy`, and an operator-
initiated restart endpoint (`POST /v1/workers/:id/restart`) that force-kills and respawns
unconditionally. When complete: killing a Python worker process in the OS task manager
triggers an immediate respawn; `POST /v1/workers/worker-0/restart` returns 202 and the
worker returns to Idle within its normal initialisation window; `cargo test --workspace
--features mock-hardware` exits 0 with 12 passing managed tests.

This task supersedes the earlier separate P10-A2 (crash detection only) and the planned
P10-A3 (respawn cycle) and P10-B1 (restart endpoint), consolidating all three into one
coherent implementation. The split was originally motivated by a desire to verify each step
independently, but the interdependencies between crash detection, respawn, and the restart
endpoint (all touching the same `ManagedWorker::run()` loop and `WorkerPool` surface) made
a single coordinated implementation more reliable in practice.

## Scope

### In Scope

- `ManagedWorker` struct: add `crash_count: u32`, `last_crash: Instant`, `cfg: ServerConfig`,
  `device: GpuDevice`, `transport: Arc<RouterTransport>`, `timeout_rx: oneshot::Receiver<()>`,
  `restart_rx: watch::Receiver<u64>`, and change `event_tx: broadcast::Sender` to
  `event_tx: Option<broadcast::Sender>` (required to avoid E0382 partial-move error)
- `ManagedWorker::new()`: add the seven new fields as parameters; wrap `event_tx` in `Some`
- `ManagedWorker::spawn()`: add `restart_rx: watch::Receiver<u64>` parameter; populate the
  three owned fields (`cfg`, `device`, `transport`); build `timeout_tx`/`timeout_rx` oneshot
  for keepalive-timeout signalling; replace the `Weak<status>` `on_timeout` closure with one
  that only fires `timeout_tx` (removing the direct status write that bypassed `run()`);
  return `Self` (not a tuple — `restart_rx` is passed in by the caller, not returned)
- `ManagedWorker::do_respawn(&mut self, consult_policy: bool)`: new private async method;
  handles `RespawnPolicy` consultation and backoff (when `consult_policy` is true), sets
  `Respawning`, calls `Self::spawn()`, overwrites `*self = new_worker`, re-subscribes
  `event_rx`; returns `Err` if `routes` is `None` (test-only path) or attempts are exhausted
- `ManagedWorker::run()`: five arms in `select!`: ready-timeout (unchanged), `event_rx.recv()`
  (unchanged), child-exit (calls `do_respawn(true)` after Dead), heartbeat-timeout (kills
  child, calls `do_respawn(true)`), manual-restart (kills child, calls `do_respawn(false)`);
  use `loop_child` local per iteration to avoid borrow conflict with `self.timeout_rx` in
  same `select!`; use `self.event_tx.take()` (not `mem::replace`) to avoid partial-move
- `WorkerPool`: add `restart_tx: watch::Sender<u64>` to `WorkerHandle`; build the
  `watch::channel(0u64)` pair in `spawn_all()` and pass `restart_rx` into `ManagedWorker::spawn()`;
  new `pub async fn restart_worker(&self, worker_id: &str) -> Result<(), AnvilError>` that
  increments the generation counter via `send_modify`
- `anvilml-server`: add `restart_worker` handler (`POST /v1/workers/{id}/restart`, returns
  202 or 404/500/503 via `AnvilError::IntoResponse`); register route in `build_router`
- All test files updated for the new `ManagedWorker::new()` 18-argument signature:
  `managed_tests.rs`, `pool_tests.rs`, `workers_tests.rs`
- New tests: `test_child_exit_transitions_dead` (updated assertion: `Dead || Respawning`),
  `test_respawn_cycle_entered_after_child_exit` (asserts `Dead || Respawning` within 9s)
- Version bumps: `anvilml-worker` 0.1.20 → 0.1.21; `anvilml-server` 0.1.17 → 0.1.18

### Out of Scope

- In-flight job failure notification — `JobScheduler` does not exist until P13-A3; the
  existing `// TODO(P14)` comment at the Dead transition site is unchanged
- Updating the `WorkerPool` background status-monitor to observe the new worker's status
  `Arc` after a respawn — the monitor captures the original `Arc` at spawn time; after
  `*self = new_worker` the monitor's copy freezes at `Dead`. This is a known scoped
  limitation; flagged in `do_respawn`'s inline comment for Phase 11 follow-up
- Python worker integration tests — the respawn cycle terminates with `Err` in test
  environments (no Python venv, no `RouteTable`); the test assertions are scoped to
  observing the `Dead`/`Respawning` transition, not the full `Idle` recovery

## Existing Codebase Assessment

At the start of this task `ManagedWorker::run()` already has a continuous `loop { select! }`
with four arms (ready-timeout, event, child-wait, shutdown), and `RespawnPolicy` is fully
implemented with the correct `&mut crash_count` signature (Phase 901). The child-wait arm
sets `Dead` unconditionally and `break`s — no respawn logic exists yet. `WorkerPool` stores
per-worker `status`, `shutdown_tx`, and `run_handle` but has no restart mechanism.

The `on_timeout` callback in `spawn()` captures a `Weak<status>` and writes `Dead` from a
detached task, completely bypassing `run()`'s select loop — this is replaced in this task
with a `oneshot::Sender<()>` that signals `run()`'s new heartbeat-timeout arm, so all state
transitions go through one controlled code path.

The `event_tx` field is `broadcast::Sender` (non-Option), and `run()` does
`drop(self.event_tx)` — a partial move that prevents `do_respawn(&mut self)` from compiling
(E0382). Wrapping the field in `Option<broadcast::Sender>` and using `.take()` resolves this
without changing any observable channel behaviour.

The child-wait arm's `async { match self.child.as_mut() { Some(c) => c.wait().await } }` is
recreated on every loop iteration. Diagnostic tests confirm this is cancel-safe on Windows
(the exit handle remains signalled), but the `self.child` borrow conflicts with the new
`self.timeout_rx` arm when added in the same `select!`. The fix: take `self.child` into
a `loop_child` local before each `select!` and restore it after, eliminating the borrow
overlap while keeping the per-iteration async block pattern that is proven to work.

## Resolved Dependencies

None. All required types (`tokio::sync::watch`, `tokio::sync::oneshot`, `broadcast::Sender`)
are already in the workspace tokio dependency (`full` feature set). No new crates added.

| Type   | Name  | Version verified | MCP source   | Feature flags confirmed |
|--------|-------|-----------------|--------------|------------------------|
| (none) | —     | —               | —            | —                      |

## Approach

1. **Change `event_tx` to `Option<broadcast::Sender>`** in the struct definition. Update
   both `new()` and `spawn()` struct literals to wrap with `Some(event_tx)`. Update
   `run()` to call `.as_ref().expect(...).subscribe()` before `.take()`. Update
   `do_respawn()` to call `.as_ref().expect(...).subscribe()` then `.take()` after
   `*self = new_worker`.

2. **Add seven new fields to `ManagedWorker`**: `crash_count: u32` (init 0), `last_crash:
   Instant` (init `Instant::now()`), `cfg: ServerConfig` (cloned from `spawn()` arg),
   `device: GpuDevice` (cloned), `transport: Arc<RouterTransport>` (cloned), `timeout_rx:
   oneshot::Receiver<()>`, `restart_rx: watch::Receiver<u64>`.

3. **Rewrite `spawn()`'s `on_timeout` closure**: remove the `Weak<status>` capture and
   detached task. Replace with `Arc<Mutex<Option<oneshot::Sender<()>>>>` (required because
   `keepalive::start` requires `Fn()`, not `FnMut()` — `Option::take()` needs `&mut`,
   so interior mutability via `Mutex` is the minimal correct solution). The closure
   acquires the lock, takes the sender, and calls `sender.send(())`.

4. **Add `restart_rx: watch::Receiver<u64>` parameter to `spawn()`**. The caller
   (`WorkerPool::spawn_all`) builds the `watch::channel(0u64)` pair and passes the
   receiver. The `watch::Receiver` is `Clone`, so `do_respawn()` passes
   `self.restart_rx.clone()` to each successive `Self::spawn()` call — the same sender in
   `WorkerHandle` remains valid across all respawns without replacement.

5. **Implement `do_respawn(&mut self, consult_policy: bool)`**:
   - If `consult_policy`: call `self.respawn_policy.should_respawn(&mut self.crash_count,
     self.last_crash)`. Return `Err(Internal(...))` if false. Update `self.last_crash`,
     compute `delay_ms = next_delay_ms(crash_count - 1)`, set `Respawning`, sleep.
   - If not `consult_policy`: set `Respawning` immediately.
   - Check `self.routes.clone()`: return `Err(Internal(...))` if `None` (test-only path,
     no RouteTable to register into).
   - Call `Self::spawn()`, subscribe new `event_rx`, assign `*self = new_worker`, call
     `self.event_tx.take()`. Return `Ok(new_event_rx)`.

6. **Extend `run()`'s `select!` with two new arms**:
   - Heartbeat-timeout: `_ = &mut self.timeout_rx`. Abort keepalive, kill `loop_child`,
     set `Dead`, call `do_respawn(true)`. `continue` on `Ok`, `break` on `Err`.
   - Manual-restart: `changed_result = async { ... self.restart_rx.changed().await }`.
     On `Err` (sender dropped), set `restart_rx_closed = true` and `continue`.
     On `Ok`: shutdown keepalive, kill `loop_child`, set `Dead`, call `do_respawn(false)`.
   - Extract `self.timeout_rx` and `self.restart_rx` as named locals (`let timeout_rx =
     &mut self.timeout_rx`) before `select!` to prevent borrow conflict with `loop_child`
     inside the same `select!` expansion.
   - Before each `select!`: `let mut loop_child = self.child.take()`. After: `self.child =
     loop_child`. Arm bodies that consume the child (kill ops) use `loop_child.take()`.
   - Child-exit arm: replace unconditional `break` with `do_respawn(true)`.

7. **Update `WorkerPool`**: add `restart_tx: watch::Sender<u64>` to `WorkerHandle`. Build
   pair in `spawn_all()`, pass `restart_rx` to `ManagedWorker::spawn()`. Add
   `restart_worker(&self, worker_id: &str) -> Result<(), AnvilError>`: find handle, call
   `send_modify(|g| *g += 1)`, return `Err(WorkerNotFound)` if absent or
   `Err(Internal(...))` if `is_closed()`.

8. **Add `restart_worker` handler** in `handlers/workers.rs`. Signature:
   `restart_worker(State<AppState>, Path<String>) -> Result<StatusCode, AnvilError>`.
   Returns `Ok(StatusCode::ACCEPTED)` or propagates `AnvilError` via existing
   `IntoResponse`. Register `POST /v1/workers/{id}/restart` in `build_router`.

9. **Update all test files** (`managed_tests.rs`, `pool_tests.rs`, `workers_tests.rs`)
   for the new 18-argument `ManagedWorker::new()` signature. Add stub helpers
   `stub_cfg()`, `stub_device()`, `stub_transport()`, `stub_timeout_pair()`,
   `stub_restart_pair()` in `managed_tests.rs`.

10. **Test assertions**: `test_child_exit_transitions_dead` polls for `Dead || Respawning`
    because `Dead` is overwritten immediately by `Respawning` before the polling task
    is scheduled on the single-threaded tokio test runtime. Same for
    `test_respawn_cycle_entered_after_child_exit` — single combined wait.

## Public API Surface

```rust
// crates/anvilml-worker/src/managed.rs

pub struct ManagedWorker {
    // ... existing fields ...
    event_tx: Option<broadcast::Sender<(String, WorkerEvent)>>,  // was broadcast::Sender
    crash_count: u32,                                             // NEW
    last_crash: Instant,                                          // NEW
    cfg: ServerConfig,                                            // NEW
    device: GpuDevice,                                            // NEW
    transport: Arc<RouterTransport>,                              // NEW
    timeout_rx: oneshot::Receiver<()>,                            // NEW
    restart_rx: tokio::sync::watch::Receiver<u64>,               // NEW
}

// new() gains 5 new parameters (cfg, device, transport, timeout_rx, restart_rx)
// spawn() gains 1 new parameter: restart_rx: watch::Receiver<u64>

// crates/anvilml-worker/src/pool.rs

impl WorkerPool {
    pub async fn restart_worker(&self, worker_id: &str) -> Result<(), AnvilError>;
}

// crates/anvilml-server/src/handlers/workers.rs

pub async fn restart_worker(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AnvilError>;
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/src/managed.rs` | Add 7 fields, rewrite on_timeout, add do_respawn, extend select!, loop_child pattern, event_tx Option |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Add restart_tx to WorkerHandle, pass restart_rx to spawn(), add restart_worker() |
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | Update new() signature, add stub helpers, new tests |
| MODIFY | `crates/anvilml-worker/tests/pool_tests.rs` | Update new() signature, async make_test_worker |
| MODIFY | `crates/anvilml-server/src/handlers/workers.rs` | Add restart_worker handler |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Register POST /v1/workers/{id}/restart |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | Update new() signature, add stub helpers |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.20 → 0.1.21 |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.17 → 0.1.18 |
| MODIFY | `docs/TESTS.md` | Add entries for test_respawn_cycle_entered_after_child_exit and restart_worker |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_child_exit_transitions_dead` | Child subprocess exit transitions status to Dead (or Respawning if do_respawn fires before polling task is scheduled) | Worker built via new() with real child process | Child exits after ~0.5s | Status is Dead or Respawning within 5s | `cargo test -p anvilml-worker --features mock-hardware -- test_child_exit_transitions_dead` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_respawn_cycle_entered_after_child_exit` | Child exit triggers do_respawn; status reaches Dead or Respawning proving the respawn cycle was entered | Worker built via new() with real child, no RouteTable (do_respawn returns Err after Respawning) | Child exits after ~0.5s | Status is Dead or Respawning within 9s | `cargo test -p anvilml-worker --features mock-hardware -- test_respawn_cycle_entered_after_child_exit` exits 0 |

## CI Impact

No CI changes required. The new test files are picked up by the existing `cargo test
--workspace --features mock-hardware` command on all four runners.

## Platform Considerations

- `ping -n 1 -w 500 127.0.0.1` used as a cross-platform short-lived child on Windows;
  `sh -c "sleep 0.5 && exit 1"` on Unix. Both are guarded with `#[cfg(windows)]` /
  `#[cfg(not(windows))]`.
- The child-wait async block is per-iteration (recreated each loop iteration). This is
  cancel-safe on Windows because the process-exit handle remains signalled after the child
  exits, so re-polling resolves immediately. Diagnostic tests `test_child_wait_resolves_directly`
  and `test_child_wait_in_minimal_select` confirm this behaviour.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `Dead` status overwritten by `Respawning` before test polling task is scheduled on single-threaded runtime | High | Medium | Test assertions accept `Dead \|\| Respawning`; both states prove crash detection fired correctly |
| `loop_child` borrow conflict with `self.timeout_rx`/`self.restart_rx` in same `select!` if Rust borrow checker treats disjoint field borrows as conflicting | Medium | High | Take `self.child` into `loop_child` local before each `select!`, eliminating `self` borrows from the child-wait arm |
| `watch::Receiver::changed()` on a closed channel resolves immediately, spinning the restart arm | Medium | High | `restart_rx_closed` flag replaces the arm expression with `std::future::pending()` after first `Err` |

## Acceptance Criteria

- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 with 12 managed tests passing
- [ ] `cargo run` starts the server and killing a Python worker in the OS task manager triggers an immediate respawn (observable via `GET /v1/workers` transitioning through Respawning back to Idle)
- [ ] `curl -X POST http://localhost:8488/v1/workers/worker-0/restart` returns HTTP 202