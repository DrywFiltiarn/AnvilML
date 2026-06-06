# Plan Report: P9-A6

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Task ID     | P9-A6                                          |
| Phase       | 009 — Worker Spawn & Handshake                 |
| Description | anvilml: spawn WorkerPool at startup + GET /v1/workers |
| Depends on  | P9-A5 (WorkerPool::spawn_all + list + acquire) |
| Project     | anvilml                                        |
| Planned at  | 2026-06-06T14:30:00Z                           |
| Attempt     | 1                                              |

## Objective

Integrate the `anvilml-worker` crate into the backend binary so that on startup, after hardware detection, a `WorkerPool` is spawned (one worker per GPU device or one CPU fallback). Send an `InitializeHardware` IPC message to each worker. Add `workers: Arc<WorkerPool>` to `AppState`. Create a new `handlers/workers.rs` module with a `list_workers` handler returning `Json<Vec<WorkerInfo>>`, and wire it as `GET /v1/workers` in the server router.

## Scope

### In Scope
- Add `anvilml-worker` dependency to `backend/Cargo.toml`.
- Add `workers: Arc<WorkerPool>` field to `AppState` (in `anvilml-server/src/state.rs`).
- Update `AppState::new()` and `AppState::new_with_hardware()` to accept an optional `Arc<WorkerPool>`.
- In `backend/src/main.rs`: after hardware detection, call `WorkerPool::spawn_all(&hw_info, &cfg)`, then pass the pool into `AppState`.
- Create `crates/anvilml-server/src/handlers/workers.rs` with a `list_workers` handler.
- Register `GET /v1/workers` route in `build_router()` (in `anvilml-server/src/lib.rs`).
- Export the new workers module from `handlers/mod.rs`.

### Out of Scope
- Worker restart endpoint (`POST /v1/workers/:id/restart`) — that's a separate task.
- Job scheduler wiring to the worker pool (phases 011+).
- Provisioning-based deferred spawn (phase 23+).
- WebSocket worker status events broadcasting — already handled by `WorkerPool::subscribe_events()`.
- Any changes to `ManagedWorker`, `pool.rs`, or `env.rs` — those are covered by P9-A3/A4/A5.

## Approach

1. **Add dependency.** Add `anvilml-worker = { path = "../crates/anvilml-worker" }` to `backend/Cargo.toml` under `[dependencies]`.

2. **Extend AppState.** In `crates/anvilml-server/src/state.rs`:
   - Add `pub workers: Option<Arc<anvilml_worker::WorkerPool>>` field (Option because during early startup / provisioning the pool may not yet exist).
   - Update `new()` to accept an optional `Arc<WorkerPool>` parameter (defaulting to `None`).
   - Update `new_with_hardware()` similarly.

3. **Wire WorkerPool spawn in main.rs.** In `backend/src/main.rs`, after hardware detection and ghost-job reset (after line ~178), before building AppState:
   ```rust
   let workers = anvilml_worker::WorkerPool::spawn_all(&hw_info, &cfg).await;
   tracing::info!(workers_spawned = workers.list().await.len(), "worker pool spawned");
   ```
   Pass `Arc::new(workers)` into the AppState constructor.

4. **Create handlers/workers.rs.** New file `crates/anvilml-server/src/handlers/workers.rs`:
   - Import `anvilml_core::WorkerInfo` and `anvilml_worker::WorkerPool`.
   - Define `list_workers(State<Arc<AppState>>) -> (StatusCode, Json<Vec<WorkerInfo>>)` that calls `state.workers.list().await` and returns the result.
   - If `state.workers` is `None`, return 503 with an empty array or a descriptive error — but since we always spawn in this task, `None` shouldn't occur after startup.

5. **Wire the route.** In `crates/anvilml-server/src/lib.rs`:
   - Add `pub mod workers;` to `handlers/mod.rs`.
   - Add `.route("/v1/workers", get(handlers::workers::list_workers))` to `build_router()`.

6. **Logging per design §11.3.** Add an INFO log at the worker pool spawn point:
   ```rust
   tracing::info!(workers_spawned = workers.list().await.len(), "worker pool spawned");
   ```
   Per §11.3, each worker spawn is logged at INFO by `ManagedWorker::spawn()` (already present: `"worker spawned"` with `worker_id` and `device_index`).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `anvilml-worker` dependency |
| Modify | `crates/anvilml-server/src/state.rs` | Add `workers: Option<Arc<WorkerPool>>` field; update constructors |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `/v1/workers` route to router |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Export new `workers` module |
| Create   | `crates/anvilml-server/src/handlers/workers.rs` | New handler: `list_workers` |
| Modify | `backend/src/main.rs` | Spawn WorkerPool after hardware detection, pass into AppState |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/lib.rs` (inline test) | `workers_endpoint_returns_200` (new) | GET /v1/workers returns 200 with a JSON array of WorkerInfo when workers pool is present in AppState. |

No new standalone test files are needed. The existing integration tests in `backend/tests/` will exercise the route once it's wired, but adding a new handler test inline in `lib.rs` (following the pattern of existing tests there) keeps scope minimal.

## CI Impact

The CI gates (`cargo clippy --workspace --features mock-hardware`, `cargo test --workspace --features mock-hardware`) must all pass. The `anvilml-openapi` binary regenerates `backend/openapi.json` — since we add a new endpoint with `utoipa::ToSchema`-derived types, the OpenAPI drift gate (`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json`) will need to be run and the updated `openapi.json` committed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::spawn_all()` calls `ManagedWorker::spawn()` which spawns a real Python subprocess — if no Python is available, spawn fails with `AnvilError::Io`. | Medium (in CI without venv) | High | The Runnable Proof specifies `ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=<venv>` for local verification. In CI, P9-B1 sets up a minimal venv before tests run. For the plan: note that spawn_all will block until workers reach Idle; if Python is missing, it returns an error that main.rs must handle (unwrap is acceptable for MVP since missing Python is a deployment issue). |
| `AppState::workers` being `Option<Arc<WorkerPool>>` means handlers must handle `None`. | Low | Medium | In this task we always spawn, so workers will always be `Some`. Future tasks (provisioning-based deferred spawn) will need the `None` path. Document the invariant that after startup, workers is always `Some`. |
| OpenAPI drift gate fails because `/v1/workers` endpoint is newly annotated. | Certain | Low | Run `cargo run -p anvilml-openapi` and commit the updated `backend/openapi.json` as part of the implementation. This is expected. |

## Acceptance Criteria

- [ ] `backend/Cargo.toml` includes `anvilml-worker` dependency
- [ ] `AppState` has a `workers: Option<Arc<WorkerPool>>` field
- [ ] `backend/src/main.rs` calls `WorkerPool::spawn_all()` after hardware detection and passes the pool into AppState
- [ ] `GET /v1/workers` returns 200 with a JSON array of `WorkerInfo` objects
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `backend/openapi.json` is updated to include the `/v1/workers` endpoint schema
- [ ] Runnable Proof passes: `ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=<venv> cargo run --features mock-hardware` starts the server, and `curl http://127.0.0.1:8488/v1/workers` returns an array with one `WorkerInfo` whose status transitions to `Idle`
