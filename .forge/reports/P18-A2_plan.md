# Plan Report: P18-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-A2                                      |
| Phase       | 018 — Worker Restart API & Preflight        |
| Description | anvilml-server: POST /v1/workers/:id/restart |
| Depends on  | P18-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-10T17:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Add a `POST /v1/workers/{id}/restart` endpoint to the `anvilml-server` crate that accepts a worker ID, calls `WorkerPool::restart(id, cfg)` (implemented in P18-A1), and returns HTTP 202 on success. Returns 404 when the worker ID is not found in the pool, and 503 when no worker pool is configured.

## Scope

### In Scope
- Add `config` field (`ServerConfig`) to `AppState` so the restart handler can pass config to `WorkerPool::restart`.
- Implement `restart_worker` handler in `crates/anvilml-server/src/handlers/workers.rs`.
- Wire `POST /v1/workers/{id}/restart` route in `build_router()`.
- Add unit test(s) in `crates/anvilml-server/src/lib.rs` (following the existing test pattern).
- Update `backend/src/main.rs` to pass `cfg` when constructing `AppState`.
- Add logging per FORGE_AGENT_RULES §11.3 (worker restart at INFO).

### Out of Scope
- Any changes to `WorkerPool::restart` implementation (belongs to P18-A1).
- WebSocket event broadcast for restart status transitions (handled by existing pool listener).
- Any changes to `anvilml-worker`, `anvilml-core`, or other crates beyond the `AppState` config field.
- Shutdown_all endpoint (belongs to P18-A4).
- Preflight check (belongs to P18-A3).

## Approach

1. **Add `config` field to `AppState`** — In `crates/anvilml-server/src/state.rs`, add `pub config: anvilml_core::ServerConfig` to the `AppState` struct. Update both `new()` and `new_with_hardware()` constructors to accept and store a `ServerConfig` argument. Update the `Clone` impl to clone the config.

2. **Implement `restart_worker` handler** — In `crates/anvilml-server/src/handlers/workers.rs`, add a new async function:
   ```rust
   pub async fn restart_worker(
       State(state): State<Arc<App>>,
       Path(worker_id): Path<String>,
   ) -> (StatusCode, Json<serde_json::Value>)
   ```
   Logic:
   - If `state.workers` is `None` → 503 with `{"error":"workers_not_configured","message":"worker pool not available"}`.
   - Call `state.workers.as_ref().unwrap().restart(&worker_id, &state.config).await`.
   - On `Err(AnvilError::WorkerDead(_))` → 404 with `{"error":"not_found","message":"worker {worker_id} not found"}`.
   - On other `Err(e)` → 500 with standard error body.
   - On `Ok(())` → 202 with `{"status":"restarting","worker_id":...}`.

3. **Wire the route** — In `crates/anvilml-server/src/lib.rs`, add `.route("/v1/workers/{id}/restart", post(handlers::workers::restart_worker))` to the router chain, alongside the existing `/v1/workers` GET route.

4. **Update `backend/src/main.rs`** — Pass the loaded `cfg: ServerConfig` to `App::new_with_hardware(...)` (or `App::new(...)` if no hardware) by adding it as the new parameter.

5. **Add unit test** — In `crates/anvilml-server/src/lib.rs`, add a test that:
   - Creates a test `WorkerPool` with a mock worker via `WorkerPool::new_test_pool_with_workers()`.
   - Builds `AppState` with the worker pool and a minimal `ServerConfig`.
   - Sends a POST to `/v1/workers/worker-0/restart`.
   - Verifies the response is 202 (restart path exercised; spawn may fail due to no real Python, but the route and handler logic are exercised).
   - Also tests 404 for a non-existent worker ID.

6. **Logging** — The `WorkerPool::restart` method in P18-A1 already emits `info!(worker_id, "restarting worker")` and `info!(worker_id, "worker restarted successfully")`. The new handler does not need additional logging beyond what `restart` provides, per §11.3 mandatory points.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/state.rs` | Add `config: ServerConfig` field to `AppState`; update `new()`, `new_with_hardware()`, and `Clone` impl |
| Modify | `crates/anvilml-server/src/handlers/workers.rs` | Add `restart_worker` handler function |
| Modify | `crates/anvilml-server/src/lib.rs` | Wire POST route; add unit test(s) |
| Modify | `backend/src/main.rs` | Pass `cfg` to `App::new_with_hardware()` / `App::new()` constructor |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/src/lib.rs` | `restart_worker_returns_202_for_existing_worker` | POST `/v1/workers/worker-0/restart` returns 202 when worker exists in pool |
| `crates/anvilml-server/src/lib.rs` | `restart_worker_returns_404_for_unknown_worker` | POST `/v1/workers/nonexistent/restart` returns 404 |

## CI Impact

Adding a new route and handler does not change CI gates. The existing gates apply:
- `cargo fmt --all -- --check` — formatting pass
- `cargo clippy --workspace --features mock-hardware -- -D warnings` — zero warnings
- `cargo test --workspace --features mock-hardware` — all tests pass (new tests included)
- `cargo check --workspace --features mock-hardware` — Linux mock-hardware check
- `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Windows cross-check
- `cargo check --bin anvilml` — real-hardware Linux check
- `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — real-hardware Windows cross-check

No CI workflow files are modified. No OpenAPI drift gate is required unless utoipa annotations are added (the plan does not include utoipa annotations for this endpoint).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::restart` requires `ServerConfig` which is not currently in `AppState` | High | Medium — handler cannot call restart without config | Add `config: ServerConfig` field to `AppState` — minimal, well-scoped change |
| `App::new()` / `App::new_with_hardware()` signature change breaks callers | Medium | High — `backend/src/main.rs` must be updated in the same task | Update `main.rs` in the same commit; the task scope includes it |
| Test restart fails due to no real Python venv in test env | High | Low — test validates route/handler logic, not actual worker restart | Test checks HTTP status codes and response shape; spawn failure from `restart` is acceptable (handler returns 202 for Ok path, or error mapping for failures) |
| `WorkerPool::restart` returns `AnvilError::WorkerDead` for unknown worker — need to distinguish from other dead-worker errors | Medium | Medium — 404 vs 500 distinction | Match on `AnvilError::WorkerDead` variant specifically; all other errors → 500 |

## Acceptance Criteria

- [ ] `POST /v1/workers/{id}/restart` route is registered and returns 202 for a known worker
- [ ] `POST /v1/workers/{id}/restart` returns 404 when worker ID does not exist in pool
- [ ] `POST /v1/workers/{id}/restart` returns 503 when no worker pool is configured
- [ ] Unit test(s) in `lib.rs` verify both 202 and 404 paths
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `backend/src/main.rs` passes config to `AppState` constructor
