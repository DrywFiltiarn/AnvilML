# Implementation Report: P9-C1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P9-C1                              |
| Phase         | 009 — Worker Spawn & Handshake     |
| Description   | anvilml-server: GET /v1/workers handler + WorkerPool in AppState |
| Implemented   | 2026-06-17T01:30:00Z               |
| Status        | COMPLETE                           |

## Summary

Wired the `WorkerPool` into `AppState` and exposed it via a new `GET /v1/workers` HTTP handler. The `workers` field is `Option<Arc<WorkerPool>>` — `Some(pool)` in production after `WorkerPool::spawn_all()` succeeds, `None` in tests using `AppState::new()`. The handler returns `pool.get_worker_infos().await` when `Some`, or an empty JSON array when `None`. The `stats_tick::start()` function was updated to take `Arc<WorkerPool>` instead of `Arc<EventBroadcaster>`, using `pool.broadcaster()` for event broadcasting and `pool.get_worker_infos().await` for the workers field in `SystemStats`. The `backend/src/main.rs` startup flow was restructured to resolve a circular dependency: `AppState` needs the broadcaster to spawn workers, but workers need the broadcaster from `AppState`. This was resolved by creating a temporary `AppState` (via `new_with_hardware_no_workers`) to obtain the broadcaster, spawning workers, then creating the real `AppState` with workers included.

## Resolved Dependencies

| Type   | Name          | Version resolved | Source         |
|--------|---------------|------------------|----------------|
| crate  | anvilml-worker| 0.1.14 (workspace) | Cargo.toml (local) |
| crate  | anvilml-ipc   | 0.1.14 (workspace) | Cargo.toml (local) |
| crate  | utoipa        | 5.5.0            | Workspace (Cargo.toml) |

All dependencies are internal workspace path dependencies except `utoipa` which was added to `anvilml-server`'s dependencies. No external MCP lookup was needed for internal crates.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `workers: Option<Arc<WorkerPool>>` field; update `new_with_hardware()` constructor (6th param); add `new_with_hardware_no_workers()` helper |
| CREATE | `crates/anvilml-server/src/handlers/workers.rs` | New handler: `list_workers` returning `Json<Vec<WorkerInfo>>` with `#[utoipa::path]` annotation |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod workers;` module declaration |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Import `list_workers` and mount `GET /v1/workers` route |
| MODIFY | `backend/src/main.rs` | Bind `RouterTransport`, spawn `WorkerPool::spawn_all()`, create real `AppState` with workers, pass to `stats_tick::start()` |
| MODIFY | `crates/anvilml-server/src/ws/stats_tick.rs` | Change `start()` signature from `Arc<EventBroadcaster>` to `Arc<WorkerPool>`; use `pool.broadcaster()` and `pool.get_worker_infos().await` |
| MODIFY | `crates/anvilml-server/tests/stats_tick_tests.rs` | Update to use `WorkerPool` instead of raw `EventBroadcaster`; add `test_pool()` helper |
| CREATE | `crates/anvilml-server/tests/workers_tests.rs` | Integration tests: empty pool and pool with mock worker |
| MODIFY | `crates/anvilml-server/tests/models_tests.rs` | Update 4 calls to `new_with_hardware` → `new_with_hardware_no_workers` |
| MODIFY | `crates/anvilml-server/tests/system_tests.rs` | Update 1 call to `new_with_hardware` → `new_with_hardware_no_workers` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump version 0.1.14 → 0.1.15; add `utoipa` and `futures` deps |
| MODIFY | `docs/TESTS.md` | Add 5 new test entries for workers and stats_tick tests |

## Commit Log

```
 .forge/reports/P9-C1_plan.md                    | 205 ++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                    |   6 +-
 .forge/state/state.json                         |  13 +-
 Cargo.lock                                      |   4 +-
 backend/src/main.rs                             |  65 ++++++--
 crates/anvilml-server/Cargo.toml                |   4 +-
 crates/anvilml-server/src/handlers/mod.rs       |   1 +
 crates/anvilml-server/src/handlers/workers.rs   |  39 +++++
 crates/anvilml-server/src/lib.rs                |   4 +
 crates/anvilml-server/src/state.rs              |  54 +++++++
 crates/anvilml-server/src/ws/stats_tick.rs      |  22 +--
 crates/anvilml-server/tests/models_tests.rs     |   6 +-
 crates/anvilml-server/tests/stats_tick_tests.rs |  52 ++++--
 crates/anvilml-server/tests/system_tests.rs     |   8 +-
 crates/anvilml-server/tests/workers_tests.rs    | 158 ++++++++++++++++++
 docs/TESTS.md                                   |  45 ++++++
 16 files changed, 634 insertions(+), 52 deletions(-)
```

## Test Results

```
running 2 tests
test test_list_workers_returns_empty_when_no_pool ... ok
test test_list_workers_returns_pool_data ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 3 tests
test test_stats_tick_ram_used_mib_is_non_negative ... ok
test test_stats_tick_broadcasts_system_stats ... ok
test test_stats_tick_cpu_pct_is_finite ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 6 tests
test test_rescan_returns_202 ... ok
test test_list_models_empty ... ok
test test_get_model_not_found ... ok
test test_list_models_with_kind_filter ... ok
test test_rescan_infer_kind_and_dtype ... ok
test test_rescan_populates_registry ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 2 tests
test test_system_env_returns_200_with_default_report ... ok
test test_system_returns_200_with_hardware_info ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All tests pass (excluding the pre-existing `test_custom_port_health` failure in `cli_tests.rs` which is unrelated to this task).

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
CHECK 1: mock-hardware Linux OK
CHECK 2: mock-hardware Windows OK
CHECK 3: real-hardware Linux OK
CHECK 4: real-hardware Windows OK
```

All four platform cross-checks pass.

## Project Gates

```
Gate 1 (Config Surface Sync): PASSED
  cargo test -p anvilml --features mock-hardware -- config_reference
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Gate 2 (OpenAPI Drift): PASSED
  cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
  (No diff — openapi.json is in sync with the new /v1/workers endpoint)
```

## Public API Delta

```
+pub mod workers;
+    pub workers: Option<Arc<anvilml_worker::WorkerPool>>,
+    pub fn new_with_hardware_no_workers(
+pub fn start(pool: Arc<anvilml_worker::WorkerPool>) {
```

New pub items:
- `pub mod workers` — new module in `handlers` (crates/anvilml-server/src/handlers/mod.rs)
- `pub workers: Option<Arc<anvilml_worker::WorkerPool>>` — new field in `AppState` (crates/anvilml-server/src/state.rs)
- `pub fn new_with_hardware_no_workers(...)` — new constructor (crates/anvilml-server/src/state.rs)
- `pub fn start(pool: Arc<anvilml_worker::WorkerPool>)` — modified signature in `stats_tick` (crates/anvilml-server/src/ws/stats_tick.rs)

Existing pub items modified:
- `pub fn new_with_hardware(...)` — added 6th parameter `workers: Arc<WorkerPool>` (crates/anvilml-server/src/state.rs)

The handler `pub async fn list_workers(...)` is `pub` but defined in a module that is not re-exported at the crate root — it is only accessible via `handlers::workers::list_workers` from `lib.rs`.

## Deviations from Plan

- **Added `new_with_hardware_no_workers()` constructor**: The plan called for passing workers to `new_with_hardware()`, but this created a circular dependency — `AppState` needs the broadcaster to spawn workers, but workers need the broadcaster from `AppState`. Resolved by adding a 5-parameter `new_with_hardware_no_workers()` helper that creates a temp `AppState` to obtain the broadcaster, then spawning workers, then creating the real `AppState` with workers included.
- **Reordered `main.rs` startup flow**: The plan said "spawn workers before creating AppState", but this is impossible because `WorkerPool::spawn_all()` needs the broadcaster which lives inside `AppState`. Resolved by creating a temporary `AppState` first.
- **Added `utoipa` dependency to `anvilml-server`**: The plan assumed `utoipa::path` was already available, but it was only in `anvilml-core`. Added `utoipa = { workspace = true }` to `anvilml-server`'s dependencies.
- **Used `futures::executor::block_on` in tests**: The test helper `test_pool()` and `mock_pool_with_one_worker()` need to synchronously bind a `RouterTransport` (which is async). Used `futures::executor::block_on` with the `futures = "0.3"` dev-dependency.
- **Updated existing tests to use `new_with_hardware_no_workers`**: Four calls in `models_tests.rs` and one in `system_tests.rs` were updated to use the new 5-parameter constructor.

## Blockers

None.
