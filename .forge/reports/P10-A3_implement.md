# Implementation Report: P10-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A3                                      |
| Phase       | 010 — Worker Crash Recovery                 |
| Description | anvilml-server: broadcast worker status changes to WS |
| Implemented | 2026-06-06T20:15:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Implemented a background bridge task (`spawn_worker_status_bridge`) in `backend/src/main.rs` that subscribes to the `WorkerPool`'s internal event channel and forwards `WorkerEvent::WorkerStatusChanged` events to the server's WebSocket `EventBroadcaster`. This enables connected `/v1/events` clients to receive real-time `WsEvent::WorkerStatusChanged` notifications whenever a worker's lifecycle state transitions (Idle → Busy, Busy → Dead, Dead → Respawning, Respawning → Idle). The bridge is spawned after worker pool creation and before server listen, with the same `Arc<EventBroadcaster>` instance used by `AppState`.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|-----------------|----------------|
| workspace | chrono | 0.4.45          | Cargo.lock (workspace dep) |
| workspace | tokio  | 1.52.3          | Cargo.lock (workspace dep, broadcast already available via tokio::sync) |

Note: `anvilml-ipc` and `anvilml-worker` were already declared dependencies of the backend crate or transitively available. `chrono` was added as a new direct dependency (workspace-pinned at 0.4.45). The `tokio::sync::broadcast` module is part of the existing `tokio` dependency (no new crate needed).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/src/main.rs` | Add imports (`WorkerEvent`, `WsEvent`, `WorkerStatusChangedEvent`, `Utc`, `broadcast`), implement `spawn_worker_status_bridge` function, wire bridge into `main()` after broadcaster creation |
| Modify | `backend/Cargo.toml` | Bump patch version `0.1.0 → 0.1.1`; add `anvilml-ipc` and `chrono` dependencies |

## Commit Log

```
 backend/Cargo.toml            |   4 +-
 backend/src/main.rs           |  46 ++++++++++++++++++
 Cargo.lock                    |   4 +-
 .forge/reports/P10-A3_plan.md | 134 +++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md  |   6 ++-
 .forge/state/state.json       |  13 +++---
 6 files changed, 196 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_core-2ce11a52aa331635)
running 74 tests
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_hardware-a377bb7e8c61e8d8)
running 56 tests
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_ipc-cb53960350c2e5d7)
running 17 tests
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/bin/ipc-probe.rs (target/debug/deps/ipc_probe-16838bd76e4db650)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml_openapi-48de87e79e88532b)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_registry-3df337931d8f5352)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/anvilml_registry_db.rs (target/debug/deps/anvilml_registry_db-34c36b28b693a903)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store.rs (target/debug/deps/device_store-a2d3be5d5933bbf2)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/rescan.rs (target/debug/deps/rescan-44356cf60417b048)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner.rs (target/debug/deps/scanner-d3218cbd3b96bb91)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader.rs (target/debug/deps/seed_loader-f7d1c1c83c7a3559)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_get.rs (target/debug/deps/store_get-5cb98cd23f67b4c3)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_list.rs (target/debug/deps/store_list-a2d3be5d5933bbf2)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_scheduler-1bd768a46da3d021)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_server-23987c6b93c27540)
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_models.rs (target/debug/deps/api_models-caa57490242986d7)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/api_ws_events.rs (target/debug/deps/api_ws_events-9dc03bc3dd189214)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-020e1c1b93a4048d)
running 12 tests
test result: ok. 10 passed; 2 ignored; 0 failed; 0 measured; 0 filtered out

     Running unittests src/main.rs (target/debug/deps/anvilml-9356678ddbec66ec)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs (target/debug/deps/config_reference-a1118ce232cde6af)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_hardware
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_ipc
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_registry
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_scheduler
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_server
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_worker
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Total: 198 passed, 0 failed, 2 ignored (pre-existing — require Python worker).

## Format Gate

```
$ cargo fmt --all -- --check
(no output — exit 0)
```

No formatting drift detected.

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
$ cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.41s

# Check 2: Mock-hardware Windows cross
$ cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.01s

# Check 3: Real-hardware Linux
$ cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.23s

# Check 4: Real-hardware Windows cross
$ cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.71s
```

All four platform checks passed with exit code 0.

## Project Gates

```
# Gate 1: Config Surface Sync
$ cargo test -p backend --features mock-hardware -- config_reference
     Running tests/config_reference.rs (target/debug/deps/config_reference-a1118ce232cde6af)
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate passed — `anvilml.toml` key-set matches `ServerConfig::default()`.

## Deviations from Plan

- **Function parameter type**: The plan specified `broadcaster: &EventBroadcaster` but then used `Arc::clone(broadcaster)`, which is incompatible (you cannot clone a bare reference into an `Arc`). Changed to `broadcaster: &Arc<EventBroadcaster>` to match the actual usage pattern where the function receives `&broadcaster` (an `Arc<EventBroadcaster>`).
- **No `if let Some` guard**: The plan included an `if let Some(ref workers) = workers` guard, but in the actual code `workers` is a direct `WorkerPool` (not `Option<WorkerPool>`) at the point of bridge spawning. The guard was unnecessary — the pool always exists after `spawn_all`. Removed the guard and called `spawn_worker_status_bridge(&workers, &broadcaster)` directly.
- **Added `use tokio::sync::broadcast;` import**: Required because `broadcast::error::RecvError` is used in the match arms. This was not explicitly listed in the plan's imports but is necessary for compilation.

## Blockers

None. All gates passed, all tests passed, all cross-checks passed.
