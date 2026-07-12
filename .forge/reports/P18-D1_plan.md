# Plan Report: P18-D1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-D1                                            |
| Phase       | 18 — HTTP/WebSocket Server Completion             |
| Description | anvilml-server: GET /v1/workers list handler      |
| Depends on  | P18-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-12T10:55:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the `GET /v1/workers` HTTP handler that returns the current state of all Python
worker subprocesses as a JSON array of `WorkerInfo` objects. The handler delegates to
`WorkerPool::list()` (already present from Phase 16's P16-D1 stats_tick), registers the
route in `build_router()`, and adds a new `workers` handler module alongside the existing
handler modules. After this task, `curl http://127.0.0.1:8488/v1/workers` returns `200`
with a JSON array reflecting the pool's current workers.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/workers.rs` with `list_workers()` handler
- Add `pub mod workers;` to `crates/anvilml-server/src/handlers/mod.rs`
- Register `GET /v1/workers` route in `crates/anvilml-server/src/lib.rs` `build_router()`
- Create `crates/anvilml-server/tests/workers_tests.rs` with ≥3 integration tests
- Bump `anvilml-server` crate version from `0.1.22` to `0.1.23` in `Cargo.toml`

### Out of Scope
- `POST /v1/workers/:id/restart` — deferred to `P18-D3` (restart handler; confirmed:
  P18-D3's context states "extend crates/anvilml-server/src/handlers/workers.rs with
  restart_worker(...)" which covers this exact route)

## Existing Codebase Assessment

**What already exists:** `WorkerPool::list()` is a `pub async fn` method in
`crates/anvilml-worker/src/pool.rs` (lines 155–168), added by Phase 16's P16-D1
stats_tick task. It zips `self.handles` with `self.devices`, constructs a
`Vec<WorkerInfo>` per worker with `worker_id`, `status`, `device_index`, `device_type`,
and `pid: None` / `current_job_id: None`. The method is `pub`, takes `&self`, and
returns `Vec<WorkerInfo>` — exactly the signature needed by the HTTP handler.

**Established patterns:** Handler functions follow a consistent style:
- Module-level doc comment referencing `ANVILML_DESIGN.md §13.4`
- Function doc comment describing the route, HTTP method, success code, and response shape
- One-line delegation with inline rationale comment
- `pub(crate)` visibility (not `pub`) for handler functions — only `build_router()` and
  `AppState` are `pub` from the crate's public API
- Response types use `serde::Serialize` and are `pub(crate)` structs
- Tests use `build_router()` with in-process `AppState`, `Request::get()`,
  `router.oneshot()`, `to_bytes()`, and `serde_json::Value` assertions

**Gap between design doc and source:** The design doc (§13.4) specifies `GET /v1/workers
→ 200 Vec<WorkerInfo>` but no route is currently registered in `build_router()` and no
`workers.rs` handler module exists. The route table in §13.4 is the target state; this
task bridges that gap.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | axum              | 0.8.9           | Cargo.lock | ws (already declared)  |
| crate  | anvilml-worker    | 0.1.22 (local)  | Local      | test-utils (dev-dep)   |

No new external dependencies are introduced. `WorkerPool::list()` is already a `pub`
method on `anvilml-worker`, and `WorkerInfo` is already exported from `anvilml-core`
via `pub use types::*`. The `anvilml-server` crate already depends on both
`anvilml-worker` and `anvilml-core` in its `[dependencies]`.

## Approach

1. **Create `crates/anvilml-server/src/handlers/workers.rs`.** Implement
   `list_workers` as `pub(crate) async fn list_workers(State(state): State<AppState>)
   -> Json<Vec<WorkerInfo>>`. The body is a single await on `state.workers.list()`
   wrapped in `Json()`. Add a module-level doc comment referencing `ANVILML_DESIGN.md
   §13.4` and a function doc comment describing the route contract. Add an inline
   comment explaining the delegation.

2. **Add `pub mod workers;` to `crates/anvilml-server/src/handlers/mod.rs`.** Append
   `pub mod workers;` after the existing module declarations (after `pub mod system;`).

3. **Register the route in `build_router()`.** Add `.route("/v1/workers",
   axum::routing::get(handlers::workers::list_workers))` to the router chain in
   `lib.rs`, placed after the existing `/v1/models` routes and before the WebSocket
   `/v1/events` route, maintaining alphabetical/logical grouping. Add a comment
   referencing `ANVILML_DESIGN.md §13.4`.

4. **Create `crates/anvilml-server/tests/workers_tests.rs`.** Write ≥3 integration
   tests following the pattern from `health_tests.rs`:
   - `test_workers_list_returns_current_pool_state`: Construct a test `AppState` with
     mock workers (using `WorkerPool::set_up_test_workers()` from the `test-utils`
     feature), verify `GET /v1/workers` returns 200 with a JSON array whose elements
     match the injected worker handles' `worker_id`, `status`, `device_index`, and
     `device_type`.
   - `test_workers_list_empty_returns_empty_array`: Use a pool with zero workers
     (just `WorkerPool::new()` without calling `set_up_test_workers()`), verify the
     response is `200` with `[]` (not `null`, not an error body). This confirms the
     handler returns an empty array, not a 404 or error.
   - `test_workers_response_shape_matches_workerinfo`: Assert that the JSON response
     contains exactly the fields `worker_id` (string), `status` (string matching a
     `WorkerStatus` variant in snake_case), `device_index` (integer), `device_type`
     (string matching a `DeviceType` variant in snake_case), `pid` (null), and
     `current_job_id` (null). This verifies the `WorkerInfo` serde representation is
     correct.

5. **Bump `anvilml-server` version.** Change `version = "0.1.22"` to
   `version = "0.1.23"` in `crates/anvilml-server/Cargo.toml`.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `list_workers` | `anvilml-server::handlers::workers` (pub(crate)) | `pub(crate) async fn list_workers(State(state): State<AppState>) -> Json<Vec<WorkerInfo>>` |

No new `pub` items are introduced. The handler function is `pub(crate)` per established
convention — only `build_router()` and `AppState` are part of the crate's public API.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/workers.rs` | New handler module with `list_workers()` |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod workers;` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Register `GET /v1/workers` route in `build_router()` |
| CREATE | `crates/anvilml-server/tests/workers_tests.rs` | ≥3 integration tests |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bump version 0.1.22 → 0.1.23 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `workers_tests.rs` | `test_workers_list_returns_current_pool_state` | `GET /v1/workers` returns 200 with a JSON array whose elements match the injected mock workers' `worker_id`, `status`, `device_index`, and `device_type` | `cargo test -p anvilml-server --features mock-hardware --test workers_tests -- test_workers_list_returns_current_pool_state` exits 0 |
| `workers_tests.rs` | `test_workers_list_empty_returns_empty_array` | With zero workers, `GET /v1/workers` returns 200 with `[]` (not `null`, not an error) | `cargo test -p anvilml-server --features mock-hardware --test workers_tests -- test_workers_list_empty_returns_empty_array` exits 0 |
| `workers_tests.rs` | `test_workers_response_shape_matches_workerinfo` | JSON response contains exactly the fields `worker_id` (string), `status` (string in snake_case), `device_index` (integer), `device_type` (string in snake_case), `pid` (null), `current_job_id` (null) | `cargo test -p anvilml-server --features mock-hardware --test workers_tests -- test_workers_response_shape_matches_workerinfo` exits 0 |

## CI Impact

No new CI job is needed. The `rust-linux` and `rust-windows` CI jobs already run
`cargo test --workspace --features mock-hardware`, which will pick up the new test file
in `crates/anvilml-server/tests/` automatically since it follows the standard Rust
integration-test convention (a `.rs` file under `tests/` is collected as a test crate).

## Platform Considerations

None identified. The handler is a pure delegation to `WorkerPool::list()` which is
platform-neutral (no `#[cfg(unix)]` or `#[cfg(windows)]` guards needed). The `WorkerInfo`
type and its serde serialization are also platform-neutral. The Windows cross-check in
ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `WorkerPool::list()` returns `Vec<WorkerInfo>` with `pid: None` — the response shape may surprise a client expecting `pid`. This is an existing known gap documented in `pool.rs`'s own doc comment (lines 147–153). | Low | Low | Documented by the existing code; not a new defect. The handler faithfully delegates — if `pid` is needed, that's a separate task. |
| The `test-utils` feature on `anvilml-worker` is already declared in `anvilml-server`'s `[dev-dependencies]` (line 39 of Cargo.toml), but if it were missing, `set_up_test_workers()` would be unavailable and the pool-state test could not inject mock handles. | Low | Medium | Already present in Cargo.toml line 39. The plan references this existing dependency. |
| `serde_json::Value` field assertions on `WorkerInfo`'s serde representation may fail if `#[serde(rename_all = "snake_case")]` is not applied to the struct. Checking: `WorkerInfo` has no `#[serde(...)]` attribute, so fields are serialized as-is (`worker_id`, `status`, `device_index`, `device_type`, `pid`, `current_job_id`). `WorkerStatus` has `#[serde(rename_all = "snake_case")]` (worker.rs line 29), so `Idle` → `"idle"`. `DeviceType` also has `#[serde(rename_all = "snake_case")]`. | Low | Medium | The plan explicitly verifies the serde attributes in the source before writing the test assertions. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware --test workers_tests` exits 0
- [ ] `cargo build -p anvilml-server` exits 0 (verifies no compilation errors from the new module)
- [ ] `grep -c "pub mod workers" crates/anvilml-server/src/handlers/mod.rs` returns 1 (module is declared)
- [ ] `grep -c '"/v1/workers"' crates/anvilml-server/src/lib.rs` returns 1 (route is registered)
- [ ] `grep -c "list_workers" crates/anvilml-server/src/handlers/workers.rs` returns ≥1 (handler function exists)
