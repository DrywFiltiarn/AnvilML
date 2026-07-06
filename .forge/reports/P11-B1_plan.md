# Plan Report: P11-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-B1                                       |
| Phase       | 11 — Dynamic Node System                    |
| Description | anvilml-server: AppState struct (initial fields only) |
| Depends on  | P11-A1                                       |
| Project     | anvilml                                      |
| Planned at  | 2026-07-06T13:00:00Z                         |
| Attempt     | 1                                            |

## Objective

Create `AppState` in `crates/anvilml-server/src/state.rs` with exactly two fields (`config: Arc<ServerConfig>`, `node_registry: Arc<NodeTypeRegistry>`) and `#[derive(Clone)]`, establish the `mod state;` / `pub use state::AppState;` declarations in `lib.rs`, and write ≥2 integration tests in `crates/anvilml-server/tests/state_tests.rs` that verify construction, cloning, and Arc-sharing semantics. This establishes the `AppState` pattern that later phase tasks will incrementally extend with additional fields.

## Scope

### In Scope
- Create `crates/anvilml-server/src/state.rs` with `pub struct AppState { config: Arc<ServerConfig>, node_registry: Arc<NodeTypeRegistry> }` and `#[derive(Clone)]`.
- Add `mod state;` and `pub use state::AppState;` to `crates/anvilml-server/src/lib.rs`.
- Create `crates/anvilml-server/tests/state_tests.rs` with ≥2 tests:
  1. `test_app_state_constructs` — constructs `AppState` with default `ServerConfig` and empty `NodeTypeRegistry`, asserts fields exist.
  2. `test_app_state_clone_shares_node_registry` — clones `AppState`, mutates via one clone (registers a node type), reads via the other clone, asserts the registered type is visible through both.

### Out of Scope
- Any other `AppState` fields beyond `config` and `node_registry` (scheduler, workers, registry, hardware, db, broadcaster, artifact_store, env_report) — these are added incrementally by later tasks/phases. Adding them now would fail clippy's dead-code lint and contradict the no-speculative-scope convention.
- `lib.rs` changes beyond `mod state;` and `pub use state::AppState;` (no handler wiring, no route registration — that is P11-C1's scope).
- `build_router()` modification — remains unchanged in this task.

## Existing Codebase Assessment

**What already exists:** `anvilml-server` crate at `crates/anvilml-server/` with `lib.rs` declaring `pub mod handlers;` and a `build_router(start_time)` function that wires the `/health` route. The crate's `Cargo.toml` already depends on `anvilml-core` (path dependency), which re-exports both `ServerConfig` and `NodeTypeRegistry`. The `handlers/` directory has `mod.rs` declaring `pub mod health;` and `health.rs` implementing the liveness handler.

**Established patterns:**
- `lib.rs` contains only `pub mod`, `pub use`, and the `//!` crate-level doc comment — no implementation code. This is the absolute rule enforced across all crates (§12.3 of FORGE_AGENT_RULES.md).
- Integration tests live in `crates/{name}/tests/` as separate test crate files (not inline `#[cfg(test)]`). The existing `tests/health_tests.rs` demonstrates the pattern: construct the router, send in-process HTTP requests via `tower::util::ServiceExt::oneshot`, parse JSON responses.
- Types from `anvilml-core` are used directly (e.g., `ServerConfig::default()` is available through the re-export).
- The `AppState` design in `ANVILML_DESIGN.md §13.2` uses `Arc` for shared ownership of all fields — this task follows that pattern with only the two fields it needs.

**Gap between design doc and source:** The design doc (§13.2) specifies ten fields on `AppState`, but this task intentionally creates only two. This is by design — the remaining fields are added incrementally. The current `lib.rs` has no `mod state;` declaration, confirming `state.rs` does not yet exist.

## Resolved Dependencies

No new external crates or packages are introduced. This task uses only existing workspace path dependencies already declared in `crates/anvilml-server/Cargo.toml`:

| Type   | Name        | Version verified | MCP source | Feature flags confirmed |
|--------|-------------|-----------------|------------|------------------------|
| crate  | anvilml-core| (path dep)      | Cargo.toml | n/a                    |

`Arc` is from `std::sync::Arc` (Rust standard library). No MCP lookup required.

## Approach

1. **Create `crates/anvilml-server/src/state.rs`.** Write a `pub struct AppState` with exactly two fields:
   - `pub config: Arc<ServerConfig>` — the server configuration.
   - `pub node_registry: Arc<NodeTypeRegistry>` — the dynamic node type registry.
   Add `#[derive(Clone)]` to derive the clone implementation. Add a `///` doc comment on the struct describing its purpose and the incremental field-growth pattern. Add `use std::sync::Arc;` at the top.

2. **Modify `crates/anvilml-server/src/lib.rs`.** Add `pub mod state;` after the existing `pub mod handlers;` line. Add `pub use state::AppState;` after the module declarations. Do not modify `build_router()` — that is handled by later tasks (P11-C1, P11-D1).

3. **Create `crates/anvilml-server/tests/state_tests.rs`.** Write two integration tests:
   - `test_app_state_constructs`: Import `AppState` from `anvilml_server`, construct it with `ServerConfig::default()` and `NodeTypeRegistry::new()`, both wrapped in `Arc::new()`. Assert the fields exist and the registry is initially empty (`state.node_registry.is_empty()`).
   - `test_app_state_clone_shares_node_registry`: Construct `AppState` as above. Clone it to produce `cloned`. Register a single `NodeTypeDescriptor` via `state.node_registry.register_all(vec![descriptor])`. Read back via `cloned.node_registry.list()` and assert the descriptor is present — this proves both clones share the same `Arc<NodeTypeRegistry>` heap allocation.

4. **Verify compilation.** Run `cargo check -p anvilml-server --features mock-hardware` to confirm the new module compiles and `pub use` re-export works.

5. **Run tests.** Run `cargo test -p anvilml-server --test state_tests` to confirm both tests pass.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `pub struct AppState` | `anvilml_server::AppState` | Application state holding `config` and `node_registry` as `Arc`-wrapped fields. `#[derive(Clone)]`. |
| `pub config: Arc<ServerConfig>` | `anvilml_server::AppState::config` | Server configuration, shared via Arc. |
| `pub node_registry: Arc<NodeTypeRegistry>` | `anvilml_server::AppState::node_registry` | Dynamic node type registry, shared via Arc. |
| `pub use state::AppState` | `anvilml_server::AppState` (re-export) | Re-exported from `lib.rs` for ergonomic access. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/state.rs` | `AppState` struct with `config` and `node_registry` fields, `#[derive(Clone)]` |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Add `pub mod state;` and `pub use state::AppState;` |
| CREATE | `crates/anvilml-server/tests/state_tests.rs` | Integration tests: construction and clone-sharing |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version 0.1.2 → 0.1.3 |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_constructs` | `AppState` constructs with default `ServerConfig` and empty `NodeTypeRegistry`; fields are accessible; registry reports `is_empty() == true` | `cargo test -p anvilml-server --test state_tests test_app_state_constructs` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_clone_shares_node_registry` | Cloning `AppState` shares the same `Arc<NodeTypeRegistry>`; mutating via one clone is visible through the other | `cargo test -p anvilml-server --test state_tests test_app_state_clone_shares_node_registry` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/state_tests.rs` is automatically picked up by `cargo test --workspace --features mock-hardware` which runs in the `rust-linux` and `rust-windows` CI jobs. No new file types, gates, or test modules are added — the crate's `Cargo.toml` already has `[dev-dependencies]` for `tokio`, `tower`, and `serde_json` which the tests use.

## Platform Considerations

None identified. `Arc`, `ServerConfig`, and `NodeTypeRegistry` are all platform-neutral Rust types. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `anvilml-core` re-exports `ServerConfig` and `NodeTypeRegistry` at the crate root, but the re-export path might change in a future task. This task uses the current path (`anvilml_server::ServerConfig`, `anvilml_server::NodeTypeRegistry`), which matches the `lib.rs` re-exports. | Low | Low | Use the re-exported names directly from `anvilml_server::` — if re-exports change, the compiler will fail and the ACT agent will discover it immediately. |
| Adding `mod state;` to `lib.rs` could conflict with future tasks that also modify `lib.rs` (P11-C1, P11-D1). The `pub mod` / `pub use` additions are additive and do not conflict. | Low | Low | Add the new lines after existing declarations; do not reorder or remove existing content. |
| Tests use `NodeTypeDescriptor` which requires constructing a valid descriptor for the clone-sharing test. The struct has `type_name`, `display_name`, `category`, `description`, `inputs`, `outputs` fields. | Low | Low | Use minimal synthetic values: `type_name: "TestNode".to_string()`, empty strings for other fields, empty vectors for `inputs`/`outputs`. This is a test-only value. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-server --test state_tests` exits 0 (≥2 tests)
