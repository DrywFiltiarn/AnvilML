# Plan Report: P8-E4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P8-E4                                         |
| Phase       | 008 — IPC Stress Gate & Worker Pool          |
| Description | anvilml-worker: ManagedWorker tracks crash attempt_history, consults RespawnPolicy |
| Depends on  | P8-E3, P8-D1                                  |
| Project     | anvilml                                       |
| Planned at  | 2026-07-01T15:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Wire the `RespawnPolicy` decision point into `ManagedWorker::run()`'s crash exit path so that `should_respawn()` is actually consulted when a worker crashes (transport recv error), rather than the policy sitting unused since P8-D1 was built. This task adds crash-attempt tracking via `attempt_history: Vec<Instant>` and logs the `should_respawn()` decision at INFO level. It does not implement the respawn loop itself — that is P8-E5's scope. The acceptance criterion is >=3 new tests in `tests/managed_tests.rs` verifying history growth, policy consultation, and log output, with `cargo test -p anvilml-worker --test managed_tests` exiting 0 (>=17 total tests).

## Scope

### In Scope
- Append `Instant::now()` to `ManagedWorker::attempt_history` on each crash (the `Err(e)` branch in `run()`'s event loop).
- Call `self.respawn_policy.should_respawn(&self.attempt_history)` on crash and log the boolean result at INFO level.
- Remove `#[allow(dead_code)]` from `respawn_policy` and `attempt_history` fields (they are now actively used).
- Add a `pub fn attempt_count(&self) -> usize` accessor on `ManagedWorker` so tests can verify the crash history grew.
- Add >=3 new integration tests in `tests/managed_tests.rs` exercising the crash-attempt tracking and policy consultation.

### Out of Scope
- Implementing the respawn loop (sleep, re-spawn subprocess, re-register, continue) — deferred to P8-E5.
- Modifying `WorkerHandle` status to `Respawning` during delay — deferred to P8-E5.
- Any changes to `RespawnPolicy` itself — already complete in P8-D1.
- Graceful shutdown or Initializing timeout paths — these remain unchanged from P8-E3; they do not append to `attempt_history` or call `should_respawn()`.
- `defers_to (from JSON): []` — this task has an empty defers_to field, so no Out of Scope bullet may defer real functionality to another task.

## Existing Codebase Assessment

**What already exists:** The `ManagedWorker` struct (in `managed.rs`) already contains both `respawn_policy: RespawnPolicy` and `attempt_history: Vec<Instant>` fields — they were scaffolded in P8-E3 but marked `#[allow(dead_code)]` and never populated or consulted. The `RespawnPolicy` type (in `respawn.rs`) is fully implemented with `should_respawn(&[Instant]) -> bool` and `next_delay() -> Duration`. The `run()` method has three exit paths (graceful shutdown via `shutdown_rx`, Initializing timeout, and crash via `transport.recv()` error), but only the crash path is relevant for this task. There are 14 existing tests in `managed_tests.rs` covering `WorkerHandle` semantics (9 tests) and `ManagedWorker::run()` lifecycle (5 tests).

**Established patterns:** Tests use in-process ZeroMQ ROUTER/DEALER pairs to simulate Python workers. The `connect_dealer()` helper sets up a DEALER socket connected to the transport's endpoint. Events are serialized via `rmp_serde::to_vec_named()` and sent through the ROUTER. Tests use bounded waits (tokio::select! with timeout) to avoid indefinite hangs. The `#[serial]` attribute from `serial_test` crate is used for tests that need sequential execution (the 60-second timeout test).

**Gap between design doc and source:** The design doc (`ANVILML_DESIGN.md §9.2`) states "a crashed worker is automatically respawned," but the source code never calls `should_respawn()` — this is the audit finding this task closes. The `attempt_history` field exists but is never appended to, and `respawn_policy` is never consulted. Both fields are `#[allow(dead_code)]` because nothing uses them yet.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| crate  | tokio     | 1.52.3          | Cargo.toml     | process, rt, sync, time |
| crate  | tracing   | 0.1             | Cargo.toml     | n/a                    |

No new external dependencies are introduced. `Instant`, `Duration`, and `Vec` are from the Rust standard library. The `RespawnPolicy` API (`should_respawn`, `next_delay`) was verified by reading `respawn.rs` directly — it matches the task context exactly.

## Approach

### Step 1: Add `pub fn attempt_count(&self) -> usize` accessor to `ManagedWorker`

In `managed.rs`, add a new public method on `impl ManagedWorker`:

```rust
/// Returns the number of crash attempts tracked in `attempt_history`.
///
/// Each call to `run()` appends an `Instant` on crash (transport recv error).
/// This accessor is primarily for testing — it lets callers verify that
/// crash-attempt tracking is working correctly without exposing the
/// internal `Vec<Instant>` directly.
pub fn attempt_count(&self) -> usize {
    self.attempt_history.len()
}
```

This is needed because `ManagedWorker` consumes `self` in `run()`, so the test cannot inspect `attempt_history` after `run()` returns. The accessor provides a read-only view.

### Step 2: Modify the crash exit path in `ManagedWorker::run()`

In the `Err(e)` branch of `run()`'s event loop (around line 280-285 of `managed.rs`), replace the current implementation:

**Before (current):**
```rust
Err(e) => {
    // Transport recv failed — this is a fatal error for the
    // managed worker. Log and break to exit path.
    tracing::error!(worker_id = %self.worker_id, error = %e, "transport recv failed");
    break;
}
```

**After (new):**
```rust
Err(e) => {
    // Transport recv failed — this is a fatal error for the
    // managed worker. Track the crash attempt and decide
    // whether a respawn is permissible.
    // P8-E5 will act on the decision by sleeping, re-spawning,
    // and continuing the loop instead of breaking.
    tracing::error!(worker_id = %self.worker_id, error = %e, "transport recv failed");
    // Record this crash attempt.
    self.attempt_history.push(Instant::now());
    // Consult the respawn policy — this is the decision point
    // that P8-D1 was built for but nothing had wired up yet.
    let should = self.respawn_policy.should_respawn(&self.attempt_history);
    tracing::info!(worker_id = %self.worker_id, should_respawn = should, "crash_respawn_decision");
    break;
}
```

Key details:
- `Instant::now()` is appended **only** on the crash path (transport `Err`), not on graceful shutdown or Initializing timeout. This matches the task specification.
- `should_respawn()` receives a slice reference to the full history — the policy internally filters by its trailing window.
- The INFO log uses structured field notation (`should_respawn = should`) per ENVIRONMENT.md §9's structured field discipline.
- The `break` is preserved — the actual respawn-and-continue logic is P8-E5's scope.

### Step 3: Remove `#[allow(dead_code)]` from `respawn_policy` and `attempt_history`

Remove the `#[allow(dead_code)]` attributes from both fields in the `ManagedWorker` struct definition (lines 184 and 191), since they are now actively used by the code in Step 2.

### Step 4: Add >=3 new integration tests in `tests/managed_tests.rs`

Add three new tests at the end of the file (following the existing pattern of using ZeroMQ ROUTER/DEALER pairs):

**Test 1: `test_crash_appends_to_attempt_history`**
- Creates a ROUTER/DEALER pair, sends a `Ready` event to transition to Idle, then causes a transport error by closing the DEALER socket (which forces `recv()` to fail on the next iteration).
- Spawns `run()`, waits for completion, then calls `attempt_count()` to verify exactly 1 crash was recorded.

**Test 2: `test_crash_history_grows_per_crash`**
- Creates a ROUTER/DEALER pair, sends `Ready`, then causes multiple transport errors by repeatedly closing and reconnecting the DEALER.
- Verifies `attempt_count()` equals the number of crashes.

**Test 3: `test_should_respawn_called_on_crash`**
- Creates a ROUTER/DEALER pair with a policy configured to allow respawns (e.g., 10 max attempts), sends `Ready`, causes a crash, and verifies that `attempt_count()` returns 1 and that the INFO log `crash_respawn_decision` appears in the output.
- The test confirms the decision point is wired by checking both the history count and the log output.

Each test follows the existing pattern: construct `ManagedWorker` with a `RouterTransport`, connect a `DealerSocket` as the simulated worker, use `rmp_serde::to_vec_named()` for event serialization, and use bounded waits via `tokio::select!`.

## Public API Surface

### New public item on `ManagedWorker`:

```rust
// crates/anvilml-worker/src/managed.rs
impl ManagedWorker {
    /// Returns the number of crash attempts tracked in `attempt_history`.
    pub fn attempt_count(&self) -> usize { ... }
}
```

No new `pub` structs, enums, or traits are introduced. No changes to existing `pub` signatures.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `attempt_count()` accessor; modify crash exit path to append `Instant` and call `should_respawn()`; remove `#[allow(dead_code)]` |
| Modify | `crates/anvilml-worker/tests/managed_tests.rs` | Add >=3 new integration tests for crash-attempt tracking |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_crash_appends_to_attempt_history` | A single transport error causes `attempt_count()` to return 1 | `cargo test -p anvilml-worker --test managed_tests -- test_crash_appends_to_attempt_history` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_crash_history_grows_per_crash` | Multiple transport errors each append to history; `attempt_count()` equals crash count | `cargo test -p anvilml-worker --test managed_tests -- test_crash_history_grows_per_crash` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_should_respawn_called_on_crash` | On crash, `should_respawn()` is consulted and the INFO log `crash_respawn_decision` appears; history has correct length | `cargo test -p anvilml-worker --test managed_tests -- test_should_respawn_called_on_crash` exits 0 |

Total tests in `managed_tests.rs` after this task: 17 (14 existing + 3 new), satisfying the >=16 requirement.

## CI Impact

No CI changes required. The task only modifies existing test files and source code within the `anvilml-worker` crate. The existing CI job `rust-linux` runs `cargo test --workspace --features mock-hardware`, which includes `anvilml-worker` tests. No new test file types, new gates, or new test modules are introduced.

## Platform Considerations

None identified. The crash-attempt tracking logic is pure Rust — it uses `Instant::now()` and `Vec<Instant>` which are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Dropping the DEALER socket to trigger a transport error may not cause `recv()` to return `Err` — it might instead block or return a different error type depending on ZeroMQ's connection state. | Medium | High | Use a bounded-wait test with `tokio::select!` to confirm the worker exits within a reasonable timeout. If dropping the DEALER doesn't trigger `Err(e)`, send a malformed msgpack payload (invalid bytes) instead, which will cause `rmp_serde::from_slice` to fail with `IpcError::RecvFailed`. |
| The INFO log `crash_respawn_decision` cannot be verified programmatically since tests don't capture tracing output. The test can only verify the code path by checking `attempt_count()` and the history length. | Low | Low | This is acceptable — the log is a side effect, not a behavioral contract. The `attempt_count()` verification proves the code path executes. The log output is verified manually during code review. |
| Modifying `run()`'s crash path may affect existing tests if they rely on the old behavior (e.g., `test_deregister_called_on_crash` uses the Dying event path, not the transport error path, so it's unaffected). | Low | Medium | The existing `test_deregister_called_on_crash` test sends a `Dying` event (not a transport error), so it follows the `WorkerEvent::Dying` match arm, not the `Err(e)` path. No existing test is affected. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test managed_tests` exits 0
- [ ] `grep -c 'async fn test_' crates/anvilml-worker/tests/managed_tests.rs` outputs >=17
- [ ] `cargo clippy -p anvilml-worker -- -D warnings` exits 0
- [ ] `grep 'crash_respawn_decision' crates/anvilml-worker/src/managed.rs` finds the new log call
- [ ] `grep 'should_respawn' crates/anvilml-worker/src/managed.rs` finds the policy consultation call
- [ ] `grep '#\[allow(dead_code)\]' crates/anvilml-worker/src/managed.rs` finds 0 results (both removed)
