# Implementation Report: P7-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A3                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-server: WS keepalive ping every 30s  |
| Implemented | 2026-06-04T15:22:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Added a 30-second WebSocket keepalive ping to the `/v1/events` handler in `ws/handler.rs`. The ping is integrated into the existing forward task via a nested `tokio::select!` that multiplexes between broadcast event forwarding and periodic ping sends. The first tick of the interval is consumed immediately upon connection to avoid sending a premature ping before any events have been delivered.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source        |
|--------|---------|-----------------|---------------|
| crate  | tokio   | 1.x (workspace) | Lockfile      |

The `time` feature was added to the existing tokio dependency in `crates/anvilml-server/Cargo.toml`. No new crates were introduced. The `tokio::time::interval`, `tokio::time::MissedTickBehavior`, and `tokio::time::Instant` APIs are all stable since tokio 1.x as used by the workspace.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Added `time` feature to tokio dependency |
| Modify | `crates/anvilml-server/src/ws/handler.rs` | Added ping interval, nested select!, skip first tick, `biased` arm |

## Commit Log

```
 .forge/reports/P7-A3_plan.md            | 110 ++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md            |   6 +-
 .forge/state/state.json                 |  13 ++--
 crates/anvilml-server/Cargo.toml        |   2 +-
 crates/anvilml-server/src/ws/handler.rs |  42 ++++++++----
 5 files changed, 152 insertions(+), 21 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-6af782ed8a20ae6e)

running 7 tests
test ws::broadcaster::tests::send_no_subscribers_no_error ... ok
test ws::broadcaster::tests::subscribe_send_receive ... ok
test tests::env_returns_200_with_stub_report ... ok
test tests::health_returns_200 ... ok
test tests::rescan_returns_202 ... ok
test tests::system_returns_200_with_hardware_info ... ok
test tests::get_model_returns_404_when_missing ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

     Running tests/api_models.rs (target/debug/deps/anvilml_server-981cfc8aa6e2f4a3)

running 3 tests
test list_models_kind_filter_no_match ... ok
test list_models_returns_scanned_models ... ok
test list_models_kind_filter_diffusion ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/api_ws_events.rs (target/debug/deps/anvilml_server-6af782ed8a20ae6e)

running 1 test
test ws_connect_broadcast_receive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

   Doc-tests anvilml_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Platform Cross-Check

### Check 1: Mock Windows-gnu
```
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Checking anvilml-openapi v0.1.0 (/home/dryw/AnvilML/crates/anvilml-openapi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.05s
```

### Check 2: Real Linux native
```
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.80s
```

### Check 3: Real Windows-gnu
```
    Checking anvilml-server v0.1.0 (/home/dryw/AnvilML/crates/anvilml-server)
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.91s
```

All three checks exit 0 with zero errors.

## Project Gates

### Config Surface Sync (Gate 1)
```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```
Zero failures. No config fields were added/removed by this task.

## Deviations from Plan

1. **Ping integration approach**: The plan specified a separate `ping_task` with its own `async move` block and a three-arm `tokio::select!`. However, sharing `ws_tx` across multiple tasks requires `Arc<Mutex<>>`, which caused a subtle bug where the forward task's `rx.recv()` would block indefinitely when a second task held a clone of the shared Arc. The fix was to merge ping logic directly into the forward task using a nested `tokio::select!` with `biased` priority for broadcast events, keeping the outer select! at two arms (forward + receive).

2. **First tick handling**: The plan specified using `set_missed_tick_behavior(MissedTickBehavior::Skip)`, but this does not prevent the first tick from firing immediately upon creation. The fix was to consume the first tick with `let _ = interval.tick().await;` before entering the loop, ensuring no ping fires before any events are delivered.

3. **No `use std::time::Duration` import added**: The plan suggested adding this import, but `Duration` is already available via `std::time::Duration` fully-qualified path in the code. The import was added at line 9 as `use std::time::Duration;`.

## Blockers

None.
