# Implementation Report: P9-A4

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P9-A4                                       |
| Phase         | 009 — Worker Spawn & Handshake              |
| Description   | anvilml-worker: keepalive.rs Ping/Pong heartbeat and pong timeout watchdog |
| Implemented   | 2026-06-16T20:45:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented `crates/anvilml-worker/src/keepalive.rs` — a background tokio task that periodically sends `Ping{seq}` messages to a worker and waits for matching `Pong{seq}` responses. If a pong is not received within `pong_timeout`, the `on_timeout` callback is invoked. The module exports `pub fn start()` which returns a `(JoinHandle<()>, HeartbeatHandle)` tuple, and `HeartbeatHandle` provides a `shutdown()` method for clean termination. Three integration tests verify timeout firing, pong deadline reset, and sequence number incrementing.

## Resolved Dependencies

None. The task uses only existing workspace dependencies: `tokio` (full features), `tracing`, and types from `anvilml-ipc` (`WorkerMessage`, `WorkerEvent`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/keepalive.rs` | Heartbeat loop: Ping/Pong matching, timeout watchdog, HeartbeatHandle |
| CREATE | `crates/anvilml-worker/tests/keepalive_tests.rs` | 3 integration tests for timeout, pong reset, seq increment |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Added `pub mod keepalive;` and `pub use keepalive::start as start_keepalive;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bumped patch version 0.1.3 → 0.1.4 |
| MODIFY | `docs/TESTS.md` | Added entries for 3 new keepalive tests |

## Commit Log

```
 .forge/reports/P9-A4_plan.md                   | 155 ++++++++++++++
 .forge/state/CURRENT_TASK.md                   |   6 +-
 .forge/state/state.json                        |  13 +-
 Cargo.lock                                     |   2 +-
 crates/anvilml-worker/Cargo.toml               |   2 +-
 crates/anvilml-worker/src/keepalive.rs         | 271 +++++++++++++++++++++++++
 crates/anvilml-worker/src/lib.rs               |   2 +
 crates/anvilml-worker/tests/keepalive_tests.rs | 232 +++++++++++++++++++++
 docs/TESTS.md                                  |  27 +++
 9 files changed, 699 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/keepalive_tests.rs (target/debug/deps/keepalive_tests-69db3f6bb025bf0e)

running 3 tests
test test_timeout_fires ... ok
test test_pong_resets_deadline ... ok
test test_seq_increments ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 136 tests passed, 0 failed.

## Format Gate

```
(cargo fmt --all -- --check returned exit 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.13s

# 3. Real-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.74s

# 4. Real-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.84s
```

All four cross-checks passed with exit code 0.

## Project Gates

```
# Gate 1 — Config Surface Sync:
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 1 passed. No config fields were modified by this task.

## Public API Delta

```
+pub mod keepalive;
+pub use keepalive::start as start_keepalive;
```

New public items:
- `pub mod keepalive` — module path: `anvilml_worker::keepalive`
- `pub use keepalive::start as start_keepalive` — re-export at `anvilml_worker::start_keepalive`

The `HeartbeatHandle` struct is `pub` within the `keepalive` module (accessible as `anvilml_worker::keepalive::HeartbeatHandle`) with a single `pub async fn shutdown(&self)` method.

## Deviations from Plan

1. **`start()` returns `(JoinHandle<()>, HeartbeatHandle)` instead of `JoinHandle<()>`**: The plan specified returning only `JoinHandle<()>`. I changed the return to a tuple so the caller can simultaneously await the task and signal shutdown via `HeartbeatHandle`. This is necessary because `HeartbeatHandle` must be stored alongside the `JoinHandle` for the caller to do both things.

2. **`HeartbeatHandle` is `pub` (not private)**: The plan said `HeartbeatHandle` should be private. However, it must be `pub` for the caller to store and use it (specifically, to call `shutdown()`). Making it private would prevent any external code from signaling shutdown.

3. **Added `break` after timeout in inner loop**: During implementation, I discovered that the inner `select!` loop was a tight loop of immediate timeouts (the deadline was never reset after firing). I added a `break` after the timeout callback so the outer loop's `sleep(ping_interval)` executes between ping cycles. This is a behavioral change from the plan's description ("continue to next ping cycle") but is required for correct timing behavior.

4. **Test timing parameters differ from plan defaults**: The plan specified `ping_interval=30s` and `pong_timeout=10s` as design-doc defaults. For tests, I use much shorter durations (`ping_interval=50-100ms`, `pong_timeout=50-500ms`) to keep test execution time reasonable. The test entries in `docs/TESTS.md` document the actual values used.

## Blockers

None.
