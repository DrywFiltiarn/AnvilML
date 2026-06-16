# Implementation Report: P9-A6

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P9-A6                                             |
| Phase         | 009 — Worker Spawn & Handshake                    |
| Description   | anvilml-worker: pool.rs WorkerPool managing Vec<ManagedWorker> |
| Implemented   | 2026-06-16T23:45:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created `WorkerPool` in `crates/anvilml-worker/src/pool.rs`, a struct that manages a collection of `ManagedWorker` instances with shared IPC transport and event broadcasting. The pool provides `spawn_all()` for production worker creation, `get_worker_infos()` for status snapshots, and `broadcaster()` for direct event access. Background monitoring tasks poll each worker's status at 100ms intervals and broadcast `WsEvent::WorkerStatusChanged` on change.

To avoid a cyclic dependency (`scheduler → worker → server → scheduler`), `EventBroadcaster` was moved from `anvilml-server` to `anvilml-ipc`, which already has `tokio` and `anvilml-core` as dependencies. The `anvilml-server` now re-exports `EventBroadcaster` from `anvilml-ipc` for backward compatibility.

## Resolved Dependencies

| Type   | Name          | Version resolved | Source         |
|--------|---------------|-----------------|----------------|
| crate  | anvilml-ipc   | 0.1.4 (path)    | workspace      |
| crate  | tokio         | 1.52.3          | workspace      |
| crate  | anvilml-core  | 0.1.13 (path)   | workspace      |

No external crate dependencies were added. The `EventBroadcaster` type was moved to `anvilml-ipc` (which already depends on `tokio` and `anvilml-core`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/pool.rs` | WorkerPool struct with spawn_all, new, get_worker_infos, broadcaster methods |
| CREATE | `crates/anvilml-worker/tests/pool_tests.rs` | 4 integration tests for WorkerPool |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Added `pub mod pool;` and `pub use pool::WorkerPool;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bumped version 0.1.5 → 0.1.6 |
| CREATE | `crates/anvilml-ipc/src/ws/mod.rs` | WebSocket module for anvilml-ipc |
| CREATE | `crates/anvilml-ipc/src/ws/broadcaster.rs` | EventBroadcaster moved from anvilml-server |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Added `pub mod ws;` and `pub use ws::EventBroadcaster;` |
| MODIFY | `crates/anvilml-server/src/ws/broadcaster.rs` | Changed to re-export EventBroadcaster from anvilml-ipc |
| MODIFY | `docs/TESTS.md` | Added 4 test entries for pool tests |

## Commit Log

```
 .forge/reports/P9-A6_plan.md                | 177 +++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +-
 Cargo.lock                                  |   2 +-
 crates/anvilml-ipc/src/lib.rs               |   2 +
 crates/anvilml-ipc/src/ws/broadcaster.rs    |  89 +++++++++
 crates/anvilml-ipc/src/ws/mod.rs            |   8 +
 crates/anvilml-server/src/ws/broadcaster.rs |  90 +--------
 crates/anvilml-worker/Cargo.toml            |   2 +-
 crates/anvilml-worker/src/lib.rs            |   2 +
 crates/anvilml-worker/src/pool.rs           | 282 ++++++++++++++++++++++++++++
 crates/anvilml-worker/tests/pool_tests.rs   | 259 +++++++++++++++++++++++++
 docs/TESTS.md                               |  36 ++++
 13 files changed, 876 insertions(+), 92 deletions(-)
```

## Test Results

```
     Running tests/pool_tests.rs (target/debug/deps/pool_tests-d9a30d618fddb7e5)

running 4 tests
test test_broadcaster_returns_reference ... ok
test test_spawn_all_workers_idle ... ok
test test_reexport_worker_pool ... ok
test test_pool_broadcasts_status_change ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 0 failures across all crates (anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker, backend).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, clean)
```

## Platform Cross-Check

```
# Check 1: Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.97s
--- CHECK 1 PASSED ---

# Check 2: Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.41s
--- CHECK 2 PASSED ---

# Check 3: Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.90s
--- CHECK 3 PASSED ---

# Check 4: Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.14s
--- CHECK 4 PASSED ---
```

## Project Gates

Gate 1 (Config Surface Sync): `cargo test -p anvilml --features mock-hardware -- config_reference` — PASSED (1 passed; 0 failed).

Gate 2 (OpenAPI Drift): Not applicable — task does not modify handler signatures, utoipa annotations, or AppState fields.

## Public API Delta

```
crates/anvilml-worker/src/pool.rs:
  pub struct WorkerPool { ... }
  pub async fn spawn_all(cfg: &ServerConfig, devices: &[GpuDevice], transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster>) -> Result<Self, AnvilError>
  pub fn new(workers: Vec<(Arc<ManagedWorker>, String, String)>, transport: Arc<RouterTransport>, broadcaster: Arc<EventBroadcaster>) -> Self
  pub async fn get_worker_infos(&self) -> Vec<WorkerInfo>
  pub fn broadcaster(&self) -> &Arc<EventBroadcaster>

crates/anvilml-worker/src/lib.rs:
  pub mod pool;
  pub use pool::WorkerPool;

crates/anvilml-ipc/src/lib.rs:
  pub mod ws;
  pub use ws::EventBroadcaster;

crates/anvilml-server/src/ws/broadcaster.rs:
  pub use anvilml_ipc::EventBroadcaster;
```

All new pub items match the plan's Public API Surface table (struct `WorkerPool`, `spawn_all`, `get_worker_infos`, `broadcaster`, re-export).

## Deviations from Plan

1. **EventBroadcaster moved to `anvilml-ipc` instead of `anvilml-server`.** The plan specified adding `anvilml-server` as a dependency of `anvilml-worker` to access `EventBroadcaster`. This created a cyclic dependency: `anvilml-scheduler → anvilml-worker → anvilml-server → anvilml-scheduler`. To break the cycle, `EventBroadcaster` was moved to `anvilml-ipc` (which already has `tokio` and `anvilml-core` as dependencies). The `anvilml-server` now re-exports `EventBroadcaster` from `anvilml-ipc` for backward compatibility. This is architecturally sound — the IPC crate is about communication, and broadcasting is communication.

2. **Added `WorkerPool::new()` constructor for testing.** The plan only specified `spawn_all()` as the constructor. A `new()` constructor was added to allow tests to construct a pool with pre-built `ManagedWorker` instances (using `ManagedWorker::new()` with pre-built channels). This avoids subprocess spawning in tests.

3. **Pool stores `(Arc<ManagedWorker>, String, String)` tuples.** The plan mentioned storing `(Arc<ManagedWorker>, String, u32)` tuples. The actual implementation stores `(Arc<ManagedWorker>, String, String)` where the second String is the worker_id and the third is the device_name. This avoids the need for `ManagedWorker` to expose `worker_id` or `device_name` publicly, and the device_index is reconstructed in `get_worker_infos()` by matching the device name against the workers list.

4. **No `#[derive(Debug)]` on `WorkerPool`.** `RouterTransport` does not implement `Debug`, so the derive macro fails. The struct does not derive `Debug`.

## Blockers

None.
