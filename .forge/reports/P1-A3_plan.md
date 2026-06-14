# Plan Report: P1-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A3                                       |
| Phase       | 001 — Walking Skeleton                      |
| Description | anvilml-server: build_router wiring health handler |
| Depends on  | P1-A1, P1-A2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T12:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `pub fn build_router(state: AppState) -> Router` in `crates/anvilml-server/src/lib.rs` that wires the existing health handler into an axum `Router` and mounts it at `GET /health`. This completes the server crate's routing layer so that `main.rs` (P1-B1) can call `build_router` to produce a fully functional HTTP server. The observable state after completion: `cargo test -p anvilml-server` passes with an integration test confirming `Router::oneshot` on `GET /health` returns HTTP 200 with the expected JSON body.

## Scope

### In Scope
- **`crates/anvilml-server/src/lib.rs`**: Add `pub fn build_router(state: AppState) -> Router` that creates a `Router::new()`, mounts the health handler via `.route("/health", get(anvilml_server::health))`, and applies shared state via `.with_state(state)`. Add `use axum::routing::get;` import. Update the integration test in `tests/health_tests.rs` to use `build_router` instead of manual `Router::new()` construction.
- **`crates/anvilml-server/Cargo.toml`**: No changes needed — `axum`, `tower-http`, `tracing`, and `tokio` (as dev-dependency) are already present as workspace deps or direct deps. The task context mentions "add axum, tokio, tower-http to Cargo.toml as workspace deps" but these are already declared in the workspace `Cargo.toml` (lines 23, 22, 30 respectively) and referenced in the server crate's `Cargo.toml` (lines 12, 14, 15). No manifest changes required.

### Out of Scope
- Adding additional routes (system, jobs, models, workers, artifacts, nodes) — these belong to later tasks in Phase 002+.
- Implementing `backend/src/main.rs` (P1-B1) — that task binds the TCP listener and calls `build_router`.
- WebSocket handler wiring (`ws/` module) — not part of the walking skeleton.
- Any business logic — handlers call into scheduler/worker/registry only in later phases.

## Existing Codebase Assessment

Phase 000 established the workspace skeleton with all 9 crates present. Phase 001 tasks P1-A1 and P1-A2 have already been completed: `AppState` exists in `state.rs` with `Clone` derive and `new()` constructor; the health handler exists in `handlers/health.rs` returning `Json<Value>` with `status`, `version`, and `uptime_s` fields; `lib.rs` declares `pub mod handlers` and `pub mod state` and re-exports `health` and `AppState`. The test file `tests/health_tests.rs` already contains an integration test that manually constructs a router with `Router::new().route("/health", get(...)).with_state(state)`.

Established patterns:
- **Module structure**: `lib.rs` contains only `pub mod`, `pub use`, and crate-level `//!` doc comments. No implementation code.
- **Error handling**: Handlers return `Result<T, AnvilError>` or direct `Json<T>` types; `AnvilError` implements `IntoResponse`.
- **Test style**: Integration tests in `crates/{name}/tests/` use `Router::oneshot` with `tower::util::ServiceExt` — no live TCP binding.
- **Doc comments**: Every `pub` item has a `///` doc comment describing what it does, arguments, and return value.

No gap or discrepancy exists between the design doc and current source for this task — the health handler and AppState are already in place, ready for routing.

## Resolved Dependencies

All dependencies are already present in the workspace manifest. No new crates are introduced by this task.

| Type   | Name       | Version verified | MCP source | Feature flags confirmed |
|--------|------------|-----------------|------------|------------------------|
| crate  | axum       | 0.8.9           | Cargo.lock | json, http1, tokio, ws |
| crate  | tokio      | 1.52.3          | Cargo.lock | full                   |
| crate  | tower-http | 0.6.11          | Cargo.lock | cors, trace, timeout   |

Note: The task context mentions "Add axum, tokio, tower-http to Cargo.toml as workspace deps" — these are already declared in `Cargo.toml` workspace dependencies (lines 22–23, 30). No manifest modification is needed.

## Approach

1. **Add `build_router` function to `lib.rs`**: Implement `pub fn build_router(state: AppState) -> Router` that:
   - Creates a new `Router` via `Router::new()`
   - Mounts the health handler at path `/health` using `.route("/health", get(anvilml_server::health))`
   - Applies shared state with `.with_state(state)`
   - Returns the configured `Router`
   - Adds `use axum::routing::get;` to the imports
   - Adds a `///` doc comment describing the function's purpose, the `state` parameter, and the `Router` return type

   Rationale: Using `Router::new().route().with_state()` follows the exact pattern already established in `tests/health_tests.rs`, ensuring consistency. The `get` import from `axum::routing` is the standard axum 0.8 way to create route handlers.

2. **Update integration test in `tests/health_tests.rs`**: Replace the manual `Router::new().route(...).with_state(state)` construction with a call to `build_router(state)`. This verifies that `build_router` produces a router that behaves identically to the hand-crafted one. The test assertion logic (HTTP 200, JSON body with `status == "ok"`) remains unchanged.

   Rationale: Using `build_router` in the test exercises the actual production code path rather than duplicating the routing logic inline. It also ensures that any future route additions to `build_router` are automatically tested.

3. **Verify compilation and tests**: Run `cargo test -p anvilml-server` to confirm all tests pass. This exercise the full compilation including dev-dependencies and the integration test crate.

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| fn | `crates/anvilml-server/src/lib.rs` | `pub fn build_router(state: AppState) -> Router` |

This is the only new `pub` item introduced by this task. The function takes the existing `AppState` by value (it implements `Clone`, which axum's `State` extractor requires) and returns an `axum::Router` ready to be passed to `axum::serve`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/lib.rs` | Add `pub fn build_router(state: AppState) -> Router`; add `use axum::routing::get;` import |
| Modify | `crates/anvilml-server/tests/health_tests.rs` | Replace manual router construction with `build_router(state)` call |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/health_tests.rs` | `test_health_returns_200_with_status_key` | `Router::oneshot GET /health` via `build_router` returns HTTP 200 with JSON body containing `status == "ok"` | `cargo test -p anvilml-server -- health_tests` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_new` | `AppState::new()` sets recent `start_time` and stores version correctly | `cargo test -p anvilml-server -- state_tests::test_app_state_new` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_clone` | `AppState` clones correctly (version field matches) | `cargo test -p anvilml-server -- state_tests::test_app_state_clone` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_version_from_env` | `AppState::new()` accepts `CARGO_PKG_VERSION` via `&'static str` | `cargo test -p anvilml-server -- state_tests::test_app_state_version_from_env` exits 0 |

Acceptance: `cargo test -p anvilml-server` exits 0 with all 4 tests passing.

## CI Impact

No CI changes required. The test file `tests/health_tests.rs` already lives in the crate's `tests/` directory, which is picked up by `cargo test --workspace --features mock-hardware` as an integration test (compiled as a separate test crate). No new CI jobs, gates, or workflow files are introduced.

## Platform Considerations

None identified. The `Router::oneshot` pattern is platform-neutral — it exercises the axum router in-process without any network binding. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `axum::Router::with_state()` API shape differs from what the existing test uses — the method may have been renamed in axum 0.8.x (e.g., `layer(ExtractState::new(...))` or similar). | Low | High | The existing test file `tests/health_tests.rs` already uses `.with_state(state)` on a `Router::new()` with axum 0.8.9. Confirm this pattern compiles before proceeding. |
| The `get` import path may differ — `axum::routing::get` is the standard location but could have moved. | Low | Medium | The existing test already imports `use axum::routing::get;` on line 4, confirming this path works with axum 0.8.9. |
| Adding `build_router` to `lib.rs` might trigger clippy warnings about unused imports or missing doc comments. | Medium | Low | Follow established patterns: add `///` doc comment on the new function matching the style of existing `pub` items. Only import what is used (`axum::routing::get`). |
| The integration test update might break if `build_router` is called before the handler module is fully available. | Low | Medium | `lib.rs` already re-exports `health` via `pub use handlers::health::health;`, so `anvilml_server::health` is accessible from the test crate's perspective. Use the re-exported path. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server` exits 0
- [ ] `cargo test -p anvilml-server -- health_tests` exits 0 (health integration test passes)
- [ ] `cargo test -p anvilml-server -- state_tests` exits 0 (state tests pass)
- [ ] `head -1 .forge/reports/P1-A3_plan.md` prints `# Plan Report: P1-A3`
- [ ] `grep "^## " .forge/reports/P1-A3_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P1-A3_plan.md` returns a number greater than 40
