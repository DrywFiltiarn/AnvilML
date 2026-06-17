# Plan Report: P901-A1

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P901-A1                                                     |
| Phase       | 901 — ManagedWorker Run-Loop and RespawnPolicy Retrofit     |
| Description | anvilml-worker: managed.rs fix run() to loop continuously instead of returning after one event |
| Depends on  | none                                                        |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-17T10:30:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Fix `ManagedWorker::run()` in `crates/anvilml-worker/src/managed.rs` so that it continues processing broadcast events for the worker's full lifetime instead of returning after the first event. The defect is that the existing `tokio::select! { ready_timeout, event_rx.recv() }` has no enclosing loop, so `run()` processes exactly one event then returns — dropping the bridge and keepalive handles — which means the supervisor never observes subsequent events (`Busy`, `Completed`, `Dying`, crash events) from a worker that has already reached `Idle`.

## Scope

### In Scope
- Wrap the existing `tokio::select!` block in `ManagedWorker::run()` with `loop { ... }`.
- Scope the `ready_timeout` branch to the `Initializing` state only: create a fresh `tokio::time::Sleep` inside the loop body each iteration, active only when `*status == Initializing`. Once the worker transitions to `Idle`, the timeout branch must not fire on any subsequent iteration.
- Break only on `Err(broadcast::error::RecvError::Closed)` — do not break after successfully processing an event.
- Add a mandatory DEBUG log call at the start of each loop iteration (per ENVIRONMENT.md §9.11.5, IPC log point: "event received from a worker").
- Bump `anvilml-worker` crate patch version from `0.1.7` to `0.1.8` (per ENVIRONMENT.md §12).

### Out of Scope
- Modifying any test file — test updates are P901-A2.
- Adding `child.wait()` or respawn logic — those are P10-A2/P10-A3.
- Adding new dependencies or modifying `Cargo.toml` beyond the version bump.
- Changing the `Shutdown` message, bridge module, or keepalive module.

## Existing Codebase Assessment

The `ManagedWorker::run()` method (lines 326–491 of `managed.rs`) currently implements a single `tokio::select!` with two arms: a 60-second `ready_timeout` sleep and `event_rx.recv()`. After the select completes (whichever arm wins), the function drops bridge/keepalive handles and returns. This means after processing exactly one event — or after the 60-second timeout fires — the supervision is torn down.

The state machine logic inside the event arm (lines 358–445) is correct and complete: it handles all valid transitions (`Initializing→Idle` on Ready, `Idle→Dead` on Dying, `Busy→Idle` on Completed/Failed/Cancelled, `Busy→Dead` on Dying, terminal states are no-ops). The device_name update from the Ready event also works correctly.

The existing tests in `managed.rs` (lines 538–713) and `crates/anvilml-worker/tests/managed_tests.rs` all follow the pattern: spawn `run()`, send one event, then `drop(event_tx)` to close the broadcast channel and let `run()` exit. This pattern was written to fit the single-iteration behaviour. After wrapping in a loop, the `drop(event_tx)` still correctly causes the loop to break on `RecvError::Closed`, so these tests should continue to work — but they do not prove the loop actually iterates.

The bridge module (`bridge.rs`) already uses a proper `loop { match transport.recv().await { ... } }` pattern for its reader task, demonstrating the established pattern in this codebase. The keepalive module (`keepalive.rs`) similarly uses nested loops for its heartbeat cycle. The `run()` function is the sole outlier.

No new external dependencies are needed. The task uses only existing tokio APIs: `tokio::time::sleep()`, `tokio::select!`, `tokio::sync::broadcast::Receiver::recv()`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | (workspace)     | Cargo.lock     | sync, time, process    |

No new external crates are introduced. The task only wraps existing code in a loop. The `tokio::time::sleep()` and `tokio::select!` APIs used are already present in the codebase and confirmed by reading `managed.rs` lines 336, 344. The `broadcast::error::RecvError::Closed` variant is from the existing `tokio::sync::broadcast` module, confirmed by reading `managed.rs` lines 456, 467.

## Approach

1. **Read and understand the existing `run()` method.** Confirm the single `tokio::select!` block at lines 344–476 and the cleanup code at lines 478–490. No changes needed to the event processing logic or cleanup — those are correct.

2. **Wrap the `tokio::select!` in `loop { ... }`.** Change the structure from:
   ```rust
   tokio::select! {
       _ = ready_timeout => { ... }
       result = event_rx.recv() => { ... }
   }
   ```
   to:
   ```rust
   loop {
       // ready_timeout scoped to Initializing only (step 3)
       let ready_timeout = if *self.status.read().await == WorkerStatus::Initializing {
           Some(tokio::time::sleep(Duration::from_secs(60)))
       } else {
           None
       };
       tokio::select! {
           _ = async { ready_timeout.as_mut().map(|s| s.as_mut()).unwrap_or_else(|| std::pin::pin!(tokio::time::sleep(std::time::Duration::MAX))).get_mut() }, ... }
   ```
   Actually, the cleanest approach is:

   ```rust
   loop {
       // Scope ready_timeout to Initializing only.
       // Once the worker is no longer Initializing, the timeout must not
       // fire — a worker sitting Idle for hours must not be killed by
       // a stale 60-second timer.
       let ready_timeout = if *self.status.read().await == WorkerStatus::Initializing {
           Some(tokio::time::sleep(Duration::from_secs(60)))
       } else {
           None
       };

       tokio::select! {
           _ = async {
               if let Some(ref mut sleep) = ready_timeout {
                   sleep.as_mut().await;
               } else {
                   // Never-firing sleep — this branch can never win.
                   // tokio::select! requires all arms to be futures,
                   // so we use a Duration::MAX sleep as a placeholder.
                   tokio::time::sleep(std::time::Duration::MAX).await;
               }
           } => {
               tracing::warn!(
                   worker_id = %self.worker_id,
                   "ready timeout, worker dead"
               );
               *self.status.write().await = WorkerStatus::Dead;
           }
           result = event_rx.recv() => {
               // existing event processing logic unchanged
           }
       }
   }
   ```

   **Rationale for `Duration::MAX` placeholder:** `tokio::select!` requires all arms to be futures with compatible types. When `ready_timeout` is `None`, we still need a valid future in that arm. `tokio::time::sleep(Duration::MAX)` creates a sleep that will never fire during any realistic test or production timeframe (≈292,471 years), serving as a safe no-op branch.

   **Rationale for `*self.status.read().await` before the select:** We need to know the current status *before* entering the select to decide whether to arm the timeout. Reading under a shared lock is cheap and non-blocking. If the status changes between the read and the timeout arm winning, the timeout branch still sets status to `Dead`, which is a no-op for any non-Initializing status (the state machine's terminal states are `Dead` and `Respawning`, and setting `Dead` when already `Dead` or `Respawning` is harmless — the state machine doesn't transition out of those states).

3. **Break only on `RecvError::Closed`.** The existing event processing match arm at line 456 already handles `Closed` correctly:
   ```rust
   Err(broadcast::error::RecvError::Closed) => {
       tracing::info!(worker_id = %self.worker_id, "broadcast channel closed, worker gone");
   }
   ```
   After the select completes (whether via timeout or event processing), the loop naturally continues. For the `Closed` case, we need to add an explicit `break` after logging:
   ```rust
   Err(broadcast::error::RecvError::Closed) => {
       tracing::info!(worker_id = %self.worker_id, "broadcast channel closed, worker gone");
       break; // bridge reader exited — no more events will arrive
   }
   ```
   **Rationale:** Without the `break`, the loop would continue and immediately re-enter the select, where `event_rx.recv()` would return `Closed` again (the channel is permanently closed). The `break` is the only exit path for a closed channel.

4. **Add mandatory DEBUG log at loop start.** Per ENVIRONMENT.md §9.11.5, the IPC subsystem requires a "Message sent to a worker" and "Event received from a worker" DEBUG log point. The existing code already logs at DEBUG when processing an event (line 364). Add a DEBUG log at the top of the loop to mark iteration:
   ```rust
   tracing::debug!(worker_id = %self.worker_id, "run loop iteration");
   ```

5. **Bump `anvilml-worker` version.** Edit `crates/anvilml-worker/Cargo.toml` line 3: change `version = "0.1.7"` to `version = "0.1.8"`. Per ENVIRONMENT.md §12, only the patch version changes.

6. **Verify no other code paths are affected.** The `run()` method is `pub async fn run(mut self)` — it consumes `self` and is called from `WorkerPool` (in `pool.rs`). Verify that `pool.rs` does not expect `run()` to return after one event (it shouldn't — the defect is precisely that it returns too early).

## Public API Surface

No new `pub` items are introduced. The only method affected is the existing:
- `pub async fn run(mut self)` in `ManagedWorker` — signature unchanged, behaviour corrected to loop continuously.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Wrap `tokio::select!` in `loop { ... }`; scope `ready_timeout` to Initializing state; add `break` on `RecvError::Closed`; add DEBUG log at loop start |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.7` → `0.1.8` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `test_spawned_task_updates_status` | Keepalive callback spawned task correctly updates status to Dead | Worker in Idle status | Callback invocation | Status becomes Dead | `cargo test -p anvilml-worker --features mock-hardware -- managed::tests::test_spawned_task_updates_status` exits 0 |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `test_managed_worker_processes_ready_event` | Ready event transitions Initializing → Idle | Worker in Initializing status | Ready event via broadcast channel | Status becomes Idle | `cargo test -p anvilml-worker --features mock-hardware -- managed::tests::test_managed_worker_processes_ready_event` exits 0 |
| `crates/anvilml-worker/src/managed.rs` (inline tests) | `test_managed_worker_processes_completed_event` | Completed event transitions Busy → Idle | Worker in Busy status | Completed event via broadcast channel | Status becomes Idle | `cargo test -p anvilml-worker --features mock-hardware -- managed::tests::test_managed_worker_processes_completed_event` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_spawn_reaches_idle` | Ready event transitions Initializing → Idle (integration) | Worker in Initializing status | Ready event via broadcast channel | Status becomes Idle | `cargo test -p anvilml-worker --features mock-hardware -- test_spawn_reaches_idle` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_ready_timeout_dead` | Ready event cancels timeout and transitions to Idle | Worker in Initializing status | Ready event via broadcast channel | Status becomes Idle | `cargo test -p anvilml-worker --features mock-hardware -- test_ready_timeout_dead` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_dying_event_transitions_dead` | Dying event transitions Idle → Dead | Worker in Idle status | Dying event via broadcast channel | Status becomes Dead | `cargo test -p anvilml-worker --features mock-hardware -- test_dying_event_transitions_dead` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_keepalive_timeout_sets_dead` | Keepalive timeout fires within expected window | Worker in Idle status, keepalive running | No Pong responses for 10+ seconds | Callback fires within 15s | `cargo test -p anvilml-worker --features mock-hardware -- test_keepalive_timeout_sets_dead` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_status_transitions_idle_to_busy_to_idle` | Busy→Idle transition on Completed event | Worker manually set to Busy status | Completed event via broadcast channel | Status becomes Idle | `cargo test -p anvilml-worker --features mock-hardware -- test_status_transitions_idle_to_busy_to_idle` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_shutdown_cleans_up_handles` | shutdown() drops all handles cleanly | Worker with bridge + keepalive handles | shutdown() call | No panic, handles dropped | `cargo test -p anvilml-worker --features mock-hardware -- test_shutdown_cleans_up_handles` exits 0 |

## CI Impact

No CI changes required. The task modifies only `managed.rs` (source) and `Cargo.toml` (version bump). The existing CI jobs (`rust-linux`, `rust-windows`) run `cargo test --workspace --features mock-hardware` which includes `anvilml-worker` tests. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The `tokio::time::sleep()` and `tokio::select!` macros are platform-neutral. The `broadcast::Receiver::recv()` method behaves identically on Linux and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ready_timeout` fires after worker reaches `Idle` because the `Duration::MAX` placeholder sleep in the timeout arm could theoretically be selected if both arms complete simultaneously. The status read happens *before* the select, so if status changed to Idle between the read and the select, the `Duration::MAX` branch is used — it cannot fire early. | Low | High | Verify the status read is performed immediately before the select (not in a prior iteration). Add a DEBUG log after the status read confirming the timeout state. |
| The `Duration::MAX` sleep in the non-timeout arm is a 292,471-year sleep. If both `event_rx.recv()` and `Duration::MAX` complete in the same tick (impossible in practice, but a theoretical concern), tokio::select! picks randomly. This means a `Closed` event could be lost if the timeout arm "wins" on the same tick. | Extremely Low | Low | The `Duration::MAX` arm sets status to `Dead`, which is harmless for non-Initializing workers. The next loop iteration will re-enter the select with only the `event_rx.recv()` branch active (since status is now `Dead`, the timeout is `None`). The `Closed` event will be detected on the next iteration. |
| Existing tests rely on `run()` returning after one event and may have timing assumptions that break with a continuous loop. For example, `test_keepalive_timeout_sets_dead` waits 15 seconds for the keepalive callback — with the loop, `run()` is still blocking on `event_rx.recv()`, so the timeout should still fire. | Medium | Medium | The existing tests all end with `drop(event_tx)` which closes the broadcast channel, causing the loop to break. No test expects `run()` to return early. Verify by running the full test suite after implementation. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-worker --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0
- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `grep -q "loop {" crates/anvilml-worker/src/managed.rs` confirms the loop wrapper is present
- [ ] `grep -q "0.1.8" crates/anvilml-worker/Cargo.toml` confirms version bump
