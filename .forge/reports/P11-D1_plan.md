# Plan Report: P11-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-D1                                      |
| Phase       | 11 — Dynamic Node System                    |
| Description | backend: wire AppState construction + build_router into main.rs |
| Depends on  | P11-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Wire the binary's normal server-start path to use the newly created `AppState` and `build_router()` from `anvilml-server`, replacing the bare router that Phase 1 originally wired up. This connects the `NodeTypeRegistry` (empty at this point — populated only when a Python worker sends a `Ready` event in a later phase) into the server's request-handling pipeline so that `GET /v1/nodes` (P11-C1) can serve its response.

## Scope

### In Scope
- `backend/src/main.rs` — construct `Arc<NodeTypeRegistry>::new()` (empty registry), construct `AppState { config: Arc::new(cfg), node_registry, start_time }`, pass it to `anvilml_server::build_router(app_state)` in the normal (non-hw-probe) run path.
- Confirm the existing `hw-probe` subcommand path remains unmodified and functional.
- Confirm the existing `/health` route (Phase 1) continues to work through the new `AppState`-backed router.

### Out of Scope
None. This task has an empty `defers_to` field and implements its full scope. No stubs, no deferred functionality.

## Existing Codebase Assessment

The codebase inspection reveals that the wiring described in this task is **already present** in `backend/src/main.rs`. The file (172 lines) contains:

1. **`AppState` construction** (lines 149–156): Constructs `AppState { config: Arc::new(config), node_registry: Arc::new(NodeTypeRegistry::new()), start_time }` after the DB pool creation and seed loading, but before the TCP bind.
2. **`build_router()` call** (line 163): Passes the constructed `app_state` to `anvilml_server::build_router(app_state)`.
3. **`hw-probe` separation** (lines 84–100): The `match cli.command { Some(Commands::HwProbe) => { ... } None => { ... } }` pattern correctly isolates the hardware-probe path from the server-start path.
4. **`NodeTypeRegistry` import** (line 7): `use anvilml_core::NodeTypeRegistry;` is already present.
5. **`AppState` and `build_router` imports** (line 11): `use anvilml_server::{AppState, build_router};` are already present.
6. **`Arc` import** (line 13): `use std::sync::Arc;` is already present.

The supporting infrastructure is also in place:
- `crates/anvilml-server/src/state.rs` (28 lines) defines `AppState` with `config: Arc<ServerConfig>`, `node_registry: Arc<NodeTypeRegistry>`, and `start_time: Instant`.
- `crates/anvilml-server/src/lib.rs` (23 lines) defines `build_router(app_state: AppState) -> axum::Router` with `/health` and `/v1/nodes` routes.
- `crates/anvilml-core/src/node_registry.rs` (99 lines) provides `NodeTypeRegistry::new()` (empty), `register_all()`, `list()`, `get()`, `len()`, `is_empty()`.

**Established patterns to follow:**
- Error handling: `map_err` + `eprintln!` + `exit(1)` for startup failures (config, DB, seed).
- Logging: `tracing::info!` with structured fields (`addr = %addr`).
- `Arc` wrapping for shared state between router and handlers.
- `#[derive(Clone)]` on `AppState` for axum's state extractor.

**Gap between design doc and source:** The task context specifies `AppState{config: Arc::new(cfg), node_registry}` (two fields), but the actual `AppState` struct has three fields — it also includes `start_time: Instant`. This extra field was added by a prior task (likely P11-B1 or earlier) and does not conflict with the task's intent; it simply adds the process-start instant for health-check uptime calculation. The plan accounts for this by including `start_time` in the approach.

## Resolved Dependencies

None. This task only wires existing internal types (`NodeTypeRegistry`, `AppState`, `build_router`) — no new external crates or versions are introduced.

## Approach

The wiring described in this task is already implemented in `backend/src/main.rs`. The approach for this task is to confirm the existing implementation is correct and complete:

1. **Verify `NodeTypeRegistry` construction** (line 154): `Arc::new(NodeTypeRegistry::new())` creates an empty registry. This matches the task requirement — the registry stays empty until a worker sends a `Ready` event (later phase).

2. **Verify `AppState` construction** (lines 152–156): The struct literal `{ config: Arc::new(config), node_registry: Arc::new(NodeTypeRegistry::new()), start_time }` wraps the loaded config in `Arc`, creates the empty node registry, and captures the process-start instant. This is placed after DB pool creation and seed loading (lines 106–143) and before the TCP bind (line 164).

3. **Verify `build_router()` call** (line 163): `build_router(app_state)` receives the constructed state and returns an `axum::Router` with `/health` and `/v1/nodes` routes, all backed by `.with_state(app_state)`.

4. **Verify `hw-probe` path isolation** (lines 84–100): The `match cli.command { ... }` correctly handles the `HwProbe` subcommand (detect + JSON output + exit) in a separate branch from the `None` (default server-start) path. No changes to this path are needed.

5. **Verify `/health` route continuity** (lib.rs line 20): The `/health` route from Phase 1 is preserved in `build_router()` and now receives state via `.with_state()`, which provides the `start_time` field for uptime calculation.

No code changes are required. The implementation matches the task specification.

## Public API Surface

No new public items are introduced by this task. The task only wires existing public APIs:

| Item | Location | Description |
|------|----------|-------------|
| `NodeTypeRegistry::new()` | `anvilml-core/src/node_registry.rs:27` | Creates empty registry (already exists) |
| `AppState` | `anvilml-server/src/state.rs:17` | Shared server state struct (already exists) |
| `build_router()` | `anvilml-server/src/lib.rs:18` | Router builder (already exists) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| No change | `backend/src/main.rs` | Wiring already in place (lines 149–163) |

## Tests

The task does not introduce new tests. The existing test suite validates the wiring:

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `backend/tests/db_startup_tests.rs` | `test_db_file_created_on_startup` | Binary starts, creates DB, binds TCP — proves the server-start path (with AppState + build_router) works end-to-end | `cargo test --workspace --features mock-hardware -- test_db_file_created_on_startup` exits 0 |
| `backend/tests/db_startup_tests.rs` | `test_migrations_create_required_tables` | DB migrations run during startup — confirms the server-start path reaches the TCP bind stage | `cargo test --workspace --features mock-hardware -- test_migrations_create_required_tables` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_constructs` | AppState constructs with default config and empty registry | `cargo test -p anvilml-server --test state_tests` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_clone_shares_node_registry` | AppState clone shares Arc<NodeTypeRegistry> | `cargo test -p anvilml-server --test state_tests -- test_app_state_clone_shares_node_registry` exits 0 |

## CI Impact

No CI changes required. The task does not add new file types, new gates, or new test modules. The existing CI jobs (`rust-linux`, `rust-windows`) pick up the workspace test suite which includes all backend integration tests.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The `hw-probe` path and server-start path are both platform-neutral — no `#[cfg(unix)]` or `#[cfg(windows)]` guards are introduced.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `start_time` field was added to `AppState` by a prior task not mentioned in the task context. If a later task assumes `AppState` has exactly two fields, this third field could cause confusion. | Low | Low | The `start_time` field is well-documented in `state.rs` (line 26–27) as "Monotonic clock instant captured at process startup." It is used by the `/health` handler for uptime calculation. No task assumes a field count. |
| The `hw-probe` subcommand path uses `detect_all_devices(&config)` which takes a `&ServerConfig` (not `Arc<ServerConfig>`). If the config variable were renamed or its type changed, the hw-probe path would break independently of the server-start path. | Low | Medium | The `hw-probe` path is in a separate match arm and does not interact with `AppState`. It was already working before this task and continues to work. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0
