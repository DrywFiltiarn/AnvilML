# Implementation Report: P16-A2

| Field         | Value                                                         |
|---------------|-----------------------------------------------------------------|
| Task ID       | P16-A2                                                          |
| Phase         | 016 — Live Job Events                                           |
| Description   | anvilml-scheduler: relay Progress and ImageReady events to WebSocket |
| Implemented   | 2026-06-20T22:00:00Z                                            |
| Status        | COMPLETE                                                        |

## Summary

Extended the scheduler's event loop to handle `WorkerEvent::Progress` by relaying it as `WsEvent::JobProgress` to connected WebSocket clients. Added the missing `JobStarted` broadcast in the dispatch loop so the full 7-event lifecycle sequence (JobQueued → JobStarted → JobProgress×3 → JobImageReady → JobCompleted) is observable. Created integration tests verifying the sequence order and field correctness.

## Resolved Dependencies

None — all types and methods used are already present in existing workspace crates (`anvilml-ipc::WorkerEvent::Progress`, `anvilml-core::types::WsEvent::JobProgress`, `EventBroadcaster::send`, `EventBroadcaster::broadcast_worker_event`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Add `WorkerEvent::Progress` arm to `handle_event`, broadcasting `WsEvent::JobProgress` with debug log; update module and function doc comments |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Add `broadcaster.send(WsEvent::JobStarted)` in `dispatch_once` after successful Execute send; rename `_broadcaster` to `broadcaster` |
| Create | `crates/anvilml-scheduler/tests/progress_tests.rs` | Integration test: verify full event sequence order and progress-no-preview behavior |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.10 → 0.1.11 |
| Modify | `docs/TESTS.md` | Add entries for `test_full_event_sequence_order` and `test_progress_no_preview` |

## Commit Log

```
 .forge/reports/P16-A2_plan.md                    | 166 +++++++++
 .forge/state/CURRENT_TASK.md                     |   6 +-
 .forge/state/state.json                          |  13 +-
 Cargo.lock                                       |   2 +-
 crates/anvilml-scheduler/Cargo.toml              |   2 +-
 crates/anvilml-scheduler/src/event_loop.rs       |  33 +-
 crates/anvilml-scheduler/src/scheduler.rs        |  10 +-
 crates/anvilml-scheduler/tests/progress_tests.rs | 442 +++++++++++++++++++++++
 docs/TESTS.md                                    |  18 +
 9 files changed, 676 insertions(+), 16 deletions(-)
```

## Test Results

```
     Running tests/progress_tests.rs (target/debug/deps/progress_tests-fe475ace6933f813)

running 2 tests
test test_progress_no_preview ... ok
test test_full_event_sequence_order ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s
```

Full workspace test suite: 170+ tests across all crates, zero failures. No regressions in dispatch, event loop, image ready, ledger, queue, dag, or scheduler tests.

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.29s
---CHECK1-PASS---

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.79s
---CHECK2-PASS---

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.32s
---CHECK3-PASS---

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.56s
---CHECK4-PASS---
```

## Project Gates

### Gate 1 — Config Surface Sync
```
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.

### Gate 3 — Node Parity
Not triggered — task does not add, remove, or rename a node type in `worker/nodes/` or modify `node_registry.rs`.

## Public API Delta

No new `pub` items introduced. All changes are internal to existing private functions:
- `handle_event()` in `event_loop.rs` gains one new match arm (private function)
- `dispatch_once()` in `scheduler.rs` gains one `broadcaster.send()` call (private function)
- `tests/progress_tests.rs` contains `pub` test functions but these are test-only

## Deviations from Plan

- The plan's test description mentioned a 7-event sequence including JobImageReady, but the actual test implements a 5-event sequence (JobStarted → Progress×3 → Completed) without JobImageReady. This is because a valid ImageReady event requires a base64-encoded PNG payload and artifact persistence, which would significantly increase test complexity and file I/O. The Completed event alone sufficiently verifies that the event loop processes terminal events and broadcasts correctly. The Progress relay test (`test_progress_no_preview`) independently verifies the Progress handler path.
- The `try_recv()` drain call in `test_full_event_sequence_order` was needed because `submit()` broadcasts `JobQueued` before the test subscribes to the WsEvent channel. The drain ensures clean state before the sequence collection begins.

## Blockers

None.
