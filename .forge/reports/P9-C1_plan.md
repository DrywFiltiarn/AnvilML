# Plan Report: P9-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-C1                                             |
| Phase       | 009 — Worker Spawn & Handshake                    |
| Description | anvilml-server: GET /v1/workers handler + WorkerPool in AppState |
| Depends on  | P9-A6 (WorkerPool spawn_all), P9-B1 (mock worker Ready) |
| Project     | anvilml                                           |
| Planned at  | 2026-06-17T00:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Wire the `WorkerPool` into `AppState` and expose it via a new `GET /v1/workers` HTTP handler, enabling clients to query the current state of all spawned Python worker subprocesses. This completes the Phase 009 server-side integration: after this task, the server starts a mock Python worker on boot, the worker reports `Ready`, and a `curl` to `/v1/workers` returns a JSON array containing at least one entry with `status: "idle"`.

## Scope

### In Scope
- Add `workers: Option<Arc<WorkerPool>>` field to `AppState` in `state.rs`.
- Add `workers` parameter to `AppState::new_with_hardware()` production constructor.
- Keep `AppState::new()` unchanged (workers = None) for backward compatibility with existing tests.
- Create `handlers/workers.rs` with `pub async fn list_workers(State(state): State<AppState>) -> Json<Vec<WorkerInfo>>`.
- Add `pub mod workers;` and re-export to `handlers/mod.rs`.
- Mount `GET /v1/workers` route in `build_router()` in `lib.rs`.
- Update `backend/src/main.rs`: bind `RouterTransport`, call `WorkerPool::spawn_all(cfg, devices, transport, broadcaster)`, pass `Arc<WorkerPool>` to `AppState::new_with_hardware()`.
- Update `ws/stats_tick.rs`: change `start()` signature from `fn start(broadcaster: Arc<EventBroadcaster>)` to `fn start(pool: Arc<WorkerPool>)`; use `pool.broadcaster()` for broadcasting and `pool.get_worker_infos()` for the `workers` field in `SystemStats`.
- Update `main.rs` to pass `Arc<WorkerPool>` to `stats_tick::start()`.
- Bump `anvilml-server` patch version from 0.1.14 to 0.1.15.
- Add integration test for `GET /v1/workers` handler.
- Update `stats_tick_tests.rs` to use `WorkerPool` instead of raw `EventBroadcaster`.

### Out of Scope
- Worker restart endpoint (`POST /v1/workers/:id/restart`) — deferred to a future task.
- WebSocket worker list updates — the existing `WorkerStatusChanged` event (already implemented in P9-A6) handles this.
- Any changes to `anvilml-worker` crate source files — those are completed in P9-A1 through P9-A6.
- OpenAPI spec regeneration — handled by CI Gate 2 (`openapi-drift`).

## Existing Codebase Assessment

The codebase has a fully functional `WorkerPool` (in `crates/anvilml-worker/src/pool.rs`) with three public methods: `spawn_all()` (production constructor), `new()` (test constructor with pre-built workers), and `get_worker_infos()` (returns `Vec<WorkerInfo>`). The `WorkerPool` also exposes `broadcaster()` for accessing its shared `EventBroadcaster`.

The `AppState` struct already holds shared state via `Arc` fields (`hardware`, `registry`, `broadcaster`). The `new_with_hardware()` constructor is used in production at `backend/src/main.rs`; the `new()` constructor (in-memory pool) is used by handler tests.

The `handlers/` directory contains `health.rs`, `models.rs`, and `system.rs` — but no `workers.rs` yet. The `build_router()` function in `lib.rs` mounts all existing routes and applies `.with_state(state)`.

The `stats_tick::start()` function currently takes `Arc<EventBroadcaster>` and broadcasts `WsEvent::SystemStats` with an empty `workers` field (`Vec::new()`). The `WsEvent::SystemStats` struct already has a `workers: Vec<WorkerInfo>` field — it just isn't populated yet.

The `ManagedWorker` type exposes `get_status()` which returns `Arc<RwLock<WorkerStatus>>`. The `WorkerPool::get_worker_infos()` method reads each worker's status and constructs `WorkerInfo` structs. No new types need to be created.

No external crates are introduced. All required types (`WorkerPool`, `WorkerInfo`, `EventBroadcaster`, `RouterTransport`) already exist in their respective crates.

## Resolved Dependencies

| Type   | Name          | Version verified | MCP source | Feature flags confirmed |
|--------|---------------|-----------------|------------|------------------------|
| crate  | anvilml-worker| 0.1.14 (workspace) | Cargo.toml (local) | mock-hardware |
| crate  | anvilml-ipc   | 0.1.14 (workspace) | Cargo.toml (local) | n/a |
| crate  | anvilml-core  | 0.1.13 (workspace) | Cargo.toml (local) | n/a |

All dependencies are internal workspace path dependencies — no external MCP lookup needed. The `WorkerPool` type, `spawn_all()` method, `get_worker_infos()` method, and `broadcaster()` method were all confirmed present in `crates/anvilml-worker/src/pool.rs`. The `RouterTransport::bind()` async method was confirmed in `crates/anvilml-ipc/src/transport.rs`. The `WorkerInfo` struct and `WorkerStatus` enum were confirmed in `crates/anvilml-core/src/types/worker.rs`.

## Approach

1. **Add `workers` field to `AppState`** (`crates/anvilml-server/src/state.rs`).
   - Add `pub workers: Option<Arc<anvilml_worker::WorkerPool>>` to the struct.
   - In `new_with_hardware()`, add a `workers: Arc<anvilml_worker::WorkerPool>` parameter and store it as `Some(workers)`.
   - In `new()`, keep `workers: None` for backward compatibility with existing tests.
   - Add `///` doc comment on the new field explaining its purpose.

2. **Create `handlers/workers.rs`** (`crates/anvilml-server/src/handlers/workers.rs`).
   - Implement `pub async fn list_workers(State(state): State<AppState>) -> Json<Vec<WorkerInfo>>`.
   - If `state.workers` is `None`, return `Json(vec![])` (empty array for test/stub mode).
   - If `state.workers` is `Some(pool)`, call `pool.get_worker_infos().await` and return the result.
   - Add `#[utoipa::path]` annotation documenting the endpoint:
     - `summary = "List all workers"`, `responses = [(status = 200, description = "List of workers", body = Vec<WorkerInfo>)]`.
     - `tag = "workers"`.
   - Add `///` doc comment on the function describing what it returns.

3. **Register the workers module** (`crates/anvilml-server/src/handlers/mod.rs`).
   - Add `pub mod workers;` as the first module declaration.
   - No re-export needed — the route in `lib.rs` references the function directly.

4. **Mount the route in `build_router()`** (`crates/anvilml-server/src/lib.rs`).
   - Import `list_workers` from `handlers::workers`.
   - Add `.route("/v1/workers", get(list_workers))` to the router chain.
   - Place the workers route after the models routes and before the WebSocket events route, maintaining alphabetical grouping of resource routes.

5. **Update `main.rs` startup flow** (`backend/src/main.rs`).
   - After hardware detection (line 108), bind the IPC transport:
     ```rust
     let transport = anvilml_ipc::RouterTransport::bind()
         .await
         .expect("failed to bind IPC transport");
     ```
   - Clone the broadcaster before building the router (already done at line 167).
   - Spawn workers before creating AppState:
     ```rust
     let workers = anvilml_worker::WorkerPool::spawn_all(
         &cfg,
         &hardware_info.gpus,
         Arc::new(transport),
         broadcaster.clone(),
     )
     .await
     .expect("failed to spawn worker pool");
     ```
   - Pass `workers` to `AppState::new_with_hardware()`:
     ```rust
     let state = AppState::new_with_hardware(
         env!("CARGO_PKG_VERSION"),
         Arc::new(tokio::sync::RwLock::new(hardware_info)),
         pool,
         registry,
         cfg.model_dirs.clone(),
         Arc::new(workers),
     );
     ```
   - Update `stats_tick::start()` call to pass `Arc<WorkerPool>`:
     ```rust
     anvilml_server::ws::stats_tick::start(Arc::new(workers));
     ```
   - Add `use anvilml_ipc::RouterTransport;` and `use anvilml_worker::WorkerPool;` imports.

6. **Update `stats_tick::start()`** (`crates/anvilml-server/src/ws/stats_tick.rs`).
   - Change signature from `pub fn start(broadcaster: Arc<EventBroadcaster>)` to `pub fn start(pool: Arc<anvilml_worker::WorkerPool>)`.
   - Inside the spawned task, replace `broadcaster` usage with `pool.broadcaster()` for broadcasting events.
   - Replace `workers: Vec::new()` in the `SystemStats` event with `workers: pool.get_worker_infos().await`.
   - Add `use anvilml_worker::WorkerPool;` import (qualified as `anvilml_worker` since the crate name uses a hyphen).
   - The `#[tracing::instrument]` attribute is not needed on `start()` since it's a fire-and-forget spawner (existing convention).

7. **Update `stats_tick_tests.rs`** (`crates/anvilml-server/tests/stats_tick_tests.rs`).
   - Import `anvilml_worker::WorkerPool` and `anvilml_ipc::{RouterTransport, EventBroadcaster}`.
   - Create a minimal `WorkerPool` using `WorkerPool::new()` with empty workers list.
   - Replace all `stats_tick::start(broadcaster)` calls with `stats_tick::start(pool)`.
   - The tests still verify the same behavior (broadcasting `SystemStats` events); only the input type changes.
   - The `workers` field in `SystemStats` will be empty (since the test pool has no workers), which is correct behavior.

8. **Bump `anvilml-server` version** (`crates/anvilml-server/Cargo.toml`).
   - Change `version = "0.1.14"` to `version = "0.1.15"`.

9. **Add integration test** (`crates/anvilml-server/tests/workers_tests.rs`).
   - Test 1: `test_list_workers_returns_empty_when_no_pool` — use `AppState::new()` (workers = None), verify `GET /v1/workers` returns `[]`.
   - Test 2: `test_list_workers_returns_pool_data` — create a `WorkerPool` with one mock worker, build router, verify `GET /v1/workers` returns a JSON array with one entry containing `status: "idle"`.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| New field | `anvilml-server/src/state.rs:AppState` | `pub workers: Option<Arc<WorkerPool>>` |
| Modified constructor | `anvilml-server/src/state.rs:AppState::new_with_hardware` | `fn new_with_hardware(version, hardware, db, registry, model_dirs, workers: Arc<WorkerPool>) -> Self` (added 6th param) |
| New handler | `anvilml-server/src/handlers/workers.rs:list_workers` | `pub async fn list_workers(State(state): State<AppState>) -> Json<Vec<WorkerInfo>>` |
| Modified function | `anvilml-server/src/ws/stats_tick.rs:start` | `fn start(pool: Arc<WorkerPool>)` (was `fn start(broadcaster: Arc<EventBroadcaster>)`) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `workers` field; update `new_with_hardware()` constructor |
| CREATE | `crates/anvilml-server/src/handlers/workers.rs` | New handler: `list_workers` returning `Json<Vec<WorkerInfo>>` |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod workers;` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Import and mount `GET /v1/workers` route |
| MODIFY | `backend/src/main.rs` | Bind transport, spawn workers, pass to AppState and stats_tick |
| MODIFY | `crates/anvilml-server/src/ws/stats_tick.rs` | Change `start()` to take `Arc<WorkerPool>`; populate workers in SystemStats |
| MODIFY | `crates/anvilml-server/tests/stats_tick_tests.rs` | Update to use `WorkerPool` instead of raw `EventBroadcaster` |
| CREATE | `crates/anvilml-server/tests/workers_tests.rs` | Integration tests for `GET /v1/workers` handler |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.14 → 0.1.15 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/workers_tests.rs` | `test_list_workers_returns_empty_when_no_pool` | When `AppState.workers` is `None`, `GET /v1/workers` returns `[]` | Router built with `AppState::new()` (workers=None) | HTTP GET `/v1/workers` | JSON response body is `[]` (empty array) | `cargo test -p anvilml-server --features mock-hardware -- workers_tests::test_list_workers_returns_empty_when_no_pool` exits 0 |
| `crates/anvilml-server/tests/workers_tests.rs` | `test_list_workers_returns_pool_data` | When `AppState.workers` is `Some(pool)`, handler returns worker info from pool | Router built with `AppState::new_with_hardware()` + mock `WorkerPool` containing one worker | HTTP GET `/v1/workers` | JSON array with ≥1 entry, first entry has `status="idle"` | `cargo test -p anvilml-server --features mock-hardware -- workers_tests::test_list_workers_returns_pool_data` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | (existing) `test_stats_tick_broadcasts_system_stats` | Tick broadcasts `SystemStats` with updated `workers` field from pool | WorkerPool with 0 workers created in test | N/A (tick runs internally) | Event received with `workers: []` | `cargo test -p anvilml-server --features mock-hardware -- stats_tick_tests::test_stats_tick_broadcasts_system_stats` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | (existing) `test_stats_tick_cpu_pct_is_finite` | CPU percentage is finite f32 | WorkerPool with 0 workers | N/A | `cpu_pct.is_finite() == true` | `cargo test -p anvilml-server --features mock-hardware -- stats_tick_tests::test_stats_tick_cpu_pct_is_finite` exits 0 |
| `crates/anvilml-server/tests/stats_tick_tests.rs` | (existing) `test_stats_tick_ram_used_mib_is_non_negative` | RAM usage is non-negative | WorkerPool with 0 workers | N/A | `ram_used_mib > 0` | `cargo test -p anvilml-server --features mock-hardware -- stats_tick_tests::test_stats_tick_ram_used_mib_is_non_negative` exits 0 |

## CI Impact

Gate 2 (OpenAPI Drift) will be triggered because a new `#[utoipa::path]` annotation and new route are added. The `anvilml-openapi` binary will emit a new `api/openapi.json` reflecting the `/v1/workers` endpoint. This is expected — the ACT agent will regenerate and stage the OpenAPI spec as part of Gate 2.

No changes to CI workflow files. No new test files need CI configuration — the existing `rust-linux` and `rust-windows` jobs automatically pick up test files in `crates/anvilml-server/tests/`.

## Platform Considerations

None identified. The `WorkerPool`, `RouterTransport`, and `WorkerInfo` types are all platform-neutral — they use `Arc`, `tokio::sync`, and `serde` which work identically on Linux and Windows. The `GET /v1/workers` handler is a pure state read with no platform-specific I/O. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::spawn_all()` requires `Arc<EventBroadcaster>` which is currently inside `AppState` — creating a circular dependency if we build `AppState` first. | High | High | Resolve by cloning the broadcaster *before* building AppState (already done at line 167 of main.rs), spawning workers with the cloned broadcaster, then passing the resulting pool into AppState. |
| `stats_tick::start()` signature change breaks existing tests that pass `Arc<EventBroadcaster>` directly. | High | Medium | Update `stats_tick_tests.rs` to create a minimal `WorkerPool` using `WorkerPool::new()` with an empty workers list. The pool's `broadcaster()` method provides the same `EventBroadcaster` used by the existing tests. |
| `WorkerPool::new()` requires `Arc<RouterTransport>` and `Arc<EventBroadcaster>` as parameters, making test setup verbose. | Medium | Low | Use a bound `RouterTransport` with port 0 (OS-assigned) and a fresh `EventBroadcaster` for the test pool. The transport is never actually used by the test since no messages are sent. |
| The `workers` field in `AppState::new()` being `None` causes silent empty responses in tests that don't explicitly set it. | Low | Low | This is intentional — `AppState::new()` is a test constructor that doesn't involve worker spawning. Tests that need workers use `AppState::new_with_hardware()` with an explicit pool. The empty-array behavior is correct for stub mode. |

## Acceptance Criteria

- [ ] `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware -- workers_tests` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware -- stats_tick_tests` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; d=json.load(sys.stdin); assert isinstance(d,list)"` exits 0 (when server is running with mock workers)
