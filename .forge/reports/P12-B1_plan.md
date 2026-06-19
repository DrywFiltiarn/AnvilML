# Plan Report: P12-B1

| Field       | Value                                     |
|-------------|-------------------------------------------|
| Task ID     | P12-B1                                    |
| Phase       | 012 — Graph Validation                    |
| Description | POST /v1/jobs wired to validate_graph, 422 on invalid graph |
| Depends on  | P12-A1, P12-A2                            |
| Project     | anvilml                                   |
| Planned at  | 2026-06-19T19:15:00Z                      |
| Attempt     | 1                                         |

## Objective

Wire the `POST /v1/jobs` endpoint in `anvilml-server` to call `validate_graph` from `anvilml-scheduler`. When the node registry is empty (no workers have reported), return 503. When validation fails, return 422 with the list of validation errors. When the graph is valid, return 202 with a placeholder job ID. This is the server-side integration of the graph validation logic built in Phase 012 Group A.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/jobs.rs` with `submit_job` handler function.
- Add `pub mod jobs` and `pub use jobs::submit_job` to `crates/anvilml-server/src/handlers/mod.rs`.
- Mount `POST /v1/jobs` route in `build_router()` in `crates/anvilml-server/src/lib.rs`.
- Update `AnvilError::InvalidGraph` HTTP status code from 400 to 422 in `crates/anvilml-core/src/error.rs` (required by design spec §12.5).
- Create `crates/anvilml-server/tests/jobs_tests.rs` with integration tests.

### Out of Scope
- Full job persistence (Phase 013).
- `GET /v1/jobs`, `DELETE /v1/jobs/:id`, `POST /v1/jobs/:id/cancel` handlers.
- `JobScheduler` integration — the handler returns a placeholder `Uuid::new_v4()`.
- OpenAPI spec regeneration (handled by a separate gate task).

## Existing Codebase Assessment

Phase 012 Group A (P12-A1, P12-A2) has already been completed. `crates/anvilml-scheduler/src/dag.rs` contains the `validate_graph` function which performs six validation checks (nodes array, duplicate IDs, node type registration, edge references, slot type compatibility, acyclicity) in non-fail-fast collect-all-errors mode. `crates/anvilml-scheduler/src/types.rs` defines the `GraphError` enum with `Display` implementation, re-exporting `ValidatedGraph`. `crates/anvilml-scheduler/src/lib.rs` re-exports both `GraphError` and `ValidatedGraph`.

The `anvilml-server` crate currently has no `jobs` handler module — `handlers/mod.rs` exports `health`, `models`, `nodes`, `system`, and `workers` modules, but not `jobs`. The `build_router()` function in `lib.rs` mounts routes for health, system, models, workers, nodes, and WebSocket events, but not `POST /v1/jobs`.

`AppState` (in `state.rs`) carries `node_registry: Arc<NodeTypeRegistry>` — the same field used by the `list_nodes` handler. The `NodeTypeRegistry` has `is_empty()` and `get()` async methods.

`AnvilError::InvalidGraph(Vec<String>)` exists in `anvilml-core/src/error.rs` and maps to `StatusCode::BAD_REQUEST` (400). Per the design spec §12.5, graph validation failures should return 422. This requires updating the `status_code()` match arm.

The established handler pattern uses `State<AppState>` extraction, `Json<T>` request/response bodies, and `AnvilError` as the error type with `IntoResponse`. Tests use `Router::oneshot` with `tower::util::ServiceExt::oneshot`.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | anvilml-scheduler | 0.1.4 (workspace) | Cargo.lock | mock-hardware forwarded |
| crate  | anvilml-core      | 0.1.x (workspace) | Cargo.lock | n/a                    |
| crate  | uuid              | 1.23.3          | Cargo.lock | serde, v4              |

No new external dependencies are introduced. `uuid` is already a workspace dependency (declared in root `Cargo.toml` and dev-dependency of `anvilml-server`). For the handler to use `Uuid::new_v4()` at runtime, `uuid` must be moved from `[dev-dependencies]` to `[dependencies]` in `anvilml-server/Cargo.toml`.

## Approach

1. **Update `AnvilError::InvalidGraph` status code from 400 to 422.** In `crates/anvilml-core/src/error.rs`, change the `status_code()` match arm: replace `AnvilError::InvalidGraph(_)` from the `StatusCode::BAD_REQUEST` arm to its own arm returning `StatusCode::UNPROCESSABLE_ENTITY`. Update the doc comment in `status_code()` and the `error_kind()` match arm (the latter already returns `"invalid_graph"` which is correct). Update the module-level doc comment that lists `InvalidGraph` under 400 errors.

   Rationale: The design spec §12.5 explicitly states 422 for graph validation failures. The existing 400 mapping is a pre-existing deviation that this task fixes as a prerequisite.

2. **Create `crates/anvilml-server/src/handlers/jobs.rs`.** Implement the `submit_job` handler:
   - Signature: `pub async fn submit_job(State(state): State<AppState>, Json(req): Json<SubmitJobRequest>) -> Result<(StatusCode, Json<SubmitJobResponse>>, AnvilError)`
   - Check `state.node_registry.is_empty().await`; if true, return `Err(AnvilError::WorkersUnavailable("no workers available to validate graph".into()))`
   - Call `anvilml_scheduler::validate_graph(&req.graph, &*state.node_registry).await` (dereference `Arc` via `&*`)
   - On `Err(errors)`: return `Err(AnvilError::InvalidGraph(errors))` — the `IntoResponse` impl will produce 422 after step 1
   - On `Ok(_)`: return `Ok((StatusCode::ACCEPTED, Json(SubmitJobResponse { job_id: Uuid::new_v4(), queue_position: 0 })))`
   - Add `#[tracing::instrument]` to the handler function
   - Add `///` doc comment describing the endpoint, status codes, and parameters

3. **Update `crates/anvilml-server/src/handlers/mod.rs`.** Add `pub mod jobs;` and `pub use jobs::submit_job;` to export the handler for use in `lib.rs`.

4. **Mount `POST /v1/jobs` in `build_router()`.** In `crates/anvilml-server/src/lib.rs`:
   - Add `use handlers::jobs::submit_job;` to the imports
   - Add `.route("/v1/jobs", post(submit_job))` to the router chain (before `.with_state(state)`)

5. **Move `uuid` from dev-dependencies to dependencies in `anvilml-server/Cargo.toml`.** The handler uses `Uuid::new_v4()` at runtime, so it must be a regular dependency, not just a dev-dependency.

6. **Create `crates/anvilml-server/tests/jobs_tests.rs`.** Write integration tests:
   - `test_submit_job_returns_503_when_no_workers`: Build `AppState` with a fresh `NodeTypeRegistry` (never updated), dispatch POST with an empty graph `{}`, assert 503 with `error: "workers_unavailable"`.
   - `test_submit_job_returns_422_with_unknown_node_type`: Build registry with `LoadModel` registered, dispatch POST with a graph containing `{id: "n1", type: "GhostNode"}`, assert 422 with `error: "invalid_graph"`.
   - `test_submit_job_returns_202_with_valid_graph`: Build registry with `LoadModel` registered, dispatch POST with a valid graph `{nodes: [{id: "n1", type: "LoadModel"}]}`, assert 202 with `job_id` present and `queue_position: 0`.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `submit_job` | `anvilml-server::handlers::jobs` | `pub async fn submit_job(State<AppState>, Json<SubmitJobRequest>) -> Result<(StatusCode, Json<SubmitJobResponse>), AnvilError>` |
| `AnvilError::InvalidGraph` status code change | `anvilml-core::error` | Returns `StatusCode::UNPROCESSABLE_ENTITY` (422) instead of `StatusCode::BAD_REQUEST` (400) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/jobs.rs` | New handler module with `submit_job` function |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod jobs` and `pub use jobs::submit_job` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Import and mount `POST /v1/jobs` route in `build_router` |
| MODIFY | `crates/anvilml-core/src/error.rs` | Change `InvalidGraph` status from 400 to 422 |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Move `uuid` from dev-dependencies to dependencies |
| CREATE | `crates/anvilml-server/tests/jobs_tests.rs` | Integration tests for submit_job handler |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_submit_job_returns_503_when_no_workers` | Empty registry returns 503 WorkersUnavailable | Fresh `NodeTypeRegistry` never updated | POST `/v1/jobs` with `{}` | 503, `error: "workers_unavailable"` | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_submit_job_returns_503_when_no_workers` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_submit_job_returns_422_with_unknown_node_type` | Invalid graph returns 422 InvalidGraph with error list | Registry has `LoadModel` registered | POST with `{nodes: [{id: "n1", type: "GhostNode"}]}` | 422, `error: "invalid_graph"`, message contains unknown type | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_submit_job_returns_422_with_unknown_node_type` exits 0 |
| `crates/anvilml-server/tests/jobs_tests.rs` | `test_submit_job_returns_202_with_valid_graph` | Valid graph returns 202 with placeholder job_id | Registry has `LoadModel` registered | POST with `{nodes: [{id: "n1", type: "LoadModel"}]}` | 202, `job_id` is valid UUID, `queue_position: 0` | `cargo test -p anvilml-server --features mock-hardware -- jobs_tests::test_submit_job_returns_202_with_valid_graph` exits 0 |

## CI Impact

The new test file `crates/anvilml-server/tests/jobs_tests.rs` will be picked up by the standard `cargo test --workspace --features mock-hardware` CI job. No CI workflow files are modified. The OpenAPI drift gate (Gate 2) will need to be run after implementation because a new handler with `#[utoipa::path]` annotation changes the OpenAPI schema — but this is handled by a separate gate task, not by this task's CI changes.

## Platform Considerations

None identified. The `POST /v1/jobs` handler is a pure HTTP route with no platform-specific I/O, path handling, or conditional compilation. The `Uuid::new_v4()` call is cross-platform. The `Arc<NodeTypeRegistry>` dereference and `validate_graph` call are platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Changing `AnvilError::InvalidGraph` from 400 to 422 breaks existing tests or integration tests in `backend/tests/` that assert 400 for invalid graph responses. | Low | Medium | Scan all test files for assertions on `invalid_graph` status code before writing. If any exist, update them in the same task. The grep search for `"400"` and `"invalid_graph"` in test files will catch these. |
| `anvilml-server`'s `uuid` dev-dependency is not available at runtime — the handler will fail to compile if `Uuid::new_v4()` is called in non-test code. | Medium | High | Move `uuid` from `[dev-dependencies]` to `[dependencies]` in `anvilml-server/Cargo.toml` before writing the handler. This is a one-line change. |
| The `validate_graph` function expects `&NodeTypeRegistry` but `AppState.node_registry` is `Arc<NodeTypeRegistry>` — incorrect dereferencing will cause a type error. | Low | Medium | Use `&*state.node_registry` which performs `Deref` coercion from `Arc<T>` to `&T`. This is the established pattern used throughout the codebase. |
| The `AnvilError` `IntoResponse` for `InvalidGraph` currently produces 400 — if the handler returns `Err(AnvilError::InvalidGraph(...))` without the status code fix, the design spec's 422 requirement is violated. | Low | High | The status code fix in step 1 is a prerequisite. Verify the change compiles and the status_code() method returns 422 before writing tests that assert 422. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware -- jobs_tests` exits 0
- [ ] `cargo test -p anvilml-core --features mock-hardware` exits 0 (no regression from InvalidGraph status code change)
- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- dag` exits 0 (no regression from validate_graph)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (full workspace green)
- [ ] `cargo fmt --all -- --check` exits 0 (formatting gate)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (lint gate)
