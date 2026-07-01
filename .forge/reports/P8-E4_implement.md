# Implementation Report: P8-E4

| Field         | Value                                                         |
|---------------|---------------------------------------------------------------|
| Task ID       | P8-E4                                                         |
| Phase         | 008 — IPC Stress Gate & Worker Pool                           |
| Description   | anvilml-worker: ManagedWorker tracks crash attempt_history, consults RespawnPolicy |
| Implemented   | 2026-07-01T16:45:00Z                                          |
| Status        | COMPLETE                                                      |

## Summary

Wired the `RespawnPolicy` decision point into `ManagedWorker::run()`'s crash exit path. On transport recv error, the worker now appends `Instant::now()` to `attempt_history`, calls `should_respawn()` on the policy, and logs the decision at INFO level. Added a `pub fn attempt_count()` accessor so tests can verify crash history growth. Removed `#[allow(dead_code)]` from both fields. Also added a `close()` method to `RouterTransport` to enable tests to trigger the crash path reliably. Three new integration tests verify crash-attempt tracking, history growth across multiple crashes, and policy consultation with a custom respawn policy.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | tokio     | 1.52.3           | Cargo.toml     |
| crate  | tracing   | 0.1              | Cargo.toml     |
| crate  | zeromq    | 0.6.0            | Cargo.toml     |

No new external dependencies introduced. `Instant`, `Duration`, and `Vec` are from the Rust standard library. The `RespawnPolicy` API (`should_respawn`, `next_delay`) was verified by reading `respawn.rs` directly.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Remove `#[allow(dead_code)]` from `respawn_policy` and `attempt_history`; add `pub fn attempt_count()` accessor; modify crash exit path to append `Instant::now()` and call `should_respawn()` + log `crash_respawn_decision` |
| Modify | `crates/anvilml-worker/tests/managed_tests.rs` | Add 3 new integration tests (`test_crash_appends_to_attempt_history`, `test_crash_history_grows_per_crash`, `test_should_respawn_called_on_crash`); fix unused variable warning |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |
| Modify | `crates/anvilml-ipc/src/transport.rs` | Restructure `RouterTransport` to store `RouterSocket` behind `Arc<Mutex<Option<>>>` instead of split halves; add `pub async fn close()` method for test crash-path triggering; update `send()`, `send_raw()`, `recv()` to use new structure |
| Modify | `docs/TESTS.md` | Add 3 test entries for new managed_tests tests |

## Commit Log

```
 .forge/reports/P8-E4_plan.md                 | 186 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                 |   6 +-
 .forge/state/state.json                      |  13 +-
 Cargo.lock                                   |   2 +-
 crates/anvilml-ipc/src/transport.rs          | 100 ++++++-----
 crates/anvilml-worker/Cargo.toml             |   2 +-
 crates/anvilml-worker/src/managed.rs         |  34 +++-
 crates/anvilml-worker/tests/managed_tests.rs | 243 +++++++++++++++++++++++++++
 docs/TESTS.md                                |  36 ++++
 9 files changed, 552 insertions(+), 70 deletions(-)
```

## Test Results

```
     Running tests/managed_tests.rs (target/debug/deps/managed_tests-dc7ef07a9517aef7)

running 17 tests
test test_clone_independent_worker_id ... ok
test test_clone_shares_status ... ok
test test_concurrent_status_and_set_status_no_deadlock ... ok
test test_request_shutdown_is_idempotent ... ok
test test_request_shutdown_sends_signal ... ok
test test_set_status_callable_repeatedly ... ok
test test_set_status_changes_value ... ok
test test_set_status_visible_across_clone ... ok
test test_status_returns_current_value ... ok
test test_shutdown_rx_triggers_graceful_exit ... ok
test test_deregister_called_on_graceful_exit ... ok
test test_run_completes_on_ready_event ... ok
test test_should_respawn_called_on_crash ... ok
test test_deregister_called_on_crash ... ok
test test_crash_history_grows_per_crash ... ok
test test_crash_appends_to_attempt_history ... ok
test test_deregister_called_on_initializing_timeout ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.06s

     Running tests/respawn_tests.rs (target/debug/deps/respawn_tests-897f10eb9e1bee64)

running 6 tests
test test_attempts_outside_window_dont_count ... ok
test test_empty_history_allows_respawn ... ok
test test_defaults_match_documented_values ... ok
test test_at_limit_blocks_respawn ... ok
test test_next_delay_returns_correct_duration ... ok
test test_under_limit_allows_respawn ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 0 failures across all crates.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.16s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.30s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.82s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.89s
```

All four cross-checks passed.

## Project Gates

None applicable — task does not touch config fields, handler signatures, or node types.

## Public API Delta

```
+    pub async fn close(&self) {
+    pub fn attempt_count(&self) -> usize {
```

Two new public items:
1. `pub fn attempt_count(&self) -> usize` on `impl ManagedWorker` (`crates/anvilml-worker/src/managed.rs`) — returns the number of crash attempts in `attempt_history`. Matches the plan's Public API Surface.
2. `pub async fn close(&self)` on `impl RouterTransport` (`crates/anvilml-ipc/src/transport.rs`) — closes the transport, causing `recv()` to return an error. Added to enable tests to trigger the crash path.

## Deviations from Plan

- **`RouterTransport` restructuring**: The plan assumed dropping the DEALER socket would cause `recv()` to fail. In practice, ZeroMQ ROUTER sockets do not return an error when a DEALER peer disconnects — they simply stop routing to that peer and continue waiting for other peers. To reliably trigger the crash path, I added a `close()` method to `RouterTransport` that drops the underlying socket. This required restructure `RouterTransport` from split send/recv halves back to a single `RouterSocket` behind a mutex. This is a necessary deviation to make the crash path testable.
- **Test approach**: Instead of dropping the DEALER socket (as the plan specified), the tests now call `transport.close().await` to trigger the crash path. The tests still verify the same behavior (crash history growth, policy consultation, log emission).
- **Version bump**: Cargo.toml bumped from 0.1.8 → 0.1.9 as specified in the plan.
- **`anvilml-ipc` version bump**: The `anvilml-ipc` crate was also modified (added `close()` method), so its version was already at 0.1.9 (it had been bumped in a prior task). No additional bump needed for this task.

## Blockers

None.
