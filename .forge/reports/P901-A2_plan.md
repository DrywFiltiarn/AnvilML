# Plan Report: P901-A2

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P901-A2                                                     |
| Phase       | 901 — ManagedWorker Run-Loop and RespawnPolicy Retrofit     |
| Description | Update managed_tests.rs to add test proving run() loops continuously |
| Depends on  | P901-A1                                                     |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-17T13:05:00Z                                        |
| Attempt     | 1                                                           |

## Objective

Add a single test, `test_run_processes_multiple_sequential_events`, to `crates/anvilml-worker/tests/managed_tests.rs` that proves `ManagedWorker::run()` processes more than one event per invocation. The existing tests send exactly one event then `drop(event_tx)` to force `run()` to exit — a pattern that fits both the old one-shot `select!` and the new continuous loop. This new test sends two sequential events on a single `run()` call and asserts both status transitions are observed, which only passes if the loop is real.

## Scope

### In Scope
- `crates/anvilml-worker/tests/managed_tests.rs` — append `test_run_processes_multiple_sequential_events` test function

### Out of Scope
- Any modification to `managed.rs` (that is P901-A1's scope)
- Any modification to other test files
- Any modification to Cargo.toml or other manifest files
- Any modification to `docs/TESTS.md` (ACT obligation, not PLAN)
- Version bumping (no source files are modified)

## Existing Codebase Assessment

The `run()` function in `managed.rs` has been wrapped in a `loop { ... }` by P901-A1 (lines 344–515). The loop breaks only on `Err(RecvError::Closed)` from the broadcast channel. The `ready_timeout` is scoped to the `Initializing` state only — it is re-evaluated each iteration and becomes a no-op once the worker reaches `Idle`.

The existing test suite in `managed_tests.rs` (409 lines, 6 tests) follows a consistent pattern: create a worker via `make_test_worker()`, spawn `run()`, send one event via `event_tx`, sleep 50–500ms, assert status, then `drop(event_tx)` to exit the loop. The `test_status_transitions_idle_to_busy_to_idle` test (line 277) already demonstrates the Idle→Busy→Idle transition pattern by manually setting Busy before sending a Completed event — this is the exact pattern the new test will build on.

No external crates or API shapes are introduced. The test uses only `anvilml_core::WorkerStatus`, `anvilml_ipc::WorkerEvent`, `tokio::sync::{broadcast, mpsc}`, and `tokio::time::timeout` — all already imported in the file.

## Resolved Dependencies

None. This task introduces no new dependencies, feature flags, or external crate references. All types and functions used by the new test are already available through existing imports.

## Approach

1. **Append the test function** to `crates/anvilml-worker/tests/managed_tests.rs` (after line 409, before the file's implicit end).

2. **Test structure — two sequential events on one `run()` invocation:**

   a. Create a worker in `Initializing` state using `make_test_worker(WorkerStatus::Initializing, "test-worker-multi", "test-device")`.
   
   b. Clone the status Arc via `worker.get_status()`.
   
   c. Spawn `run()` via `tokio::spawn(worker.run())`.
   
   d. Sleep 50ms to let run() subscribe and enter the select loop.
   
   e. Send a `Ready` event through `event_tx`. This triggers the `Initializing → Idle` transition.
   
   f. Sleep 200ms for the event to be processed.
   
   g. Assert status is `Idle`.
   
   h. Manually set status to `Busy` (simulating a job dispatch from the scheduler).
   
   i. Sleep 50ms for the write to propagate.
   
   j. Send a `Completed` event through `event_tx`. This triggers the `Busy → Idle` transition.
   
   k. Sleep 500ms for the second event to be processed.
   
   l. Assert status is `Idle` again.
   
   m. Drop `event_tx` to close the broadcast channel.
   
   n. Await `run_handle` with a 10-second timeout.

3. **Rationale for two-event pattern:** The first event (Ready) proves the initial transition works (which existing tests already cover). The second event (Completed after manual Busy) is the critical proof: if `run()` had exited after processing the first event, the second event would never be received and the final status would remain `Busy`. Asserting `Idle` at the end proves the loop continued processing.

4. **Rationale for starting in `Initializing`:** Starting from `Initializing` exercises the full Ready→Idle transition path (which is the most critical path in the state machine), then transitions to Busy manually and verifies the second event. This is more comprehensive than starting from `Idle` because it covers both the Ready timeout scoping (the timeout disarms after Ready) and the subsequent loop continuity.

5. **Rationale for `tokio::time::timeout` on `run_handle.await`:** The 10-second timeout protects against an infinite hang if the loop somehow fails to break on channel close. It makes the test fail fast rather than stall the CI.

## Public API Surface

None. This task does not modify any source files — no new `pub` items, no signature changes, no re-exports.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | Append `test_run_processes_multiple_sequential_events` test (~65 lines) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_run_processes_multiple_sequential_events` | `run()` processes two sequential events (Ready, then Completed) on a single invocation, proving the loop is continuous. The second event must be received and applied after the first, which would fail if `run()` exited after one iteration. | Worker starts in `Initializing` state. `run()` is spawned. | Ready event → Completed event (sent sequentially on same `event_tx`). | Status transitions: Initializing → Idle (on Ready) → Busy (manual) → Idle (on Completed). Final status is `Idle`. | `cargo test -p anvilml-worker --features mock-hardware -- test_run_processes_multiple_sequential_events` exits 0 |

## CI Impact

No CI changes required. The new test is a standard Rust test in `crates/anvilml-worker/tests/managed_tests.rs`, which is already picked up by `cargo test --workspace --features mock-hardware` (the rust-linux and rust-windows CI jobs). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The test uses only `tokio::sync::broadcast`, `tokio::time`, and `WorkerStatus`/`WorkerEvent` — all platform-neutral types and primitives. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Broadcast channel capacity (16) is exhausted if events are sent faster than `run()` consumes them, causing `send()` to return `Err(SendError)` and the test to silently miss an event. | Low | High | The test sends events sequentially with 50–500ms sleeps between them. `run()` processes each event synchronously in the select loop, so the channel never fills. The 16-capacity channel is more than sufficient for two events spaced 200ms+ apart. |
| The `ready_timeout` fires during the test if the Ready event is delayed beyond 60 seconds, causing a spurious `Initializing → Dead` transition that prevents the second event from being processed correctly. | Very Low | High | The test sends the Ready event within 50ms of spawning `run()`. The 60-second timeout is far beyond any realistic processing delay in a test environment. The timeout is also disarmed after the Ready event is processed (the `ready_timeout.take()` pattern in `run()`). |
| Test timing is too tight and the second event is sent before `run()` has processed the first, causing a race where both events are buffered and processed in rapid succession. The test assertion still passes but the timing margin is insufficient for CI reliability. | Low | Medium | The test uses 200ms sleep after the Ready event and 500ms after the Completed event — these are generous relative to the sub-millisecond event processing time in the select loop. If needed, the ACT agent can increase the sleeps. |
| `tokio::time::timeout` on `run_handle.await` with 10-second duration masks a hang if `run()` never exits, making the test appear to pass when it actually hangs. | Low | Medium | The 10-second timeout is a safety net, not a pass condition. The test asserts status before the timeout check, so a hang would cause the status assertions to fail (status would remain Busy) before the timeout fires. The timeout only matters for the final `run_handle.await` line. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --features mock-hardware -- test_run_processes_multiple_sequential_events` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware` exits 0 (all existing tests still pass)
- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
