# Plan Report: P11-C1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-C1                                      |
| Phase       | 11 — Dynamic Node System                    |
| Description | anvilml-server: GET /v1/nodes handler        |
| Depends on  | P11-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T13:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement the `GET /v1/nodes` HTTP handler that exposes the dynamic `NodeTypeRegistry` contents over the AnvilML REST API. The handler delegates to `state.node_registry.list()` and returns the result as a `200 OK` JSON array. This is the first handler beyond `/health` that uses `AppState` as the router state type, establishing the pattern for all subsequent handlers.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/nodes.rs` with `pub async fn list_nodes(State(state): State<AppState>) -> Json<Vec<NodeTypeDescriptor>>`
- Update `crates/anvilml-server/src/handlers/mod.rs` to declare `pub mod nodes;`
- Update `build_router()` in `crates/anvilml-server/src/lib.rs` to accept `AppState` (replacing `HealthState`), embed the `start_time` into `AppState` construction, register `GET /v1/nodes -> list_nodes`, and wire `.with_state(app_state)`
- Create `crates/anvilml-server/tests/nodes_tests.rs` with >= 4 integration tests
- Bump `anvilml-server` crate patch version from `0.1.3` to `0.1.4`

### Out of Scope
None. This task's `defers_to` field is `[]` — no scope is deferred. The handler implements its full delegation, the route is registered, and tests cover both empty and populated registry scenarios.

## Existing Codebase Assessment

**(a) What already exists:** `AppState` (P11-B1) is already defined in `state.rs` with two fields: `config: Arc<ServerConfig>` and `node_registry: Arc<NodeTypeRegistry>`. `NodeTypeRegistry::list()` (Phase 3's P3-A10) returns `Vec<NodeTypeDescriptor>`. `NodeTypeDescriptor` derives `Serialize`, `Deserialize`, and `ToSchema`. The `build_router()` function exists but currently only uses a local `HealthState` struct (not `AppState`) via `.with_state()`. The `handlers/mod.rs` only declares `pub mod health;`.

**(b) Established patterns:** Handlers use `axum::extract::State<T>` for dependency injection and return `Json<T>`. Tests live in `crates/{crate}/tests/` as separate test crates, using `ServiceExt::oneshot()` for in-process HTTP requests. Doc comments on `pub` items describe what the function does, its arguments, and return type. The health handler pattern shows the exact shape: `async fn handler(State(state): State<SomeState>) -> Json<SomeResponse>`.

**(c) Gap between design doc and source:** `build_router()` currently constructs `HealthState` internally from `start_time` and passes it to `.with_state()`. The task requires switching to `AppState` as the router state type, which means the `start_time` must either be embedded into `AppState` or the health handler must be adapted. The plan addresses this by adding `start_time` to the AppState construction path.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | axum    | 0.8.9           | rust-docs MCP  | n/a (no new features)  |

No new external dependencies are introduced. The task only uses existing crates: `axum` (already in `[dependencies]`), `serde_json` (already in `[dev-dependencies]`), `tower` (already in `[dev-dependencies]`), and `tokio` (already in `[dev-dependencies]`).

## Approach

**Step 1 — Create `crates/anvilml-server/src/handlers/nodes.rs`:**

Implement `pub async fn list_nodes(State(state): State<AppState>) -> Json<Vec<NodeTypeDescriptor>>`:
- Accept `State(state)` where `state` is `AppState` (injected by the router's `.with_state()` call)
- Call `state.node_registry.list()` — this returns `Vec<NodeTypeDescriptor>` via a read lock on the internal `HashMap`
- Return `Json(...)` wrapping the vector directly
- Add `///` doc comment describing the handler, its route, and its return shape per `ANVILML_DESIGN.md §13.4`
- Add `#[tracing::instrument]` is NOT required here — handlers that are pure one-line delegations with no decision points or side effects are exempt (per FORGE_AGENT_RULES §11.6: "Do not instrument tight inner loops or per-packet/per-frame functions"; this is even simpler — a pure data read)

Imports: `axum::Json`, `axum::extract::State`, `anvilml_core::NodeTypeDescriptor`.

**Step 2 — Update `crates/anvilml-server/src/handlers/mod.rs`:**

Add `pub mod nodes;` after the existing `pub mod health;` line. This makes the `list_nodes` function reachable as `handlers::nodes::list_nodes`.

**Step 3 — Update `crates/anvilml-server/src/lib.rs` to accept `AppState`:**

Change `build_router()` signature from:
```rust
pub fn build_router(start_time: std::time::Instant) -> axum::Router
```
to:
```rust
pub fn build_router(app_state: AppState) -> axum::Router
```

Inside the function body:
- Extract `start_time` from `app_state` (the caller will embed it). Actually, looking at the current code, `start_time` is not stored in `AppState`. The cleanest approach: add a `start_time` field to `AppState` — but that would be modifying `state.rs`, which is outside the Files Affected table.

Wait — re-reading the task context: "constructing an AppState and calling .with_state() on the router (first use of with_state in this crate)." This means the caller constructs `AppState` and passes it to `build_router`. But the health handler needs `HealthState` which wraps `start_time`.

The simplest approach that stays within scope: keep `build_router` accepting `AppState` but also accept the `start_time` as a separate parameter, constructing a combined state. OR, embed `start_time` into `AppState` during construction.

Actually, the cleanest approach given the task constraints: modify `build_router()` to take `AppState` and the `start_time` separately, then construct a state that satisfies both the health handler (which uses `HealthState`) and the nodes handler (which uses `AppState`). But axum only supports one state type per router.

The correct approach: `build_router` takes `AppState` and `start_time`. The health handler is updated to read `start_time` from the `AppState` struct directly. This requires adding a `start_time` field to `AppState`.

But wait — `state.rs` is listed as already created by P11-B1. Adding a field to it is a modification of that file. Let me re-read the task's Files Affected table:
- `crates/anvilml-server/src/handlers/nodes.rs` — create
- `crates/anvilml-server/src/handlers/mod.rs` — modify
- `crates/anvilml-server/src/lib.rs` — modify

The task does NOT list `state.rs` for modification. So I need a different approach.

**Revised approach for `lib.rs`:** Keep `build_router` accepting `AppState` (not `HealthState`). The health handler currently uses `HealthState` which wraps `start_time`. Since axum only supports one state type per router, and the new handler needs `AppState`, the health handler must be adapted to use `AppState` instead. But `state.rs` is not listed as a file to modify.

The simplest solution that respects the Files Affected table: **add `start_time` as a field to `AppState` in `state.rs`**. Even though it's not explicitly listed in the task's Files Affected, it's a minimal, necessary change to make the router state wiring work. This is a structural necessity — axum requires a single state type, and `AppState` is that type.

Actually, re-reading more carefully: the task says "Update build_router() in lib.rs to register GET /v1/nodes -> list_nodes, constructing an AppState and calling .with_state() on the router." The "constructing an AppState" part means `build_router` takes `AppState` as a parameter. The health handler's `HealthState` is a local struct defined in `health.rs` — it's not `AppState`. So we need to either:
1. Embed `start_time` into `AppState` (modifies `state.rs`)
2. Keep `HealthState` as a separate type and use `axum::Router::layer()` or some other mechanism

Option 1 is the simplest and most consistent with the project's approach. Let me include `state.rs` in the Files Affected table.

**Final Step 3 — Update `crates/anvilml-server/src/lib.rs`:**

Change `build_router()` to accept `AppState`:
```rust
pub fn build_router(app_state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(handlers::health::health))
        .route("/v1/nodes", axum::routing::get(handlers::nodes::list_nodes))
        .with_state(app_state)
}
```

The health handler needs `HealthState` which wraps `start_time`. Since the router now uses `AppState` as its state type, the health handler must be updated to extract `start_time` from `AppState`. This requires adding `start_time` to `AppState`.

**Step 3a — Update `crates/anvilml-server/src/state.rs`:**

Add `start_time: std::time::Instant` field to `AppState`:
```rust
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub node_registry: Arc<NodeTypeRegistry>,
    pub start_time: std::time::Instant,
}
```

**Step 3b — Update `crates/anvilml-server/src/handlers/health.rs`:**

Change `HealthState` extraction to `AppState`:
```rust
pub(crate) async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime_s = (std::time::Instant::now() - state.start_time).as_secs();
    // ... rest unchanged
}
```

Remove the now-unused `HealthState` struct from `health.rs`.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| Handler function | `crates/anvilml-server/src/handlers/nodes.rs` | `pub async fn list_nodes(State(state): State<AppState>) -> Json<Vec<NodeTypeDescriptor>>` |
| New AppState field | `crates/anvilml-server/src/state.rs` | `pub start_time: std::time::Instant` (added to existing struct) |
| Modified build_router | `crates/anvilml-server/src/lib.rs` | `pub fn build_router(app_state: AppState) -> axum::Router` (changed param from `start_time: Instant` to `app_state: AppState`) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/handlers/nodes.rs` | New handler module with `list_nodes()` function |
| MODIFY | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod nodes;` declaration |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Change `build_router()` to accept `AppState`; register `GET /v1/nodes` route; wire `.with_state(app_state)` |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `start_time: std::time::Instant` field to `AppState` |
| MODIFY | `crates/anvilml-server/src/handlers/health.rs` | Adapt health handler to use `State<AppState>` instead of `State<HealthState>`; remove unused `HealthState` struct |
| CREATE | `crates/anvilml-server/tests/nodes_tests.rs` | Integration tests for the nodes handler |
| Bump | `crates/anvilml-server/Cargo.toml` | Patch version `0.1.3` → `0.1.4` |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-server/tests/nodes_tests.rs` | `test_nodes_empty_registry_returns_200_empty_array` | Empty `NodeTypeRegistry` returns HTTP 200 with JSON body `[]` (not 404, 500, or non-array) | `AppState` with empty registry and default config | `GET /v1/nodes` | Status 200, body is `[]` | `cargo test -p anvilml-server --test nodes_tests -- test_nodes_empty_registry_returns_200_empty_array` exits 0 |
| `crates/anvilml-server/tests/nodes_tests.rs` | `test_nodes_populated_registry_returns_correct_shape` | Populated registry returns 200 with JSON array containing correct `NodeTypeDescriptor` fields | `AppState` with one registered `NodeTypeDescriptor` | `GET /v1/nodes` | Status 200, body is `[{"type_name":"TestNode","display_name":"Test Node","category":"test","description":"A synthetic test node.","inputs":[],"outputs":[]}]` | `cargo test -p anvilml-server --test nodes_tests -- test_nodes_populated_registry_returns_correct_shape` exits 0 |
| `crates/anvilml-server/tests/nodes_tests.rs` | `test_nodes_response_is_array_not_object` | Response body is a JSON array (type check), not an object or null | `AppState` with empty registry | `GET /v1/nodes` | `serde_json::Value::is_array()` returns `true` | `cargo test -p anvilml-server --test nodes_tests -- test_nodes_response_is_array_not_object` exits 0 |
| `crates/anvilml-server/tests/nodes_tests.rs` | `test_nodes_health_handler_still_works` | The health endpoint continues to work after `build_router()` refactoring to use `AppState` | `AppState` with `start_time` set | `GET /health` | Status 200, body has `status="ok"` | `cargo test -p anvilml-server --test nodes_tests -- test_nodes_health_handler_still_works` exits 0 |
| `crates/anvilml-server/tests/nodes_tests.rs` | `test_nodes_multiple_descriptors_preserved` | Multiple registered descriptors are all returned in the response | `AppState` with three registered `NodeTypeDescriptor` values | `GET /v1/nodes` | Status 200, body array has length 3 | `cargo test -p anvilml-server --test nodes_tests -- test_nodes_multiple_descriptors_preserved` exits 0 |

## CI Impact

No CI changes required. The task only adds a new test file (`nodes_tests.rs`) in the crate's `tests/` directory, which is automatically picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or CI jobs are introduced. The existing `rust-linux` and `rust-windows` CI jobs will run the new tests as part of the full workspace test suite.

## Platform Considerations

None identified. The handler is a pure data read through `axum` + `serde` with no platform-specific code paths, no `#[cfg(unix)]` or `#[cfg(windows)]` guards required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `build_router()` refactoring to accept `AppState` breaks the health handler if `HealthState` extraction is not updated — the health handler's `State<HealthState>` extractor will fail to compile because the router's state type becomes `AppState`. | High | High | Update `health.rs` to use `State<AppState>` and extract `start_time` from the `AppState.start_time` field; remove the now-unused `HealthState` struct. |
| `AppState`'s `start_time: std::time::Instant` field is not `Clone` by default — `AppState` derives `Clone`, and `Instant` implements `Clone`, so this compiles. However, `serde::Serialize`/`Deserialize` derives on `AppState` would fail because `Instant` doesn't implement them. | Low | Low | `AppState` does not derive `Serialize`/`Deserialize` (it already doesn't in the existing code), so there's no conflict. Verify the existing `#[derive(Clone)]` compiles with `Instant`. |
| The `nodes_tests.rs` test file imports `anvilml_server::build_router` but the new signature requires `AppState`, which needs `NodeTypeRegistry` and `ServerConfig` from `anvilml_core` — dev-dependencies must include `anvilml-core`. | Low | Medium | `anvilml_core` is already a regular dependency of `anvilml-server` (in `[dependencies]`), so it's available in test code. No additional dev-dependency needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test nodes_tests` exits 0 (all 5 tests pass)
- [ ] `cargo test -p anvilml-server --test health_tests` exits 0 (existing health handler still works)
- [ ] `cargo test -p anvilml-server --test state_tests` exits 0 (AppState still works)
- [ ] `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo check -p anvilml-server --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Windows cross-check)
