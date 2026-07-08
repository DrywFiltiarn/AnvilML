# Implementation Report: P16-A1

| Field       | Value                                           |
|-------------|-------------------------------------------------|
| Task ID     | P16-A1                                          |
| Phase       | 016 — Live Events                               |
| Description | anvilml-scheduler: event_loop subscribes WorkerEvent, publishes WsEvent |
| Implemented | 2026-07-09T01:15:00Z                            |
| Status      | COMPLETE                                        |

## Summary

Implemented the scheduler's event loop that subscribes to `WorkerEvent` variants from workers via a `RouterTransport`, maps each event to its corresponding `WsEvent` counterpart, and publishes it via the shared `EventBroadcaster`. Added `map_worker_event()` for the one-to-one event mapping, `spawn_event_loop()` for the infinite receive-publish loop with `ImageReady` save-before-publish ordering, and 7 integration tests covering all 4 new event variants, the `ImageReady` ordering, and the end-to-end transport path.

## Resolved Dependencies

No new external crates introduced. All types and APIs used are from existing dependencies already declared in `anvilml-scheduler/Cargo.toml`.

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | zeromq    | 0.6.0            | Cargo.lock     |
| crate  | tokio     | 1.52.3           | Cargo.lock     |
| crate  | bytes     | 1.12             | Cargo.lock     |
| crate  | rmp-serde | 1.3.1            | Cargo.lock     |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-scheduler/src/event_loop.rs` | Added `map_worker_event()` and `spawn_event_loop()` functions; updated module doc comment; added imports for `WsEvent`, `EventBroadcaster`, `RouterTransport`, `JoinHandle` |
| Modify | `crates/anvilml-scheduler/src/lib.rs` | Re-exported `map_worker_event` and `spawn_event_loop` |
| Modify | `crates/anvilml-scheduler/src/scheduler.rs` | Made `artifact_store` field `pub(crate)` so `event_loop` module can access it for the event loop task |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bumped patch version 0.1.20 → 0.1.21 |
| Modify | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | Added 7 new integration tests; added imports for `Bytes`, `PeerIdentity`, `SocketOptions`, `JobScheduler`, `NodeTypeRegistry`, `broadcast` |
| Modify | `docs/TESTS.md` | Added 7 test catalogue entries for the new tests |

## Commit Log

```
 .forge/reports/P16-A1_plan.md                      | 412 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   2 +-
 crates/anvilml-scheduler/Cargo.toml                |   2 +-
 crates/anvilml-scheduler/src/event_loop.rs         | 267 ++++++++++++-
 crates/anvilml-scheduler/src/lib.rs                |   2 +-
 crates/anvilml-scheduler/src/scheduler.rs          |   5 +-
 crates/anvilml-scheduler/tests/event_loop_tests.rs | 386 ++++++++++++++++++-
 docs/TESTS.md                                      |  83 +++++
 10 files changed, 1155 insertions(+), 23 deletions(-)
```

## Test Results

```
     Running tests/event_loop_tests.rs (target/debug/deps/event_loop_tests-ff8eb71e2023d420)

running 11 tests
test test_image_ready_publishes_after_save ... ok
test test_map_cancelled ... ok
test test_map_completed ... ok
test test_map_failed ... ok
test test_image_ready_malformed_base64_errors ... ok
test test_map_progress ... ok
test test_image_ready_saves_artifact ... ok
test test_image_ready_empty_image_b64 ... ok
test test_image_ready_artifact_meta_fields_match ... ok
test test_spawn_event_loop_handles_recv_error ... ok
test test_spawn_event_loop_receives_and_publishes ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 293 tests passed, 0 failed.

## Format Gate

```
(Exit 0 — no drift after pass 3 in-place reformat)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.28s
=== CHECK 1 PASSED ===

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 34.02s
=== CHECK 2 PASSED ===

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.69s
=== CHECK 3 PASSED ===

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.29s
=== CHECK 4 PASSED ===
```

## Project Gates

Gate 1 (config_reference): Not triggered — no ServerConfig changes.
Gate 2 (openapi-drift): Not triggered — no handler changes.
Gate 3 (node-parity): Not triggered — no node changes.
Gate 4 (parity-markers): Not triggered — no node execute() changes.

## Public API Delta

```
+pub fn map_worker_event(event: WorkerEvent) -> WsEvent {
+pub fn spawn_event_loop(
+pub use event_loop::{handle_image_ready, map_worker_event, spawn_event_loop};
```

New pub items:
- `map_worker_event` (fn) — module path `anvilml_scheduler::event_loop::map_worker_event`, re-exported at `anvilml_scheduler::map_worker_event`
- `spawn_event_loop` (fn) — module path `anvilml_scheduler::event_loop::spawn_event_loop`, re-exported at `anvilml_scheduler::spawn_event_loop`

Both match the plan's Public API Surface table.

## Deviations from Plan

1. **Parameter name**: The plan specified `self: Arc<JobScheduler>` as the first parameter of `spawn_event_loop()`. This is only valid inside an `impl` block; for a standalone function, Rust requires a regular parameter name. Changed to `scheduler: Arc<JobScheduler>`.

2. **`artifact_store` visibility**: The plan assumed `self.artifact_store` was accessible from `event_loop.rs`. The field was private (`artifact_store: Arc<ArtifactStore>`). Made it `pub(crate) artifact_store` to allow access within the crate.

3. **`ImageReady` handling in `spawn_event_loop()`**: The plan's approach of extracting `width`, `height`, `seed`, `steps` before calling `handle_image_ready()` was correct, but the implementation needed to destructure the event first to extract those fields since `handle_image_ready()` consumes the event by value.

4. **Test transport message format**: The initial test implementation sent a single-frame ZmqMessage, which the ROUTER couldn't parse correctly. Fixed by sending a 2-frame message (empty delimiter + payload), matching the pattern used in `crates/anvilml-ipc/tests/roundtrip_tests.rs`.

5. **Test DEALER identity**: Added `PeerIdentity` to the test DEALER socket, matching the pattern in the existing roundtrip tests. Without a PeerIdentity, the ROUTER may not correctly associate the message.

6. **Test connection delay**: Added a 50ms sleep after DEALER connect to give it time to register with the ROUTER, matching the pattern in existing roundtrip tests.

## Blockers

None.
